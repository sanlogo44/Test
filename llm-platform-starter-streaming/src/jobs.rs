//! Persistent SQLite-backed job queue primitives.
//!
//! A lease is a temporary claim, not a guarantee that work ran exactly once.
//! Handlers should be idempotent because a worker can crash after performing
//! an external side effect but before acknowledging completion.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Job {
    pub id: String,
    pub job_type: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub priority: i64,
    pub attempt_count: i64,
    pub maximum_attempts: i64,
    pub scheduled_at: String,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<String>,
    pub result_reference: Option<String>,
    pub error_code: Option<String>,
}

/// Add a job. Reusing an idempotency key returns the existing job.
pub async fn enqueue(
    pool: &SqlitePool,
    job_type: &str,
    payload: &serde_json::Value,
    priority: i64,
    maximum_attempts: i64,
    idempotency_key: Option<&str>,
) -> Result<Job> {
    anyhow::ensure!(!job_type.trim().is_empty(), "job type must not be empty");
    anyhow::ensure!(maximum_attempts > 0, "maximum_attempts must be positive");
    let id = Uuid::new_v4().to_string();
    let payload_json = serde_json::to_string(payload)?;

    let inserted = sqlx::query(
        "INSERT INTO jobs (id, job_type, payload_json, priority, maximum_attempts, idempotency_key) \
         VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(idempotency_key) DO NOTHING",
    )
    .bind(&id)
    .bind(job_type)
    .bind(payload_json)
    .bind(priority)
    .bind(maximum_attempts)
    .bind(idempotency_key)
    .execute(pool)
    .await
    .context("failed to enqueue job")?;

    let selected_id = if inserted.rows_affected() == 1 {
        id
    } else {
        sqlx::query_scalar::<_, String>("SELECT id FROM jobs WHERE idempotency_key = ?")
            .bind(idempotency_key)
            .fetch_one(pool)
            .await
            .context("idempotency key conflicted but existing job was not found")?
    };
    get(pool, &selected_id).await?.context("enqueued job not found")
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Job>> {
    let row = sqlx::query(
        "SELECT id, job_type, payload_json, status, priority, attempt_count, maximum_attempts, scheduled_at, lease_owner, lease_expires_at, result_reference, error_code FROM jobs WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    row.map(|row| -> Result<Job> {
        let payload: String = row.try_get("payload_json")?;
        Ok(Job {
            id: row.try_get("id")?,
            job_type: row.try_get("job_type")?,
            payload: serde_json::from_str(&payload)?,
            status: row.try_get("status")?,
            priority: row.try_get("priority")?,
            attempt_count: row.try_get("attempt_count")?,
            maximum_attempts: row.try_get("maximum_attempts")?,
            scheduled_at: row.try_get("scheduled_at")?,
            lease_owner: row.try_get("lease_owner")?,
            lease_expires_at: row.try_get("lease_expires_at")?,
            result_reference: row.try_get("result_reference")?,
            error_code: row.try_get("error_code")?,
        })
    })
    .transpose()
    .context("failed to decode job row")
}

/// List jobs with bounded limits and an optional validated status filter.
pub async fn list(pool: &SqlitePool, status: Option<&str>, limit: i64) -> Result<Vec<Job>> {
    anyhow::ensure!((1..=500).contains(&limit), "limit must be between 1 and 500");
    if let Some(status) = status {
        anyhow::ensure!(["queued", "running", "completed", "failed", "cancelled", "dead_letter"].contains(&status), "unsupported job status filter");
    }
    let rows = sqlx::query("SELECT id,job_type,payload_json,status,priority,attempt_count,maximum_attempts,scheduled_at,lease_owner,lease_expires_at,result_reference,error_code FROM jobs WHERE (? IS NULL OR status=?) ORDER BY priority DESC,scheduled_at ASC,created_at ASC LIMIT ?")
        .bind(status).bind(status).bind(limit).fetch_all(pool).await?;
    rows.into_iter().map(|row| -> Result<Job> {
        let payload: String = row.try_get("payload_json")?;
        Ok(Job {
            id: row.try_get("id")?, job_type: row.try_get("job_type")?,
            payload: serde_json::from_str(&payload)?, status: row.try_get("status")?,
            priority: row.try_get("priority")?, attempt_count: row.try_get("attempt_count")?,
            maximum_attempts: row.try_get("maximum_attempts")?, scheduled_at: row.try_get("scheduled_at")?,
            lease_owner: row.try_get("lease_owner")?, lease_expires_at: row.try_get("lease_expires_at")?,
            result_reference: row.try_get("result_reference")?, error_code: row.try_get("error_code")?,
        })
    }).collect()
}

