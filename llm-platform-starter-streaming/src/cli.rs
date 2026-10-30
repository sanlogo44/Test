use clap::{Parser, Subcommand};
use std::net::SocketAddr;

#[derive(Debug, Parser)]
#[command(
    name = "llm-platform",
    version,
    about = "Local-first LLM intelligence platform"
)]
pub struct Cli {
    /// SQLite connection URL. Example: sqlite://llm-platform.db
    #[arg(
        long,
        global = true,
        env = "LLM_PLATFORM_DATABASE_URL",
        default_value = "sqlite://llm-platform.db"
    )]
    pub database_url: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Database migration and inspection commands.
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
    /// Inspectably restructure a prompt without making an inference request.
    Prompt {
        #[command(subcommand)]
        command: PromptCommands,
    },
    /// Send a prompt to a registered OpenAI-compatible provider.
    Chat {
        /// Registered model external ID (or display name).
        model: String,
        /// User prompt.
        prompt: String,
        /// Optional system instruction.
        #[arg(long)]
        system: Option<String>,
    },
    /// Start the local HTTP API (loopback by default).
    Serve {
        #[arg(long, default_value = "127.0.0.1:8787")]
        bind: SocketAddr,
    },
    /// Manage model-provider records.
    Providers {
        #[command(subcommand)]
        command: ProviderCommands,
    },
    /// Manage registered models.
    Models {
        #[command(subcommand)]
        command: ModelCommands,
    },
    /// Inspect and run persistent background jobs.
    Jobs {
        #[command(subcommand)]
        command: JobCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum DbCommands {
    /// Apply forward-only SQLx migrations. This is explicit; startup never migrates automatically.
    Migrate,
    /// Show the migration state of the configured database.
    Status,
}

#[derive(Debug, Subcommand)]
pub enum ProviderCommands {
    /// Register a provider without storing credentials in the database.
    Add {
        name: String,
        #[arg(long, default_value = "local")]
        provider_type: String,
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// List registered providers.
    List,
}

#[derive(Debug, Subcommand)]
pub enum ModelCommands {
    /// Register an existing local or remote model. This does not download or execute it.
    Register {
        #[arg(long)]
        provider: String,
        #[arg(long)]
        external_id: String,
        #[arg(long)]
        display_name: String,
        #[arg(long)]
        local_path: Option<String>,
        #[arg(long)]
        context_window: Option<i64>,
    },
    /// List registered models.
    List,
}

#[derive(Debug, Subcommand)]
pub enum JobCommands {
    /// Enqueue a JSON job payload.
    Enqueue {
        job_type: String,
        payload: String,
        #[arg(long, default_value_t = 0)]
        priority: i64,
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(i64).range(1..))]
        maximum_attempts: i64,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// List jobs, optionally filtered by status.
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(i64).range(1..=500))]
        limit: i64,
    },
    /// Inspect a job by ID.
    Inspect { id: String },
    /// Cancel a queued job.
    Cancel { id: String },
    /// Recover expired worker leases.
    Recover,
    /// Run a worker; --once processes at most one job.
    Worker {
        #[arg(long)]
        worker_id: Option<String>,
        #[arg(long, default_value_t = 240, value_parser = clap::value_parser!(i64).range(1..))]
        lease_seconds: i64,
        #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u64).range(100..))]
        poll_interval_ms: u64,
        #[arg(long)]
        once: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum PromptCommands {
    /// Format a request into a stable prompt structure.
    Optimize {
        /// Original user request.
        prompt: String,
        /// conservative preserves scope; advanced adds general quality guidance.
        #[arg(long, default_value = "conservative", value_parser = ["conservative", "advanced"])]
        mode: String,
    },
}
