# Database Architecture — Local-First LLM Platform

## 1. Database Strategy

Use SQLx with Rust and maintain database migrations as version-controlled SQL files.

Supported databases:

- SQLite for standalone local installations.
- PostgreSQL for multi-user and server deployments.

Use a shared logical schema and database abstraction layer. Keep database-specific SQL isolated where behavior differs.

Requirements:

- Versioned, forward-only production migrations.
- Transactional migrations whenever supported.
- Schema version tracking.
- Startup compatibility checks.
- Explicit backup and restore procedures.
- Foreign keys and integrity constraints.
- UTC timestamps stored consistently.
- UUID-based identifiers for entities that may be synchronized across installations.
- Soft deletion only where it provides a concrete benefit.
- No automatic destructive schema changes during application startup.

SQLite must use foreign-key enforcement, an appropriate journal mode, and a documented backup strategy. PostgreSQL must use proper connection pooling and least-privilege database roles.

## 2. Core Tables

### users

Stores local and server user accounts.

Fields:

- id: UUID primary key
- username: unique
- password_hash: nullable when an external identity provider is used
- status: active, disabled, pending
- created_at
- updated_at
- last_login_at: nullable

Never store plaintext passwords.

### roles and user_roles

Define roles and associate them with users.

Initial roles:

- administrator
- operator
- user
- read_only

Use explicit permissions for sensitive operations rather than relying exclusively on role names.

### model_providers

Stores provider configurations.

Fields:

- id
- name
- provider_type
- endpoint
- configuration_json
- enabled
- created_at
- updated_at

API secrets must be stored in a dedicated secrets-management mechanism. Store only a reference to a secret, never the raw secret in ordinary configuration JSON.

### models

Stores registered model definitions.

Fields:

- id
- provider_id
- external_model_id
- display_name
- model_family
- source
- local_path
- quantization
- parameter_count
- context_window
- capability_json
- license_metadata
- checksum
- status
- created_at
- updated_at

A model may exist without every metadata field being known.

Enforce uniqueness for the appropriate provider and external model identifier.

### model_runtime_instances

Tracks loaded model instances.

Fields:

- id
- model_id
- host_id
- runtime_backend
- runtime_configuration_json
- status
- loaded_at
- last_health_check_at
- resource_snapshot_json

Runtime state must be recoverable or reconstructible. Do not treat a database record alone as proof that a model is currently loaded.

### hardware_hosts

Tracks machines that can execute workloads.

Fields:

- id
- name
- host_type
- operating_system
- architecture
- cpu_information_json
- memory_bytes
- gpu_information_json
- runtime_capabilities_json
- last_seen_at

Sensitive infrastructure information must be visible only to authorized users.

## 3. Conversations and Messages

### conversations

Fields:

- id
- user_id
- title
- execution_mode
- selected_model_id: nullable
- routing_mode
- system_prompt
- settings_json
- created_at
- updated_at
- archived_at: nullable

Execution modes:

- chat
- agent

### messages

Fields:

- id
- conversation_id
- parent_message_id: nullable
- role
- content
- content_format
- status
- sequence_number
- token_count_input: nullable
- token_count_output: nullable
- created_at
- completed_at: nullable

Roles include system, user, assistant, and tool.

Use a separate message-parts table or a versioned structured content representation for multimodal messages and attachments.

Do not store large binary files directly in ordinary message rows.

### message_model_runs

Tracks individual model executions associated with a message.

Fields:

- id
- message_id
- model_id
- runtime_instance_id: nullable
- execution_status
- routing_reason
- latency_ms: nullable
- time_to_first_token_ms: nullable
- input_tokens: nullable
- output_tokens: nullable
- estimated_cost: nullable
- actual_cost: nullable
- error_code: nullable
- started_at
- completed_at: nullable

This table enables parallel model execution, response comparisons, and transparent routing decisions.

### message_attachments

Fields:

- id
- message_id
- storage_key
- original_filename
- media_type
- file_size_bytes
- checksum
- processing_status
- created_at