/// Cancel work only while it is queued. Running jobs remain owned by their worker.
pub async fn cancel(pool: &SqlitePool, id: &str) -> Result<bool> {
    let result = sqlx::query("UPDATE jobs SET status='cancelled',completed_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND status='queued'")
        .bind(id).execute(pool).await?;
    Ok(result.rows_affected() == 1)
}

/// Atomically claim the highest-priority due job and record its attempt.
pub async fn claim(pool: &SqlitePool, worker_id: &str, lease_seconds: i64) -> Result<Option<Job>> {
    anyhow::ensure!(!worker_id.trim().is_empty(), "worker_id must not be empty");
    anyhow::ensure!(lease_seconds > 0, "lease_seconds must be positive");
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "UPDATE jobs SET status = 'running', attempt_count = attempt_count + 1, \
         started_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
         lease_owner = ?, lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ? || ' seconds'), \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         WHERE id = (SELECT id FROM jobs WHERE status = 'queued' AND attempt_count < maximum_attempts \
         AND scheduled_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         ORDER BY priority DESC, scheduled_at ASC, created_at ASC LIMIT 1) \
         RETURNING id, job_type, payload_json, status, attempt_count, maximum_attempts, lease_owner, lease_expires_at",
    )
    .bind(worker_id)
    .bind(lease_seconds)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(row) = row {
        let id: String = row.try_get("id")?;
        let attempt: i64 = row.try_get("attempt_count")?;
        sqlx::query(
            "INSERT INTO job_attempts (id, job_id, attempt_number, worker_id, status) VALUES (?, ?, ?, ?, 'running')",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&id)
        .bind(attempt)
        .bind(worker_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        get(pool, &id).await
    } else {
        tx.commit().await?;
        Ok(None)
    }
}

