use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::json;
use sqlx::{FromRow, SqlitePool};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct ModelListItem {
    pub id: String,
    pub display_name: String,
    pub external_model_id: String,
    pub provider_name: String,
    pub status: String,
    pub context_window: Option<i64>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ProviderListItem {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub endpoint: Option<String>,
    pub enabled: bool,
}

pub async fn add_provider(
    pool: &SqlitePool,
    name: &str,
    provider_type: &str,
    endpoint: Option<&str>,
) -> Result<String> {
    if name.trim().is_empty() || provider_type.trim().is_empty() {
        bail!("provider name and provider type must not be empty");
    }

    let normalized_endpoint = validate_endpoint(endpoint)?;
    let id = Uuid::new_v4().to_string();
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO model_providers (id, name, provider_type, endpoint) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name.trim())
    .bind(provider_type.trim())
    .bind(normalized_endpoint.as_deref())
    .execute(&mut *transaction)
    .await
    .with_context(|| format!("could not register provider `{name}`; names must be unique"))?;
    sqlx::query(
        "INSERT INTO audit_events (id, action, resource_type, resource_id, outcome, metadata_json) \
         VALUES (?, 'provider.registered', 'model_provider', ?, 'success', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&id)
    .bind(json!({ "name": name.trim(), "provider_type": provider_type.trim() }).to_string())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(id)
}

pub async fn list_providers(pool: &SqlitePool) -> Result<Vec<ProviderListItem>> {
    let rows = sqlx::query_as::<_, ProviderListItem>(
        "SELECT id, name, provider_type, endpoint, enabled \
         FROM model_providers ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn register_model(
    pool: &SqlitePool,
    provider_name: &str,
    external_id: &str,
    display_name: &str,
    local_path: Option<&str>,
    context_window: Option<i64>,
) -> Result<String> {
    if external_id.trim().is_empty() || display_name.trim().is_empty() {
        bail!("external model ID and display name must not be empty");
    }
    if context_window.is_some_and(|value| value <= 0) {
        bail!("context window must be a positive integer");
    }

    let provider_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM model_providers WHERE name = ? AND enabled = 1")
            .bind(provider_name.trim())
            .fetch_optional(pool)
            .await?;
    let provider_id = provider_id
        .with_context(|| format!("enabled provider `{provider_name}` was not found"))?;

    let normalized_path = local_path.map(str::trim).filter(|value| !value.is_empty());
    let id = Uuid::new_v4().to_string();
    let source = if normalized_path.is_some() {
        "local"
    } else {
        "remote"
    };
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO models (id, provider_id, external_model_id, display_name, source, local_path, context_window, status) \
         VALUES (?, ?, ?, ?, ?, ?, ?, 'registered')",
    )
    .bind(&id)
    .bind(provider_id)
    .bind(external_id.trim())
    .bind(display_name.trim())
    .bind(source)
    .bind(normalized_path)
    .bind(context_window)
    .execute(&mut *transaction)
    .await
    .with_context(|| {
        format!("could not register model `{external_id}`; IDs must be unique per provider")
    })?;
    sqlx::query(
        "INSERT INTO audit_events (id, action, resource_type, resource_id, outcome, metadata_json) \
         VALUES (?, 'model.registered', 'model', ?, 'success', ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&id)
    .bind(json!({ "external_model_id": external_id.trim(), "display_name": display_name.trim() }).to_string())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(id)
}

pub async fn list_models(pool: &SqlitePool) -> Result<Vec<ModelListItem>> {
    let rows = sqlx::query_as::<_, ModelListItem>(
        "SELECT m.id, m.display_name, m.external_model_id, p.name AS provider_name, m.status, m.context_window \
         FROM models m JOIN model_providers p ON p.id = m.provider_id \
         WHERE p.enabled = 1 ORDER BY p.name COLLATE NOCASE, m.display_name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

fn validate_endpoint(endpoint: Option<&str>) -> Result<Option<String>> {
    let Some(endpoint) = endpoint.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = Url::parse(endpoint).context("provider endpoint must be an absolute HTTP(S) URL")?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        bail!("provider endpoint scheme must be http or https");
    }
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        bail!("provider endpoint must not contain user info, a query string, or a fragment; keep credentials in a secret store");
    }
    Ok(Some(endpoint.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn provider_and_model_registry_roundtrip() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database should open");
        db::migrate(&pool).await.expect("migration should apply");

        add_provider(
            &pool,
            "local",
            "local",
            Some("http://127.0.0.1:8080/v1"),
        )
        .await
        .expect("provider should register");
        register_model(
            &pool,
            "local",
            "tiny-q4",
            "Tiny Q4",
            Some("/models/tiny.gguf"),
            Some(4096),
        )
        .await
        .expect("model should register");

        let models = list_models(&pool).await.expect("models should list");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].display_name, "Tiny Q4");
        assert_eq!(models[0].provider_name, "local");
        assert_eq!(models[0].status, "registered");

        let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_events")
            .fetch_one(&pool)
            .await
            .expect("audit records should be queryable");
        assert_eq!(audit_count, 2);
    }

    #[test]
    fn endpoint_rejects_credentials_in_query_and_non_http_schemes() {
        assert!(validate_endpoint(Some("https://user:password@example.com/v1")).is_err());
        assert!(validate_endpoint(Some("https://example.com/v1?api_key=secret")).is_err());
        assert!(validate_endpoint(Some("file:///tmp/model")).is_err());
        assert!(validate_endpoint(Some("http://127.0.0.1:8080/v1")).is_ok());
    }
}
