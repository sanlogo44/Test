use anyhow::{bail, Context, Result};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::str::FromStr;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/sqlite");

pub async fn connect(database_url: &str) -> Result<SqlitePool> {
    if !database_url.starts_with("sqlite:") {
        bail!("Phase 1 supports SQLite only; use a URL such as sqlite://llm-platform.db");
    }

    let options = SqliteConnectOptions::from_str(database_url)
        .with_context(|| format!("invalid SQLite database URL: {database_url}"))?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .context("failed to open SQLite database")
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    MIGRATOR
        .run(pool)
        .await
        .context("database migration failed")?;
    Ok(())
}

pub async fn status(pool: &SqlitePool) -> Result<()> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;

    if exists == 0 {
        println!("Database has not been migrated. Run: llm-platform db migrate");
        return Ok(());
    }

    let rows =
        sqlx::query("SELECT version, description, success FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await?;

    if rows.is_empty() {
        println!("No migrations have been applied. Run: llm-platform db migrate");
        return Ok(());
    }

    println!("Applied migrations:");
    for row in rows {
        let version: i64 = row.try_get("version")?;
        let description: String = row.try_get("description")?;
        let success: bool = row.try_get("success")?;
        println!(
            "  {version}: {description} [{}]",
            if success { "ok" } else { "FAILED" }
        );
    }
    Ok(())
}

pub async fn require_migrated(pool: &SqlitePool) -> Result<()> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    if exists == 0 {
        bail!("database is not initialized; run `llm-platform db migrate` first");
    }

    let failed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE success = 0")
        .fetch_one(pool)
        .await?;
    if failed > 0 {
        bail!("database has a failed migration; inspect the database before continuing");
    }

    let applied_versions: Vec<i64> = sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations WHERE success = 1 ORDER BY version",
    )
    .fetch_all(pool)
    .await?;
    let expected_versions: Vec<i64> = MIGRATOR.iter().map(|migration| migration.version).collect();
    if applied_versions != expected_versions {
        bail!(
            "database migration versions do not match this application; run `llm-platform db migrate` and inspect any migration errors"
        );
    }
    Ok(())
}