pub async fn complete(pool: &SqlitePool, job_id: &str, worker_id: &str, result_reference: Option<&str>) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE jobs SET status = 'completed', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
         lease_owner = NULL, lease_expires_at = NULL, result_reference = ?, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND status = 'running' AND lease_owner = ?",
    )
    .bind(result_reference)
    .bind(job_id)
    .bind(worker_id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 1 {
        sqlx::query("UPDATE job_attempts SET status = 'completed', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE job_id = ? AND status = 'running'")
            .bind(job_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(result.rows_affected() == 1)
}

/// Fail a claimed job. It is re-queued if attempts remain; otherwise dead-lettered.
pub async fn fail(pool: &SqlitePool, job_id: &str, worker_id: &str, error_code: &str, error_summary: &str) -> Result<bool> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT attempt_count, maximum_attempts FROM jobs WHERE id = ? AND status = 'running' AND lease_owner = ?",
    ).bind(job_id).bind(worker_id).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { tx.commit().await?; return Ok(false); };
    let attempt_count: i64 = row.try_get("attempt_count")?;
    let maximum_attempts: i64 = row.try_get("maximum_attempts")?;
    let status = if attempt_count >= maximum_attempts { "dead_letter" } else { "queued" };
    // Exponential retry delay: 2, 4, 8, ... up to five minutes.
    let retry_delay_seconds = 2_i64.pow(attempt_count.min(8) as u32).min(300);
    sqlx::query(
        "UPDATE jobs SET status = ?, error_code = ?, lease_owner = NULL, lease_expires_at = NULL, \
         completed_at = CASE WHEN ? = 'dead_letter' THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE NULL END, \
         scheduled_at = CASE WHEN ? = 'queued' THEN strftime('%Y-%m-%dT%H:%M:%fZ','now', '+' || ? || ' seconds') ELSE scheduled_at END, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    ).bind(status).bind(error_code).bind(status).bind(status).bind(retry_delay_seconds).bind(job_id).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE job_attempts SET status = 'failed', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), error_summary = ? WHERE job_id = ? AND status = 'running'",
    ).bind(error_summary).bind(job_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}

/// Recover expired leases after a crash. Exhausted jobs are dead-lettered.
pub async fn recover_expired_leases(pool: &SqlitePool) -> Result<u64> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE job_attempts SET status = 'abandoned', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
         error_summary = 'worker lease expired' WHERE status = 'running' AND job_id IN \
         (SELECT id FROM jobs WHERE status = 'running' AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
    ).execute(&mut *tx).await?;
    let result = sqlx::query(
        "UPDATE jobs SET status = CASE WHEN attempt_count >= maximum_attempts THEN 'dead_letter' ELSE 'queued' END, \
         lease_owner = NULL, lease_expires_at = NULL, \
         completed_at = CASE WHEN attempt_count >= maximum_attempts THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE NULL END, \
         error_code = 'lease_expired', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         WHERE status = 'running' AND lease_expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    ).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(result.rows_affected())
}

/// Execute a single job. chat_completion invokes the registered inference adapter.
pub async fn execute_next(pool: &SqlitePool, worker_id: &str, lease_seconds: i64) -> Result<Option<Job>> {
    // The OpenAI-compatible adapter has a 180-second request timeout.
    anyhow::ensure!(lease_seconds > 180, "worker lease must be longer than the 180-second provider timeout");
    let Some(job) = claim(pool, worker_id, lease_seconds).await? else { return Ok(None); };
    let execution: Result<String> = async {
        match job.job_type.as_str() {
            "chat_completion" => {
                let model = job.payload.get("model").and_then(serde_json::Value::as_str)
                    .context("chat_completion payload requires model")?;
                let value = job.payload.get("messages").cloned()
                    .context("chat_completion payload requires messages")?;
                let messages: Vec<crate::inference::ChatMessage> = serde_json::from_value(value)
                    .context("chat_completion messages are invalid")?;
                let content = crate::inference::complete_messages_by_name(pool, model, &messages).await?;
                anyhow::ensure!(content.len() <= 1_000_000, "completion exceeds the 1 MB job-result limit");
                Ok(serde_json::json!({"model":model,"content":content}).to_string())
            }
            other => anyhow::bail!("unsupported job type {other}"),
        }
    }.await;
    match execution {
        Ok(result) => {
            anyhow::ensure!(complete(pool,&job.id,worker_id,Some(&result)).await?, "job lease was lost before completion");
        }
        Err(error) => {
            tracing::error!(job_id=%job.id,job_type=%job.job_type,error=%error,"job execution failed");
            anyhow::ensure!(fail(pool,&job.id,worker_id,"job_execution_failed","job execution failed").await?, "job lease was lost before failure");
        }
    }
    get(pool,&job.id).await?.context("processed job disappeared")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.expect("open test database");
        db::migrate(&pool).await.expect("apply schema");
        pool
    }

    #[tokio::test]
    async fn enqueue_is_idempotent_and_claim_completes() {
        let pool = pool().await;
        let payload = serde_json::json!({"prompt":"hello"});
        let first = enqueue(&pool, "test", &payload, 5, 2, Some("request-1")).await.unwrap();
        let second = enqueue(&pool, "test", &payload, 5, 2, Some("request-1")).await.unwrap();
        assert_eq!(first.id, second.id);
        let claimed = claim(&pool, "worker-a", 60).await.unwrap().unwrap();
        assert_eq!(claimed.id, first.id);
        assert_eq!(claimed.attempt_count, 1);
        assert!(claim(&pool, "worker-b", 60).await.unwrap().is_none());
        assert!(complete(&pool, &first.id, "wrong-worker", None).await.unwrap() == false);
        assert!(complete(&pool, &first.id, "worker-a", Some("result-1")).await.unwrap());
        assert_eq!(get(&pool, &first.id).await.unwrap().unwrap().status, "completed");
    }

    #[tokio::test]
    async fn queued_cancellation_and_job_listing_work() {
        let pool=pool().await;
        let a=enqueue(&pool,"test",&serde_json::json!({}),2,2,None).await.unwrap();
        let b=enqueue(&pool,"test",&serde_json::json!({}),0,2,None).await.unwrap();
        assert_eq!(list(&pool,Some("queued"),10).await.unwrap().len(),2);
        assert!(cancel(&pool,&a.id).await.unwrap());
        assert!(!cancel(&pool,&a.id).await.unwrap());
        assert_eq!(get(&pool,&a.id).await.unwrap().unwrap().status,"cancelled");
        assert_eq!(get(&pool,&b.id).await.unwrap().unwrap().status,"queued");
        assert!(list(&pool,Some("bogus"),10).await.is_err());
        assert!(list(&pool,None,0).await.is_err());
    }

    #[tokio::test]
    async fn unsupported_job_type_becomes_dead_letter() {
        let pool=pool().await;
        let job=enqueue(&pool,"unknown",&serde_json::json!({}),0,1,None).await.unwrap();
        let result=execute_next(&pool,"worker-a",240).await.unwrap().unwrap();
        assert_eq!(result.id,job.id);
        assert_eq!(result.status,"dead_letter");
        assert_eq!(result.error_code.as_deref(),Some("job_execution_failed"));
    }

    #[tokio::test]
    async fn failed_jobs_retry_then_dead_letter() {
        let pool = pool().await;
        let job = enqueue(&pool, "test", &serde_json::json!({}), 0, 1, None).await.unwrap();
        let claimed = claim(&pool, "worker-a", 60).await.unwrap().unwrap();
        assert!(fail(&pool, &claimed.id, "worker-a", "test_error", "failed").await.unwrap());
        assert_eq!(get(&pool, &job.id).await.unwrap().unwrap().status, "dead_letter");
        assert!(claim(&pool, "worker-b", 60).await.unwrap().is_none());
    }
}