Store attachment contents in managed file storage. Validate file types, enforce size limits, and prevent path traversal.

## 4. Model Routing and Benchmarking

### routing_policies

Fields:

- id
- owner_user_id: nullable
- name
- strategy
- constraints_json
- enabled
- created_at
- updated_at

Strategies include best_answer, fastest, lowest_cost, and custom.

### routing_decisions

Fields:

- id
- request_id
- selected_model_id
- candidate_models_json
- decision_reason
- policy_snapshot_json
- decision_latency_ms
- created_at

Persist enough information to explain why a model was selected without unnecessarily retaining sensitive prompt content.

### benchmark_runs

Fields:

- id
- model_id
- host_id
- benchmark_suite
- benchmark_version
- configuration_json
- status
- started_at
- completed_at

### benchmark_results

Fields:

- id
- benchmark_run_id
- metric_name
- metric_value
- metric_unit
- sample_count
- metadata_json

Keep benchmark measurements separate from inferred estimates.

## 5. Agent Execution

### agent_definitions

Fields:

- id
- owner_user_id
- name
- description
- configuration_json
- enabled
- version
- created_at
- updated_at

### agent_runs

Fields:

- id
- agent_definition_id: nullable
- user_id
- conversation_id: nullable
- parent_run_id: nullable
- task_description
- status
- execution_state_json
- resource_budget_json
- current_step
- created_at
- started_at: nullable
- completed_at: nullable

### agent_steps

Fields:

- id
- agent_run_id
- sequence_number
- step_type
- status
- input_reference
- output_reference
- model_id: nullable
- started_at
- completed_at: nullable
- error_code: nullable

Avoid storing secrets or unrestricted raw tool output in execution records.

### tool_definitions

Fields:

- id
- name
- version
- manifest_json
- permission_requirements_json
- enabled
- created_at
- updated_at

### tool_execution_runs

Fields:

- id
- agent_run_id: nullable
- user_id
- tool_id
- status
- approved_by_user_id: nullable
- input_reference
- output_reference
- started_at
- completed_at: nullable
- error_code: nullable

Every tool execution must be authorized at execution time, not merely when an agent is created.

## 6. Persistent Queue and Scheduling

### jobs

Fields:

- id
- job_type
- owner_user_id
- status
- priority
- payload_json
- idempotency_key: nullable
- attempt_count
- maximum_attempts
- scheduled_at
- started_at: nullable
- completed_at: nullable
- lease_owner: nullable
- lease_expires_at: nullable
- result_reference: nullable
- error_code: nullable
- created_at
- updated_at

Use database transactions to claim jobs safely. PostgreSQL workers may use SKIP LOCKED where appropriate. SQLite requires a compatible single-writer coordination strategy.

Implement leases and recovery for abandoned jobs.

### job_attempts

Fields:

- id
- job_id
- attempt_number
- worker_id
- status
- started_at
- completed_at: nullable
- error_summary: nullable

### scheduled_tasks

Fields:

- id
- owner_user_id
- name
- schedule_expression
- timezone
- task_definition_json
- enabled
- misfire_policy
- last_scheduled_at: nullable
- next_scheduled_at: nullable
- created_at
- updated_at

### scheduled_task_runs

Fields:

- id
- scheduled_task_id
- job_id: nullable
- scheduled_for
- status
- started_at: nullable
- completed_at: nullable
- result_reference: nullable

Enforce an appropriate uniqueness constraint to prevent duplicate creation of the same scheduled occurrence.

## 7. Prompt Optimization and Profiles

### prompt_profiles

Fields:

- id
- owner_user_id: nullable
- name
- optimization_mode
- template
- configuration_json
- created_at
- updated_at

### prompt_optimization_runs

Fields:

- id
- user_id
- conversation_id: nullable
- profile_id: nullable
- optimizer_model_id: nullable
- original_prompt_reference
- optimized_prompt_reference
- status
- created_at
- completed_at: nullable

