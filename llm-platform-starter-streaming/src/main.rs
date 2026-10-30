mod api;
mod cli;
mod db;
mod inference;
mod jobs;
mod prompt;
mod registry;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands, DbCommands, JobCommands, ModelCommands, ProviderCommands};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let pool = db::connect(&cli.database_url).await?;

    match cli.command {
        Commands::Db {
            command: DbCommands::Migrate,
        } => {
            db::migrate(&pool).await?;
            println!("Database migrations applied successfully.");
        }
        Commands::Db {
            command: DbCommands::Status,
        } => db::status(&pool).await?,
        Commands::Providers { command } => {
            db::require_migrated(&pool).await?;
            match command {
                ProviderCommands::Add {
                    name,
                    provider_type,
                    endpoint,
                } => {
                    let id =
                        registry::add_provider(&pool, &name, &provider_type, endpoint.as_deref())
                            .await?;
                    println!("Provider registered: {name} ({id})");
                    println!(
                        "Note: credentials are intentionally not accepted or stored by this command."
                    );
                }
                ProviderCommands::List => {
                    let providers = registry::list_providers(&pool).await?;
                    if providers.is_empty() {
                        println!("No providers registered.");
                    }
                    for provider in providers {
                        println!(
                            "{}\t{}\t{}\t{}",
                            provider.name,
                            provider.provider_type,
                            provider.endpoint.as_deref().unwrap_or("(no endpoint)"),
                            if provider.enabled {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        );
                    }
                }
            }
        }
        Commands::Models { command } => {
            db::require_migrated(&pool).await?;
            match command {
                ModelCommands::Register {
                    provider,
                    external_id,
                    display_name,
                    local_path,
                    context_window,
                } => {
                    let id = registry::register_model(
                        &pool,
                        &provider,
                        &external_id,
                        &display_name,
                        local_path.as_deref(),
                        context_window,
                    )
                    .await?;
                    println!("Model registered: {display_name} ({id})");
                    println!(
                        "Registration does not prove runtime compatibility or model availability."
                    );
                }
                ModelCommands::List => {
                    let models = registry::list_models(&pool).await?;
                    if models.is_empty() {
                        println!("No models registered.");
                    }
                    for model in models {
                        println!(
                            "{}\t{}\tprovider={}\tstatus={}\tcontext={}",
                            model.display_name,
                            model.external_model_id,
                            model.provider_name,
                            model.status,
                            model
                                .context_window
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        );
                    }
                }
            }
        }

        Commands::Jobs { command } => {
            db::require_migrated(&pool).await?;
            match command {
                JobCommands::Enqueue {
                    job_type,
                    payload,
                    priority,
                    maximum_attempts,
                    idempotency_key,
                } => {
                    let value: serde_json::Value =
                        serde_json::from_str(&payload).context("job payload must be valid JSON")?;
                    let job = jobs::enqueue(
                        &pool,
                        &job_type,
                        &value,
                        priority,
                        maximum_attempts,
                        idempotency_key.as_deref(),
                    )
                    .await?;
                    println!("{}", serde_json::to_string_pretty(&job)?);
                }
                JobCommands::List { status, limit } => {
                    for job in jobs::list(&pool, status.as_deref(), limit).await? {
                        println!("{}", serde_json::to_string(&job)?);
                    }
                }
                JobCommands::Inspect { id } => match jobs::get(&pool, &id).await? {
                    Some(job) => println!("{}", serde_json::to_string_pretty(&job)?),
                    None => anyhow::bail!("job {id} was not found"),
                },
                JobCommands::Cancel { id } => {
                    if jobs::cancel(&pool, &id).await? {
                        println!("Job {id} cancelled.");
                    } else {
                        println!("Job {id} was not found or is no longer queued.");
                    }
                }
                JobCommands::Recover => {
                    println!(
                        "Recovered {} expired job lease(s).",
                        jobs::recover_expired_leases(&pool).await?
                    );
                }
                JobCommands::Worker {
                    worker_id,
                    lease_seconds,
                    poll_interval_ms,
                    once,
                } => {
                    let worker =
                        worker_id.unwrap_or_else(|| format!("worker-{}", uuid::Uuid::new_v4()));
                    tracing::info!(worker_id=%worker,lease_seconds,"job worker started");
                    loop {
                        match jobs::execute_next(&pool, &worker, lease_seconds).await {
                            Ok(Some(job)) => {
                                println!(
                                    "Job {} -> {} (attempt {}/{})",
                                    job.id, job.status, job.attempt_count, job.maximum_attempts
                                );
                                if once {
                                    break;
                                }
                            }
                            Ok(None) if once => {
                                println!("Queue is empty.");
                                break;
                            }
                            Ok(None) => {
                                tokio::select! {
                                    _ = tokio::signal::ctrl_c() => { tracing::info!("worker shutdown requested"); break; }
                                    _ = tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)) => {}
                                }
                            }
                            Err(error) => {
                                tracing::error!(error=%error,"worker iteration failed");
                                if once {
                                    return Err(error);
                                }
                                tokio::select! {
                                    _ = tokio::signal::ctrl_c() => { tracing::info!("worker shutdown requested"); break; }
                                    _ = tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)) => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        Commands::Prompt {
            command: cli::PromptCommands::Optimize { prompt, mode },
        } => {
            let result = crate::prompt::optimize(&prompt, &mode)?;
            println!(
                "Original prompt:\n{}\n\nOptimized prompt:\n{}",
                result.original, result.optimized
            );
        }
        Commands::Chat {
            model,
            prompt,
            system,
        } => {
            db::require_migrated(&pool).await?;
            let response =
                inference::complete_by_name(&pool, &model, system.as_deref(), &prompt).await?;
            println!("{response}");
        }
        Commands::Serve { bind } => {
            if !bind.ip().is_loopback() {
                anyhow::bail!(
                    "this Phase 1 API has no authentication; refusing non-loopback bind {bind}"
                );
            }
            db::require_migrated(&pool).await?;
            let listener = tokio::net::TcpListener::bind(bind)
                .await
                .with_context(|| format!("could not bind HTTP listener to {bind}"))?;
            tracing::info!(address = %listener.local_addr()?, "HTTP API listening");
            axum::serve(listener, api::router(pool))
                .await
                .context("HTTP server failed")?;
        }
    }

    Ok(())
}
