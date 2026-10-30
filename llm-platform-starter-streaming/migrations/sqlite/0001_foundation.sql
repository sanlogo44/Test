-- Foundation schema. Forward-only migrations are managed by SQLx.
-- Timestamps are UTC ISO-8601 strings. SQLite foreign keys are enabled in connection options.

CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL COLLATE NOCASE UNIQUE,
    password_hash TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled', 'pending')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_login_at TEXT
);

CREATE TABLE roles (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT ''
);

CREATE TABLE user_roles (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id TEXT NOT NULL REFERENCES roles(id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (user_id, role_id)
);

INSERT INTO roles (id, name, description) VALUES
    ('role-administrator', 'administrator', 'Full administrative access'),
    ('role-operator', 'operator', 'Operate approved platform workflows'),
    ('role-user', 'user', 'Use permitted platform features'),
    ('role-read-only', 'read_only', 'Read-only access');

CREATE TABLE model_providers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    provider_type TEXT NOT NULL,
    endpoint TEXT,
    configuration_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(configuration_json)),
    secret_ref TEXT,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE models (
    id TEXT PRIMARY KEY NOT NULL,
    provider_id TEXT NOT NULL REFERENCES model_providers(id) ON DELETE RESTRICT,
    external_model_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    model_family TEXT,
    source TEXT NOT NULL DEFAULT 'unknown' CHECK (source IN ('local', 'remote', 'unknown')),
    local_path TEXT,
    quantization TEXT,
    parameter_count INTEGER CHECK (parameter_count IS NULL OR parameter_count >= 0),
    context_window INTEGER CHECK (context_window IS NULL OR context_window > 0),
    capability_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(capability_json)),
    license_metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(license_metadata_json)),
    checksum TEXT,
    status TEXT NOT NULL DEFAULT 'registered' CHECK (status IN ('registered', 'available', 'unavailable', 'disabled')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (provider_id, external_model_id)
);

CREATE TABLE conversations (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT 'New conversation',
    execution_mode TEXT NOT NULL DEFAULT 'chat' CHECK (execution_mode IN ('chat', 'agent')),
    selected_model_id TEXT REFERENCES models(id) ON DELETE SET NULL,
    routing_mode TEXT NOT NULL DEFAULT 'best_answer' CHECK (routing_mode IN ('best_answer', 'fastest', 'lowest_cost', 'custom')),
    system_prompt TEXT,
    settings_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(settings_json)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    archived_at TEXT
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    parent_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
    content TEXT NOT NULL,
    content_format TEXT NOT NULL DEFAULT 'text',
    status TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'streaming', 'completed', 'failed', 'cancelled')),
    sequence_number INTEGER NOT NULL CHECK (sequence_number >= 0),
    token_count_input INTEGER CHECK (token_count_input IS NULL OR token_count_input >= 0),
    token_count_output INTEGER CHECK (token_count_output IS NULL OR token_count_output >= 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT,
    UNIQUE (conversation_id, sequence_number)
);

CREATE TABLE message_model_runs (
    id TEXT PRIMARY KEY NOT NULL,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    model_id TEXT REFERENCES models(id) ON DELETE SET NULL,
    execution_status TEXT NOT NULL CHECK (execution_status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    routing_reason TEXT NOT NULL DEFAULT '',
    latency_ms INTEGER CHECK (latency_ms IS NULL OR latency_ms >= 0),
    time_to_first_token_ms INTEGER CHECK (time_to_first_token_ms IS NULL OR time_to_first_token_ms >= 0),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    estimated_cost REAL CHECK (estimated_cost IS NULL OR estimated_cost >= 0),
    actual_cost REAL CHECK (actual_cost IS NULL OR actual_cost >= 0),
    error_code TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT
);

CREATE TABLE jobs (
    id TEXT PRIMARY KEY NOT NULL,
    job_type TEXT NOT NULL,
    owner_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled', 'dead_letter')),
    priority INTEGER NOT NULL DEFAULT 0,
    payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
    idempotency_key TEXT UNIQUE,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    maximum_attempts INTEGER NOT NULL DEFAULT 3 CHECK (maximum_attempts > 0),
    scheduled_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    completed_at TEXT,
    lease_owner TEXT,
    lease_expires_at TEXT,
    result_reference TEXT,
    error_code TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE job_attempts (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL CHECK (attempt_number > 0),
    worker_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'abandoned')),
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT,
    error_summary TEXT,
    UNIQUE (job_id, attempt_number)
);

CREATE TABLE audit_events (
    id TEXT PRIMARY KEY NOT NULL,
    actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'failure', 'denied')),
    correlation_id TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_user_roles_role_id ON user_roles(role_id);
CREATE INDEX idx_models_provider_status ON models(provider_id, status);
CREATE INDEX idx_conversations_user_updated ON conversations(user_id, updated_at DESC);
CREATE INDEX idx_messages_conversation_sequence ON messages(conversation_id, sequence_number);
CREATE INDEX idx_message_runs_message ON message_model_runs(message_id, started_at DESC);
CREATE INDEX idx_jobs_status_priority_schedule ON jobs(status, priority DESC, scheduled_at ASC);
CREATE INDEX idx_jobs_lease_expiration ON jobs(status, lease_expires_at);
CREATE INDEX idx_job_attempts_job ON job_attempts(job_id, attempt_number);
CREATE INDEX idx_audit_actor_created ON audit_events(actor_user_id, created_at DESC);
CREATE INDEX idx_audit_resource_created ON audit_events(resource_type, resource_id, created_at DESC);