Make prompt retention configurable. Do not retain original prompts indefinitely by default merely for analytics.

## 8. Training and Fine-Tuning

### datasets

Fields:

- id
- owner_user_id
- name
- storage_key
- format
- checksum
- license_metadata
- validation_status
- record_count: nullable
- created_at

### training_jobs

Fields:

- id
- owner_user_id
- base_model_id
- dataset_id
- training_backend
- configuration_json
- status
- resource_budget_json
- started_at: nullable
- completed_at: nullable
- output_artifact_id: nullable

### model_artifacts

Fields:

- id
- owner_user_id
- source_model_id: nullable
- training_job_id: nullable
- artifact_type
- storage_key
- checksum
- format
- metadata_json
- created_at

### evaluation_runs

Fields:

- id
- model_artifact_id
- evaluation_suite
- configuration_json
- status
- results_reference
- started_at
- completed_at: nullable

Use dataset access controls, artifact provenance, storage quotas, and explicit cleanup policies.

## 9. Security and Audit

### audit_events

Fields:

- id
- actor_user_id: nullable
- action
- resource_type
- resource_id: nullable
- outcome
- correlation_id
- metadata_json
- created_at

Do not include secrets or full sensitive prompt contents in audit metadata.

### api_credentials

Store credential identifiers, ownership, scopes, expiration, revocation status, and secret references.

Actual credentials must be hashed when they are bearer tokens that need verification, or stored in an appropriate secrets manager when the original secret must be retrieved.

### approval_requests

Fields:

- id
- requester_user_id
- agent_run_id: nullable
- tool_execution_run_id: nullable
- requested_action
- risk_level
- status
- expires_at
- decided_by_user_id: nullable
- decided_at: nullable
- created_at

Approvals must be bound to the exact requested action and expire when appropriate. Any material change to the action requires renewed authorization.

## 10. Indexing and Retention

Create indexes for:

- Conversation ownership and update time.
- Messages by conversation and sequence number.
- Model availability and provider.
- Job status, priority, and scheduled execution time.
- Job lease expiration.
- Agent runs by owner and status.
- Scheduled runs by task and scheduled time.
- Audit events by actor, resource, and timestamp.
- Benchmark runs by model and host.

Use full-text search appropriate to the selected database and language requirements.

Define retention policies for messages, execution traces, audit records, benchmark history, and temporary artifacts.

Ensure deletion and export operations account for associated files, references, and derived data.

## 11. Migration and Recovery Strategy

Organize migrations in chronological, immutable files.

Examples:

- 0001_initial_schema.sql
- 0002_model_registry.sql
- 0003_conversations_and_messages.sql
- 0004_jobs_and_scheduling.sql
- 0005_agents_and_tools.sql
- 0006_training_and_artifacts.sql
- 0007_security_and_audit.sql

The actual migration boundaries may change as the implementation evolves.

Requirements:

1. Record applied migration versions.
2. Detect incompatible schema versions.
3. Test migrations against clean databases.
4. Test upgrades from supported previous versions.
5. Use explicit backup and restore workflows.
6. Avoid destructive automatic downgrades.
7. Preserve data when upgrading.
8. Verify referential integrity after migrations.
9. Test crash recovery during migration where practical.
10. Document database-specific behavior.

For production upgrades, require a backup or a verified recovery point before potentially destructive operations.

## 12. Implementation Deliverables

Produce:

- SQL migration files for SQLite and PostgreSQL where required.
- Rust data models.
- SQLx query and repository modules.
- Transaction boundaries.
- Index definitions.
- Validation and authorization rules.
- Migration tests.
- Repository integration tests.
- Backup and restore commands.
- Data retention documentation.
- Example database configurations.

Use database migrations as the source of truth for the schema.

Do not implement the entire schema in one untested operation. Start with users, providers, models, conversations, messages, and persistent jobs. Add the remaining domains incrementally.

The first implementation must compile, pass its tests, and support a real end-to-end workflow.
