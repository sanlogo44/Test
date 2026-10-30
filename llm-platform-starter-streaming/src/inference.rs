//! OpenAI-compatible chat adapter. Provider endpoints are only contacted on explicit inference calls.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse { choices: Vec<Choice> }
#[derive(Debug, Deserialize)]
struct Choice { message: ChatMessage }

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
}

pub async fn complete_by_name(
    pool: &SqlitePool,
    model_name: &str,
    system_prompt: Option<&str>,
    prompt: &str,
) -> Result<String> {
    anyhow::ensure!(!prompt.trim().is_empty(), "prompt must not be empty");
    let mut messages = Vec::new();
    if let Some(system) = system_prompt.filter(|v| !v.trim().is_empty()) {
        messages.push(ChatMessage { role: "system".into(), content: system.into() });
    }
    messages.push(ChatMessage { role: "user".into(), content: prompt.into() });
    complete_messages_by_name(pool, model_name, &messages).await
}

/// Sends the complete conversation, preserving prior user/assistant turns.
pub async fn complete_messages_by_name(
    pool: &SqlitePool,
    model_name: &str,
    messages: &[ChatMessage],
) -> Result<String> {
    anyhow::ensure!(!model_name.trim().is_empty(), "model must not be empty");
    validate_messages(messages)?;

    let row = sqlx::query(
        "SELECT m.external_model_id, p.endpoint, p.provider_type FROM models m \
         JOIN model_providers p ON p.id = m.provider_id \
         WHERE p.enabled = 1 AND m.status != 'disabled' \
         AND (m.external_model_id = ? COLLATE NOCASE OR m.display_name = ? COLLATE NOCASE) \
         ORDER BY CASE WHEN m.external_model_id = ? COLLATE NOCASE THEN 0 ELSE 1 END LIMIT 1",
    )
    .bind(model_name.trim()).bind(model_name.trim()).bind(model_name.trim())
    .fetch_optional(pool).await?
    .with_context(|| format!("no enabled registered model matches `{model_name}`"))?;
    let external_id: String = row.try_get("external_model_id")?;
    let endpoint: Option<String> = row.try_get("endpoint")?;
    let provider_type: String = row.try_get("provider_type")?;
    anyhow::ensure!(
        provider_type.eq_ignore_ascii_case("openai-compatible")
            || provider_type.eq_ignore_ascii_case("openai")
            || provider_type.eq_ignore_ascii_case("local"),
        "provider type is not supported by the OpenAI-compatible adapter"
    );
    let endpoint = endpoint.context("provider has no endpoint; configure an OpenAI-compatible base URL")?;
    let endpoint = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let request = ChatRequest { model: &external_id, messages, stream: false };
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()?;
    let mut call = client.post(endpoint).json(&request);
    if let Ok(key) = std::env::var("LLM_PLATFORM_API_KEY") {
        if !key.trim().is_empty() { call = call.bearer_auth(key); }
    }
    let response = call.send().await.context("provider request failed")?;
    let status = response.status();
    if !status.is_success() { bail!("provider returned HTTP {status}"); }
    let decoded: ChatResponse = response.json().await.context("provider returned an invalid chat-completion response")?;
    decoded.choices.into_iter().next().map(|c| c.message.content)
        .filter(|s| !s.is_empty()).context("provider returned no completion choices")
}


/// Validates common chat messages before any provider is contacted.
fn validate_messages(messages: &[ChatMessage]) -> Result<()> {
    anyhow::ensure!(!messages.is_empty(), "messages must not be empty");
    anyhow::ensure!(messages.iter().any(|m| m.role == "user"), "at least one user message is required");
    for message in messages {
        anyhow::ensure!(
            ["system", "user", "assistant", "tool"].contains(&message.role.as_str()),
            "unsupported message role"
        );
        anyhow::ensure!(!message.content.trim().is_empty(), "message content must not be empty");
    }
    Ok(())
}

/// Opens an OpenAI-compatible SSE streaming response. The caller owns the response
/// and can stop polling it to cancel the underlying HTTP request.
pub async fn open_stream_by_name(
    pool: &SqlitePool,
    model_name: &str,
    messages: &[ChatMessage],
) -> Result<reqwest::Response> {
    anyhow::ensure!(!model_name.trim().is_empty(), "model must not be empty");
    validate_messages(messages)?;

    let row = sqlx::query(
        "SELECT m.external_model_id, p.endpoint, p.provider_type FROM models m \
         JOIN model_providers p ON p.id = m.provider_id \
         WHERE p.enabled = 1 AND m.status != 'disabled' \
         AND (m.external_model_id = ? COLLATE NOCASE OR m.display_name = ? COLLATE NOCASE) \
         ORDER BY CASE WHEN m.external_model_id = ? COLLATE NOCASE THEN 0 ELSE 1 END LIMIT 1",
    )
    .bind(model_name.trim()).bind(model_name.trim()).bind(model_name.trim())
    .fetch_optional(pool).await?
    .with_context(|| format!("no enabled registered model matches `{model_name}`"))?;

    let external_id: String = row.try_get("external_model_id")?;
    let endpoint: Option<String> = row.try_get("endpoint")?;
    let provider_type: String = row.try_get("provider_type")?;
    anyhow::ensure!(
        provider_type.eq_ignore_ascii_case("openai-compatible")
            || provider_type.eq_ignore_ascii_case("openai")
            || provider_type.eq_ignore_ascii_case("local"),
        "provider type is not supported by the OpenAI-compatible adapter"
    );
    let endpoint = endpoint.context("provider has no endpoint; configure an OpenAI-compatible base URL")?;
    let endpoint = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let request = ChatRequest { model: &external_id, messages, stream: true };
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()?;
    let mut call = client.post(endpoint).json(&request);
    if let Ok(key) = std::env::var("LLM_PLATFORM_API_KEY") {
        if !key.trim().is_empty() { call = call.bearer_auth(key); }
    }
    let response = call.send().await.context("provider streaming request failed")?;
    let status = response.status();
    if !status.is_success() { bail!("provider returned HTTP {status}"); }
    Ok(response)
}

#[cfg(test)]
mod stream_tests {
    use super::*;

    #[test]
    fn validates_full_multi_turn_history() {
        let messages = vec![
            ChatMessage { role: "system".into(), content: "Be concise".into() },
            ChatMessage { role: "user".into(), content: "Hi".into() },
            ChatMessage { role: "assistant".into(), content: "Hello".into() },
            ChatMessage { role: "user".into(), content: "Again".into() },
        ];
        assert!(validate_messages(&messages).is_ok());
    }

    #[test]
    fn rejects_unknown_chat_roles() {
        let messages = vec![ChatMessage { role: "root".into(), content: "Hi".into() }];
        assert!(validate_messages(&messages).is_err());
    }
}
