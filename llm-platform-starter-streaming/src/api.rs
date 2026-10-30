use crate::{db, registry};
use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::error;

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    active_requests: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

pub fn router(pool: SqlitePool) -> Router {
    let state = AppState {
        pool,
        active_requests: Arc::new(Mutex::new(HashMap::new())),
    };
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/v1/models", get(models))
        .route("/v1/conversations", get(list_conversations).post(create_conversation))
        .route("/v1/conversations/{id}/messages", get(list_messages).post(add_message))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/requests/{id}/cancel", post(cancel_request))
        .with_state(state)
}


#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<crate::inference::ChatMessage>,
    #[serde(default)]
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: &'static str,
    model: String,
    choices: Vec<ChatCompletionChoice>,
}
#[derive(Debug, Serialize)]
struct ChatCompletionChoice {
    index: u8,
    message: crate::inference::ChatMessage,
    finish_reason: &'static str,
}

async fn chat_completions(
    State(state): State<AppState>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<axum::response::Response, (StatusCode, Json<Value>)> {
    let pool = state.pool.clone();
    if let Err(err) = db::require_migrated(&pool).await {
        error!(error = %err, "chat endpoint unavailable because schema is not ready");
        return Err((StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":{"message":"database schema is not ready"}}))));
    }
    if request.model.trim().is_empty() || request.messages.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error":{"message":"model and messages are required"}}))));
    }
    if request.messages.len() > 256 {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error":{"message":"at most 256 messages are allowed per request"}}))));
    }
    if !request.messages.iter().any(|m| m.role == "user") {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error":{"message":"at least one user message is required"}}))));
    }
    if request.messages.iter().any(|m| !["system", "user", "assistant", "tool"].contains(&m.role.as_str())) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error":{"message":"unsupported message role"}}))));
    }
    if request.messages.iter().any(|m| m.content.trim().is_empty() || m.content.len() > 100_000) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error":{"message":"message content must be between 1 and 100000 bytes"}}))));
    }

    if request.stream {
        let upstream = match crate::inference::open_stream_by_name(&pool, &request.model, &request.messages).await {
            Ok(response) => response,
            Err(err) => {
                error!(error = %err, model = %request.model, "streaming chat setup failed");
                return Err((StatusCode::BAD_GATEWAY, Json(json!({"error":{"message":"chat stream could not be started; inspect server logs for details"}}))));
            }
        };

        let request_id = uuid::Uuid::new_v4().to_string();
        let cancellation = CancellationToken::new();
        state.active_requests.lock().await.insert(request_id.clone(), cancellation.clone());
        let (sender, mut receiver) = mpsc::channel::<Bytes>(16);
        let active_requests = state.active_requests.clone();
        let task_request_id = request_id.clone();
        let model = request.model.clone();
        tokio::spawn(async move {
            pump_provider_sse(upstream, sender, cancellation, task_request_id.clone(), model).await;
            active_requests.lock().await.remove(&task_request_id);
        });

        let body_stream = async_stream::stream! {
            while let Some(bytes) = receiver.recv().await {
                yield Ok::<Bytes, Infallible>(bytes);
            }
        };
        let mut response = Body::from_stream(body_stream).into_response();
        response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/event-stream; charset=utf-8"));
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache, no-transform"));
        response.headers_mut().insert("x-accel-buffering", HeaderValue::from_static("no"));
        // UUIDs are ASCII and always valid HTTP header values.
        response.headers_mut().insert("x-llm-platform-request-id", HeaderValue::from_str(&request_id).expect("UUID is a valid header value"));
        return Ok(response);
    }

    match crate::inference::complete_messages_by_name(&pool, &request.model, &request.messages).await {
        Ok(content) => Ok(Json(ChatCompletionResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: "chat.completion",
            model: request.model,
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: crate::inference::ChatMessage { role: "assistant".into(), content },
                finish_reason: "stop",
            }],
        }).into_response()),
        Err(err) => {
            error!(error = %err, "chat completion failed");
            Err((StatusCode::BAD_GATEWAY, Json(json!({"error":{"message":"chat completion failed; inspect server logs for details"}}))))
        }
    }
}

async fn cancel_request(
    State(state): State<AppState>,
    Path(request_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let token = state.active_requests.lock().await.get(&request_id).cloned();
    if let Some(token) = token {
        token.cancel();
        Ok(Json(json!({"id": request_id, "status": "cancellation_requested"})))
    } else {
        Err(api_error(StatusCode::NOT_FOUND, "active request not found"))
    }
}

async fn pump_provider_sse(
    mut upstream: reqwest::Response,
    sender: mpsc::Sender<Bytes>,
    cancellation: CancellationToken,
    request_id: String,
    model: String,
) {
    let created = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let mut buffer: Vec<u8> = Vec::new();
    let mut finished = false;

    while !finished {
        let next_chunk = tokio::select! {
            _ = cancellation.cancelled() => break,
            _ = sender.closed() => {
                cancellation.cancel();
                break;
            },
            chunk = upstream.chunk() => chunk,
        };
        match next_chunk {
            Ok(Some(chunk)) => buffer.extend_from_slice(&chunk),
            Ok(None) => {
                if !buffer.is_empty() {
                    if let Some(data) = parse_sse_line(&buffer) {
                        if data == "[DONE]" {
                            finished = true;
                        } else if let Some(payload) = translate_provider_chunk(&data, &request_id, &model, created) {
                            if !send_sse_data(&sender, &cancellation, payload).await { break; }
                        }
                    }
                }
                break;
            }
            Err(err) => {
                error!(error = %err, request_id = %request_id, "provider stream failed while reading");
                let error_payload = json!({"error":{"message":"provider stream failed"}}).to_string();
                let event = Bytes::from(format!("event: error\ndata: {error_payload}\n\n"));
                let _ = sender.try_send(event);
                break;
            }
        }

        while let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            let raw_line: Vec<u8> = buffer.drain(..=newline).collect();
            let Some(data) = parse_sse_line(&raw_line) else { continue; };
            if data == "[DONE]" {
                finished = true;
                break;
            }
            if let Some(payload) = translate_provider_chunk(&data, &request_id, &model, created) {
                if !send_sse_data(&sender, &cancellation, payload).await {
                    finished = true;
                    break;
                }
            }
        }
    }

    // Do not wait indefinitely for a slow/disconnected HTTP client. EOF also
    // terminates SSE clients, so the terminal marker is best-effort.
    let _ = sender.try_send(Bytes::from_static(b"data: [DONE]\n\n"));
}

fn parse_sse_line(raw_line: &[u8]) -> Option<String> {
    let line = String::from_utf8_lossy(raw_line);
    let line = line.trim_end_matches(|character| character == '\r' || character == '\n');
    line.strip_prefix("data:").map(|data| data.trim().to_string())
}

fn translate_provider_chunk(data: &str, request_id: &str, model: &str, created: u64) -> Option<String> {
    let provider_value: Value = serde_json::from_str(data).ok()?;
    let choice = provider_value.get("choices")?.as_array()?.first()?;
    let delta = choice.get("delta").cloned().unwrap_or_else(|| json!({}));
    let finish_reason = choice.get("finish_reason").cloned().unwrap_or(Value::Null);
    Some(json!({
        "id": format!("chatcmpl-{request_id}"),
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish_reason}]
    }).to_string())
}

async fn send_sse_data(sender: &mpsc::Sender<Bytes>, cancellation: &CancellationToken, payload: String) -> bool {
    let bytes = Bytes::from(format!("data: {payload}\n\n"));
    tokio::select! {
        _ = cancellation.cancelled() => false,
        result = sender.send(bytes) => result.is_ok(),
    }
}



#[derive(Debug, Deserialize)]
struct CreateConversationRequest {
    title: Option<String>,
    model: Option<String>,
    system_prompt: Option<String>,
}

#[derive(Debug, Serialize)]
struct Conversation {
    id: String,
    title: String,
    selected_model_id: Option<String>,
    system_prompt: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct AddMessageRequest {
    content: String,
}

#[derive(Debug, Serialize)]
struct StoredMessage {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    sequence_number: i64,
    status: String,
    created_at: String,
}

async fn create_conversation(
    State(state): State<AppState>,
    Json(request): Json<CreateConversationRequest>,
) -> Result<(StatusCode, Json<Conversation>), (StatusCode, Json<Value>)> {
    let pool = state.pool.clone();
    if let Err(err) = db::require_migrated(&pool).await {
        error!(error = %err, "conversation endpoint unavailable because schema is not ready");
        return Err(api_error(StatusCode::SERVICE_UNAVAILABLE, "database schema is not ready"));
    }
    let title = request.title.unwrap_or_else(|| "New conversation".to_string());
    if title.trim().is_empty() || title.len() > 200 {
        return Err(api_error(StatusCode::BAD_REQUEST, "title must be between 1 and 200 characters"));
    }
    let model_id = if let Some(model_name) = request.model.as_deref() {
        match sqlx::query_scalar::<_, String>(
            "SELECT m.id FROM models m JOIN model_providers p ON p.id=m.provider_id \
             WHERE p.enabled=1 AND m.status!='disabled' \
             AND (m.external_model_id=? COLLATE NOCASE OR m.display_name=? COLLATE NOCASE) LIMIT 1"
        ).bind(model_name.trim()).bind(model_name.trim()).fetch_optional(&pool).await {
            Ok(value) => value,
            Err(err) => {
                error!(error = %err, "failed to resolve conversation model");
                return Err(api_error(StatusCode::INTERNAL_SERVER_ERROR, "could not create conversation"));
            }
        }
    } else { None };
    if request.model.is_some() && model_id.is_none() {
        return Err(api_error(StatusCode::BAD_REQUEST, "model is not registered or enabled"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT INTO conversations (id,title,selected_model_id,system_prompt) VALUES (?,?,?,?)"
    ).bind(&id).bind(title.trim()).bind(model_id).bind(request.system_prompt.as_deref())
     .execute(&pool).await;
    if let Err(err) = result {
        error!(error = %err, "failed to create conversation");
        return Err(api_error(StatusCode::INTERNAL_SERVER_ERROR, "could not create conversation"));
    }
    let row = sqlx::query(
        "SELECT id,title,selected_model_id,system_prompt,created_at,updated_at FROM conversations WHERE id=?"
    ).bind(&id).fetch_one(&pool).await;
    match row {
        Ok(row) => {
            use sqlx::Row;
            Ok((StatusCode::CREATED, Json(Conversation {
                id: row.get("id"), title: row.get("title"),
                selected_model_id: row.get("selected_model_id"),
                system_prompt: row.get("system_prompt"),
                created_at: row.get("created_at"), updated_at: row.get("updated_at"),
            })))
        }
        Err(err) => {
            error!(error = %err, "failed to read created conversation");
            Err(api_error(StatusCode::INTERNAL_SERVER_ERROR, "could not create conversation"))
        }
    }
}

async fn list_conversations(
    State(state): State<AppState>,
) -> Result<Json<Vec<Conversation>>, (StatusCode, Json<Value>)> {
    let pool = state.pool.clone();
    if db::require_migrated(&pool).await.is_err() {
        return Err(api_error(StatusCode::SERVICE_UNAVAILABLE, "database schema is not ready"));
    }
    let rows = sqlx::query(
        "SELECT id,title,selected_model_id,system_prompt,created_at,updated_at \
         FROM conversations WHERE archived_at IS NULL ORDER BY updated_at DESC LIMIT 100"
    ).fetch_all(&pool).await.map_err(|err| {
        error!(error = %err, "failed to list conversations");
        api_error(StatusCode::INTERNAL_SERVER_ERROR, "could not list conversations")
    })?;
    use sqlx::Row;
    Ok(Json(rows.into_iter().map(|row| Conversation {
        id: row.get("id"), title: row.get("title"),
        selected_model_id: row.get("selected_model_id"),
        system_prompt: row.get("system_prompt"),
        created_at: row.get("created_at"), updated_at: row.get("updated_at"),
    }).collect()))
}

async fn list_messages(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Result<Json<Vec<StoredMessage>>, (StatusCode, Json<Value>)> {
    let pool = state.pool.clone();
    if db::require_migrated(&pool).await.is_err() {
        return Err(api_error(StatusCode::SERVICE_UNAVAILABLE, "database schema is not ready"));
    }
    let rows = sqlx::query(
        "SELECT id,conversation_id,role,content,sequence_number,status,created_at \
         FROM messages WHERE conversation_id=? ORDER BY sequence_number ASC"
    ).bind(&conversation_id).fetch_all(&pool).await.map_err(|err| {
        error!(error = %err, "failed to list conversation messages");
        api_error(StatusCode::INTERNAL_SERVER_ERROR, "could not list messages")
    })?;
    use sqlx::Row;
    Ok(Json(rows.into_iter().map(|row| StoredMessage {
        id: row.get("id"), conversation_id: row.get("conversation_id"),
        role: row.get("role"), content: row.get("content"),
        sequence_number: row.get("sequence_number"), status: row.get("status"),
        created_at: row.get("created_at"),
    }).collect()))
}

async fn add_message(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
    Json(request): Json<AddMessageRequest>,
) -> Result<(StatusCode, Json<Vec<StoredMessage>>), (StatusCode, Json<Value>)> {
    let pool = state.pool.clone();
    if db::require_migrated(&pool).await.is_err() {
        return Err(api_error(StatusCode::SERVICE_UNAVAILABLE, "database schema is not ready"));
    }
    if request.content.trim().is_empty() || request.content.len() > 100_000 {
        return Err(api_error(StatusCode::BAD_REQUEST, "message content must be between 1 and 100000 bytes"));
    }
    let conversation = sqlx::query(
        "SELECT selected_model_id, system_prompt FROM conversations WHERE id=? AND archived_at IS NULL"
    ).bind(&conversation_id).fetch_optional(&pool).await.map_err(|err| {
        error!(error = %err, "failed to load conversation");
        api_error(StatusCode::INTERNAL_SERVER_ERROR, "could not load conversation")
    })?.ok_or_else(|| api_error(StatusCode::NOT_FOUND, "conversation not found"))?;
    use sqlx::Row;
    let model_id: Option<String> = conversation.get("selected_model_id");
    let system_prompt: Option<String> = conversation.get("system_prompt");
    let model_id = model_id.ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "conversation has no selected model"))?;
    let model_name: String = sqlx::query_scalar("SELECT external_model_id FROM models WHERE id=?")
        .bind(model_id).fetch_one(&pool).await.map_err(|err| {
            error!(error = %err, "failed to resolve conversation model");
            api_error(StatusCode::BAD_REQUEST, "conversation model is unavailable")
        })?;

    let mut messages: Vec<crate::inference::ChatMessage> = Vec::new();
    if let Some(system) = system_prompt.filter(|v| !v.trim().is_empty()) {
        messages.push(crate::inference::ChatMessage { role: "system".into(), content: system });
    }
    let existing = sqlx::query(
        "SELECT role,content FROM messages WHERE conversation_id=? AND status='completed' ORDER BY sequence_number ASC"
    ).bind(&conversation_id).fetch_all(&pool).await.map_err(|err| {
        error!(error = %err, "failed to load message history");
        api_error(StatusCode::INTERNAL_SERVER_ERROR, "could not load message history")
    })?;
    for row in existing {
        let role: String = row.get("role");
        let content: String = row.get("content");
        if ["user","assistant","system"].contains(&role.as_str()) {
            messages.push(crate::inference::ChatMessage { role, content });
        }
    }
    messages.push(crate::inference::ChatMessage { role: "user".into(), content: request.content.clone() });

    let max_sequence: Option<i64> = sqlx::query_scalar("SELECT MAX(sequence_number) FROM messages WHERE conversation_id=?")
        .bind(&conversation_id).fetch_one(&pool).await.map_err(|err| {
            error!(error = %err, "failed to determine message sequence");
            api_error(StatusCode::INTERNAL_SERVER_ERROR, "could not save message")
        })?;
    let user_sequence = max_sequence.map(|n| n + 1).unwrap_or(0);
    let user_id = uuid::Uuid::new_v4().to_string();
    if let Err(err) = sqlx::query(
        "INSERT INTO messages (id,conversation_id,role,content,sequence_number,status) VALUES (?,?, 'user', ?, ?, 'completed')"
    ).bind(&user_id).bind(&conversation_id).bind(&request.content).bind(user_sequence).execute(&pool).await {
        error!(error = %err, "failed to persist user message");
        return Err(api_error(StatusCode::INTERNAL_SERVER_ERROR, "could not save message"));
    }

    let answer = match crate::inference::complete_messages_by_name(&pool, &model_name, &messages).await {
        Ok(answer) => answer,
        Err(err) => {
            error!(error = %err, "conversation inference failed");
            return Err(api_error(StatusCode::BAD_GATEWAY, "model request failed; user message was saved"));
        }
    };
    let assistant_id = uuid::Uuid::new_v4().to_string();
    if let Err(err) = sqlx::query(
        "INSERT INTO messages (id,conversation_id,role,content,sequence_number,status,completed_at) \
         VALUES (?,?, 'assistant', ?, ?, 'completed', strftime('%Y-%m-%dT%H:%M:%fZ','now'))"
    ).bind(&assistant_id).bind(&conversation_id).bind(answer).bind(user_sequence + 1).execute(&pool).await {
        error!(error = %err, "failed to persist assistant response");
        return Err(api_error(StatusCode::INTERNAL_SERVER_ERROR, "model responded but the assistant message could not be saved"));
    }
    let _ = sqlx::query("UPDATE conversations SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?")
        .bind(&conversation_id).execute(&pool).await;
    let rows = sqlx::query(
        "SELECT id,conversation_id,role,content,sequence_number,status,created_at FROM messages \
         WHERE conversation_id=? AND sequence_number IN (?,?) ORDER BY sequence_number"
    ).bind(&conversation_id).bind(user_sequence).bind(user_sequence + 1).fetch_all(&pool).await.map_err(|err| {
        error!(error = %err, "failed to return saved messages");
        api_error(StatusCode::INTERNAL_SERVER_ERROR, "messages saved but could not be returned")
    })?;
    let output = rows.into_iter().map(|row| StoredMessage {
        id: row.get("id"), conversation_id: row.get("conversation_id"),
        role: row.get("role"), content: row.get("content"),
        sequence_number: row.get("sequence_number"), status: row.get("status"),
        created_at: row.get("created_at"),
    }).collect();
    Ok((StatusCode::CREATED, Json(output)))
}

fn api_error(status: StatusCode, message: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"error":{"message":message}})))
}

async fn live() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn ready(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let pool = state.pool.clone();
    match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&pool)
        .await
    {
        Ok(_) => Ok(Json(json!({ "status": "ready", "database": "ok" }))),
        Err(err) => {
            error!(error = %err, "readiness check failed");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

async fn models(
    State(state): State<AppState>,
) -> Result<Json<Vec<registry::ModelListItem>>, StatusCode> {
    let pool = state.pool.clone();
    if let Err(err) = db::require_migrated(&pool).await {
        error!(error = %err, "model endpoint unavailable because schema is not ready");
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    registry::list_models(&pool).await.map(Json).map_err(|err| {
        error!(error = %err, "failed to list models");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    async fn test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database should open")
    }


    #[test]
    fn provider_stream_chunk_is_translated_to_openai_chunk_shape() {
        let payload = translate_provider_chunk(
            r#"{"choices":[{"index":0,"delta":{"content":"hello"},"finish_reason":null}]}"#,
            "request-123",
            "tiny-model",
            123,
        ).expect("provider delta should translate");
        let parsed: Value = serde_json::from_str(&payload).expect("translated payload should be JSON");
        assert_eq!(parsed["object"], "chat.completion.chunk");
        assert_eq!(parsed["choices"][0]["delta"]["content"], "hello");
        assert_eq!(parsed["model"], "tiny-model");
    }

    #[tokio::test]
    async fn cancelling_unknown_request_returns_not_found() {
        let response = router(test_pool().await).oneshot(
            Request::builder().method("POST").uri("/v1/requests/missing/cancel")
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn live_endpoint_reports_ok() {
        let response = router(test_pool().await)
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();
        let value: Value = serde_json::from_slice(&body).expect("response should be JSON");
        assert_eq!(value["status"], "ok");
    }

    #[tokio::test]
    async fn model_endpoint_requires_schema_migrations() {
        let response = router(test_pool().await)
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }


    #[tokio::test]
    async fn conversation_can_be_created_and_listed() {
        let pool = test_pool().await;
        db::migrate(&pool).await.expect("migration should apply");
        let app = router(pool);
        let response = app.clone().oneshot(
            Request::builder().method("POST").uri("/v1/conversations")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Test conversation"}"#)).unwrap()
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let created: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(created["title"], "Test conversation");
        assert!(created["id"].as_str().is_some());

        let response = app.oneshot(
            Request::builder().uri("/v1/conversations").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let list: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn conversation_title_is_validated() {
        let pool = test_pool().await;
        db::migrate(&pool).await.expect("migration should apply");
        let response = router(pool).oneshot(
            Request::builder().method("POST").uri("/v1/conversations")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"   "}"#)).unwrap()
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn model_endpoint_lists_models_after_migration() {
        let pool = test_pool().await;
        db::migrate(&pool).await.expect("migration should apply");
        let response = router(pool)
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();
        let value: Value = serde_json::from_slice(&body).expect("response should be JSON");
        assert_eq!(value, json!([]));
    }
}
