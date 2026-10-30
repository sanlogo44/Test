# Master Development Prompt: Local-First LLM Intelligence Platform

## 1. Role and Persona

You are a principal software architect, senior Rust engineer, machine learning infrastructure engineer, AI systems researcher, cybersecurity specialist, and product designer.

You are responsible for designing, implementing, testing, documenting, and delivering a production-ready, modular AI platform that runs primarily on local hardware and self-hosted servers.

Think like the technical lead of a serious open-source infrastructure project. Prioritize correctness, security, maintainability, performance, observability, and a clear separation of concerns.

Do not merely generate a prototype, mockup, or proof of concept. Build a functioning application with real integrations, executable code, automated tests, and reproducible deployment instructions.

All source code, identifiers, documentation, comments, and technical specifications should use English consistently.

## 2. Context and Project Vision

Build a local-first AI platform that combines:

- A unified LLM inference and model management engine.
- A powerful command-line interface (CLI) as the primary user interface.
- A normal conversational chat.
- An autonomous but permission-controlled agent mode.
- A multi-model orchestration and routing system.
- Local model inference with automatic hardware detection.
- Model benchmarking, comparison, and selection.
- Fine-tuning and training workflows.
- Prompt optimization using a dedicated lightweight model.
- Persistent background jobs, task queues, and scheduling.
- Optional Flutter-based desktop and web interfaces.
- Secure multi-user server operation.
- Extensible providers, tools, plugins, and model backends.

The platform must work offline for supported local models and local functionality. Cloud services are optional extensions, never mandatory dependencies for the core local workflow.

Treat this as an extensible platform, not a monolithic chat application.

## 3. Core Architectural Principles

Follow these principles throughout the implementation:

1. Local-first execution and data ownership.
2. Security by default and least-privilege permissions.
3. Modular, replaceable components.
4. Stable interfaces between the core engine and external integrations.
5. Explicit capability detection rather than assumptions.
6. Persistent and recoverable background execution.
7. Transparent model selection and routing decisions.
8. Reproducible builds, tests, and deployment.
9. Backward-compatible APIs and versioned configuration.
10. Graceful degradation when optional components are unavailable.

Use a modular monolith initially, with well-defined internal boundaries. Introduce separate processes or services only when isolation, GPU resource management, scaling, or operational requirements justify them.

Do not introduce microservices merely for architectural appearance.

## 4. Technology Stack

Use the following preferred technologies:

### Core

- Rust for the engine, orchestration, CLI, scheduling, and security-critical infrastructure.
- Tokio for asynchronous execution.
- Axum for HTTP APIs and WebSocket or streaming endpoints where appropriate.
- Serde for serialization and configuration.
- SQLx or an equally appropriate database abstraction.
- SQLite for standalone installations.
- PostgreSQL for multi-user and server deployments.

### User interfaces

- Flutter for the optional desktop and web interfaces.
- A terminal-native CLI as the primary interface.
- A clean, dark, minimalist visual design.
- Shared API contracts between the CLI, GUI, and server.

### Model infrastructure

Design an adapter-based runtime supporting compatible backends such as llama.cpp, ONNX Runtime, and other established inference engines.

Support external model APIs through provider adapters, including OpenAI-compatible endpoints where appropriate.

Do not assume that every model format, quantization, GPU backend, or training operation is supported by every runtime. Detect capabilities explicitly.

### Deployment

- Native installation and execution.
- Docker and Docker Compose.
- Optional GPU-enabled containers.
- Self-hosted server deployment.
- Environment-specific configuration.
- Reproducible development and production environments.

Use stable, compatible dependency versions and document any important platform limitations.

## 5. Unified Model Interface

Implement a provider-independent model interface.

It should support operations such as:

- Listing available models.
- Loading and unloading models.
- Generating completions.
- Streaming responses.
- Chat completions.
- Cancellation.
- Embeddings where supported.
- Structured outputs where supported.
- Tool calling where supported.
- Token counting where available.
- Capability and context-window inspection.
- Runtime health checks.
- Model metadata and resource reporting.

Represent unsupported features explicitly.

Each provider adapter must translate its native behavior into the platform's common interfaces without hiding important differences.

Support local models, self-hosted remote models, and optional cloud providers.

## 6. Hardware Discovery and Resource Management

Build a hardware discovery subsystem that identifies:

- CPU architecture and available cores.
- System memory.
- Available storage.
- GPU vendors and devices.
- GPU memory and relevant compute capabilities.
- Supported acceleration backends.
- Operating system and runtime compatibility.

Implement automatic resource mapping to estimate which models can run on the available hardware.

The estimator should consider:

- Model parameter count and architecture.
- Quantization and weight size.
- Context length and KV-cache requirements.
- Runtime overhead.
- GPU memory and CPU memory.
- Available CPU and GPU acceleration.
- Concurrent workloads.
- A configurable safety margin.

Distinguish estimates from measured values. Never guarantee that a model will fit solely because its weight file fits in memory.

Provide recommended configurations and explain why a configuration was selected.

Support manual overrides and resource limits.

GPU detection must be capability-based and platform-specific. Do not assume that all NVIDIA, AMD, or Apple devices support the same backend.

## 7. Model Registry and Benchmarking

Create a persistent model registry.

Track:

- Model identity and source.
- Local path or provider reference.
- Version and checksum where applicable.
- License and provenance metadata.
- Quantization.
- Supported tasks and capabilities.
- Context limits.
- Hardware requirements.
- Runtime compatibility.
- Benchmark results.
- Availability and health status.

Allow users to compare multiple models using reproducible evaluation tasks.

Measure, where applicable:

- Time to first token.
- Generation throughput.
- End-to-end latency.
- Memory consumption.
- Task-specific quality.
- Reliability and structured-output validity.
- Estimated and actual provider costs.

Keep measured results separate from estimates. Record benchmark conditions so comparisons remain meaningful.

Provide import, removal, validation, and model discovery workflows. Never execute downloaded model files or arbitrary installation scripts without appropriate validation and explicit authorization.

## 8. Intelligent Model Router

Build a model-routing subsystem that selects suitable models for each request.

Support these user-selectable routing modes:

### Best Answer

Optimize for expected answer quality, task suitability, and reliability.

### Fastest Answer

Optimize for estimated response latency and time to first token, subject to capability and quality constraints.

### Lowest Cost

Minimize estimated monetary cost and resource cost according to configurable policies.

### Custom

Allow users to define priorities and constraints.

The router should consider:

- Task type.
- Model capabilities.
- Hardware availability.
- Model quality benchmarks.
- Context length.
- Queue depth.
- Current latency.
- Available memory.
- Provider cost.
- Reliability.
- Privacy requirements.
- User-defined restrictions.

Implement a small-model-based routing option, but do not make the entire system dependent on a separate routing LLM.

Start with a deterministic, explainable routing baseline. Optionally use a lightweight classifier or LLM to classify tasks and recommend models.

Provide a routing explanation containing the selected model, alternatives, relevant constraints, and the reason for the decision.

Support routing to one model or multiple models concurrently.

## 9. Multi-Model Execution and Answer Comparison

Allow multiple models to process the same task concurrently.

Support strategies such as:

- Parallel independent responses.
- Specialist model delegation.
- Draft-and-review workflows.
- Independent verification.
- Candidate ranking.
- Optional synthesis of multiple answers.

Implement configurable concurrency limits, cancellation, timeouts, and resource budgets.

Do not assume that combining more model outputs always improves accuracy.

When the user requests the best answer, optionally use a separate evaluator or judge model to compare candidates against a defined rubric.

Keep evaluation criteria explicit and avoid relying exclusively on an LLM judge.

Show users which models contributed to the final response and distinguish individual outputs from synthesized conclusions.

## 10. Prompt Optimization Engine

Implement an optional lightweight prompt optimization subsystem.

Its purpose is to transform an underspecified user request into a clearer, more structured prompt while preserving the original intent.

Use the following structure:

1. Role and Persona — who the model should be.
2. Context and Background — relevant information.
3. Task — exactly what must be accomplished.
4. Constraints and Rules — requirements, prohibitions, and boundaries.
5. Examples — optional few-shot examples when useful.
6. Output Format — required structure and presentation.

The prompt optimizer should operate in English internally by default when configured to do so, while preserving the user's requested output language.

Support at least these modes:

- Disabled: send the original request.
- Conservative: improve clarity without changing scope.
- Advanced: infer missing structure and add relevant task-specific guidance.
- User-defined: apply a custom optimization template.

Requirements:

- Preserve user intent.
- Do not invent facts or claim nonexistent context.
- Do not silently add risky permissions or actions.
- Keep original and optimized prompts available for inspection.
- Allow users to edit, approve, or reject the optimized prompt.
- Provide a way to disable optimization per conversation or request.
- Prevent untrusted input from overriding system policies.
- Avoid unnecessary prompt expansion and excessive token consumption.

Prompt optimization must not replace authorization, validation, or security checks.

## 11. Chat Application

Build a fully functional conversational interface accessible through the CLI and optional GUI.

Required functionality:

- Streaming responses.
- Conversation history.
- Multiple independent conversations.
- Search across conversations.
- Rename, archive, and delete conversations.
- System prompts and reusable profiles.
- Model selection and routing-mode selection.
- Attachments and document analysis.
- Tool invocation and status indicators.
- Cancellation and retry.
- Copy and export functionality.
- Conversation-specific settings.
- Context-window and token-usage information where available.
- Clear indication of the active model or models.
- Error messages and recovery options.

Support normal chat and agent mode as distinct execution modes.

Normal chat must not silently perform privileged actions or autonomous multi-step workflows.

## 12. Agent Runtime

Implement a controlled agent execution framework.

An agent should be able to:

1. Receive a task.
2. Interpret the objective and constraints.
3. Develop a plan.
4. Select appropriate models and tools.
5. Execute authorized actions.
6. Inspect intermediate results.
7. Recover from eligible failures.
8. Verify the outcome.
9. Produce a final report.

Support:

- Configurable agent profiles.
- Tool registration.
- Model assignment.
- Persistent execution state.
- Task deadlines and budgets.
- Retry policies.
- Checkpoints.
- Cancellation.
- Human approval gates.
- Execution logs.
- Result verification.
- Optional agent collaboration.

Implement clear limits on steps, time, token consumption, concurrency, and tool calls.

Agents must not receive unrestricted operating-system access by default.

Treat tool outputs, retrieved documents, websites, and uploaded files as untrusted input. Protect against prompt injection and attempts to bypass system policies.

Require explicit authorization for sensitive actions, external communications, destructive operations, and other consequential side effects.

## 13. Task Queue and Scheduling

Create a persistent task queue for user prompts, model jobs, agent runs, and scheduled tasks.

Support:

- Priority levels.
- FIFO execution within a priority class.
- Configurable concurrency.
- Model-specific resource limits.
- Queue inspection.
- Cancellation.
- Retry with backoff.
- Timeouts.
- Failure states.
- Dead-letter handling.
- Persistent job history.
- Restart recovery.
- Idempotency controls where applicable.

Implement a scheduler that supports cron expressions and explicit time zones.

Example:

Every day at 07:00, run an authorized news-research agent, gather current information from configured sources, verify publication dates, summarize significant developments, and save a report.

The scheduler must:

- Survive application restarts.
- Prevent accidental duplicate execution.
- Handle missed schedules according to explicit policy.
- Record execution results.
- Support pause, resume, edit, and deletion.
- Respect model and tool permissions.
- Avoid claiming that a report or external action succeeded unless it actually did.

Background jobs must have configurable resource limits and should not starve interactive chat requests.

## 14. Fine-Tuning and Training

Provide a dedicated training subsystem that integrates established machine learning tools rather than reimplementing training algorithms unnecessarily.

Support, where compatible:

- Dataset import and validation.
- Dataset inspection and preprocessing.
- Training configuration.
- Fine-tuning workflows.
- LoRA and QLoRA where supported.
- Training job queues.
- GPU and memory estimation.
- Training progress monitoring.
- Checkpoint management.
- Evaluation before and after training.
- Artifact versioning.
- Export and deployment of compatible models.

Integrate suitable external training backends, such as Hugging Face tooling, through clearly defined adapters or worker processes.

Use Python only where necessary for ML ecosystem compatibility, keeping the primary application and orchestration in Rust.

Separate training jobs from inference processes where appropriate.

Training must require explicit user initiation, validated datasets, resource limits, and clear status reporting.

Do not claim that a model has been trained, improved, or exported unless the operation actually completed and the resulting artifacts were verified.

## 15. Security and Multi-User Operation

Implement security as a first-class architectural concern.

Required capabilities include:

- Local authentication options and secure remote authentication.
- Role-based access control.
- User and group permissions.
- Secure session management.
- TLS for remote connections.
- Secrets management.
- Encryption for sensitive stored data where appropriate.
- API key protection and rotation.
- Audit logging.
- Request validation.
- Rate limiting.
- Resource quotas.
- Secure file handling.
- Path traversal protection.
- Protection against command injection.
- SSRF protections for network-enabled tools.
- Sandboxed execution of untrusted code.
- Explicit network access policies.
- Permission-controlled plugins and tools.
- Secure configuration defaults.

Do not expose unauthenticated administrative endpoints.

Bind services to loopback by default for local installations. Require deliberate configuration for remote network exposure.

Never log passwords, API secrets, authentication tokens, or sensitive prompt content by default.

Use least privilege for users, agents, tools, and background workers.

Document the security assumptions and known limitations of each execution mode.

## 16. API, Plugins, and Extensibility

Design a versioned API for the CLI, GUI, and external integrations.

Support, where appropriate:

- REST endpoints.
- Streaming responses.
- Health and readiness checks.
- Authentication and authorization.
- Model discovery.
- Chat and agent execution.
- Queue management.
- Scheduling.
- Training job management.
- Usage metrics.

Provide a plugin system with explicit manifests, permissions, version compatibility, and lifecycle management.

Distinguish trusted in-process extensions from untrusted or third-party code that requires process isolation.

Provide stable interfaces for:

- Model providers.
- Inference backends.
- Training backends.
- Tools.
- Agent capabilities.
- Prompt optimizers.
- Routing strategies.
- Storage providers.
- External notification integrations.

Use an OpenAI-compatible API where it provides practical interoperability, without claiming full compatibility until conformance tests verify it.

## 17. CLI Design

The CLI is the primary interface and must be complete before the optional GUI is considered production-ready.

Design an intuitive command hierarchy, for example:

- `platform chat`
- `platform agent run`
- `platform models list`
- `platform models inspect`
- `platform models benchmark`
- `platform models download`
- `platform models remove`
- `platform route explain`
- `platform queue list`
- `platform queue inspect`
- `platform jobs list`
- `platform schedule create`
- `platform schedule list`
- `platform train`
- `platform server start`
- `platform config`
- `platform doctor`

These commands are illustrative. Refine the names and arguments for consistency.

Include help text, validation, structured errors, meaningful exit codes, configuration discovery, and noninteractive operation for automation.

Support JSON output for scripts and machine-readable integrations.

Provide a first-run setup flow and a diagnostic command that checks hardware, runtimes, storage, configuration, and permissions.

## 18. Flutter GUI

Treat the GUI as an optional client of the same underlying platform.

Implement a minimalist dark theme with:

- Conversation sidebar.
- Chat interface.
- Model and routing-mode selector.
- Agent execution panel.
- Queue and scheduler views.
- Model registry.
- Hardware and GPU monitoring.
- Benchmark comparisons.
- Training job management.
- Configuration and security settings.
- Clear loading, empty, success, and error states.

The GUI must not contain a separate copy of the core business logic.

Use the same versioned API and permission model as the CLI.

Provide an installation and packaging strategy appropriate to the supported operating systems. Treat web deployment as a separate target with its own security and browser constraints.

## 19. Observability and Reliability

Implement structured logging, metrics, tracing, and health checks.

Monitor, where applicable:

- Request latency.
- Time to first token.
- Token throughput.
- Model loading time.
- GPU and memory utilization.
- Queue depth.
- Job success and failure rates.
- Tool execution duration.
- API errors.
- Resource exhaustion.

Provide diagnostic information without exposing sensitive user data.

Implement graceful shutdown, cancellation propagation, recovery of persistent jobs, and bounded resource usage.

Distinguish recoverable errors from unrecoverable failures.

## 20. Testing and Quality Assurance

Implement a comprehensive testing strategy.

Include:

- Unit tests.
- Integration tests.
- API contract tests.
- Provider adapter tests.
- Database migration tests.
- CLI tests.
- Queue and scheduler tests.
- Agent permission tests.
- Prompt injection and security tests.
- Concurrency tests.
- Failure recovery tests.
- Hardware detection tests.
- Benchmark correctness tests.
- GUI integration tests where appropriate.

Use mock providers for deterministic tests. Do not require paid API access or GPU hardware for the default test suite.

Provide separate optional tests for real model inference, GPU acceleration, training, and external providers.

Test malformed inputs, unavailable models, timeouts, cancellation, storage failures, process restarts, and unauthorized actions.

Do not mark a feature complete merely because its code compiles.

## 21. Development and Implementation Strategy

Develop the platform in independently testable phases.

### Phase 1: Foundation

- Establish the repository structure.
- Define architectural boundaries.
- Implement configuration and logging.
- Build the CLI.
- Implement the database and migrations.
- Establish error handling and testing conventions.
- Create the initial security model.

### Phase 2: Inference

- Implement the unified model interface.
- Integrate the first local inference backend.
- Add model discovery and management.
- Implement streaming chat.
- Add hardware detection and resource estimation.

### Phase 3: Orchestration

- Implement the persistent task queue.
- Add model routing and explainability.
- Add parallel model execution.
- Implement prompt optimization.
- Add benchmarking and comparison.

### Phase 4: Agents and Automation

- Implement the agent runtime.
- Add permission-controlled tools.
- Implement persistent workflows.
- Add scheduling and restart recovery.
- Add reporting and verification.

### Phase 5: Training and Advanced Integrations

- Integrate training backends.
- Add datasets, fine-tuning, checkpoints, and evaluations.
- Add external provider adapters.
- Expand plugin support.

### Phase 6: Multi-User and GUI

- Complete authentication and authorization.
- Add remote server operation.
- Implement the Flutter application.
- Add installation and packaging.
- Complete security hardening.

### Phase 7: Production Readiness

- Run the full test suite.
- Verify clean installation.
- Test migrations and recovery.
- Conduct security reviews.
- Validate documentation and deployment procedures.
- Publish reproducible release artifacts.

Reorder phases when justified by technical dependencies, but explain the rationale.

## 22. Repository and Deliverables

Produce a complete, coherent repository containing:

- Application source code.
- Workspace and package configuration.
- Dependency manifests and lockfiles.
- Database migrations.
- Configuration templates.
- Automated tests.
- Docker and deployment files.
- API specifications.
- CLI documentation.
- Architecture documentation.
- Security documentation.
- Development setup instructions.
- User guide.
- Troubleshooting guide.
- Example configurations.
- Example agent definitions.
- Example scheduled tasks.
- CI workflows.
- Licensing and third-party attribution guidance.

Include an architecture decision record for significant technical choices.

Use clear module boundaries and avoid unnecessary abstractions.

Do not use pseudocode, placeholder implementations, or hardcoded fake results for features presented as complete.

If a feature cannot be implemented in the current environment, explain what is missing, provide the actual interface and implementation plan, and do not misrepresent its status.

## 23. Constraints and Engineering Rules

- Never sacrifice security for convenience without explicitly documenting the trade-off.
- Never execute agent actions outside their granted permissions.
- Never fabricate benchmark results or model capabilities.
- Never claim that a build or test passed without actually running it.
- Never silently discard queued work.
- Never hardcode secrets.
- Never assume a GPU or specific runtime is available.
- Never make cloud access mandatory for local inference.
- Never couple the GUI directly to internal engine modules.
- Never make a small routing model a single point of failure.
- Never permit unbounded agent loops, concurrency, or resource consumption.
- Never claim universal compatibility without verification.
- Prefer simple, testable implementations over speculative abstractions.

## 24. Definition of Done

A feature is complete only when:

1. Its requirements and limitations are documented.
2. Its implementation is integrated into the real application.
3. Relevant automated tests exist and pass.
4. Errors and unsupported configurations are handled.
5. Permissions and resource limits are enforced.
6. The feature can be exercised using documented commands or interfaces.
7. Its status is reported accurately.
8. It does not break existing supported functionality.

The overall platform is production-ready only when its documented core workflows can be installed, configured, executed, tested, and recovered successfully in a clean environment.

## 25. First Task

Begin by analyzing the project requirements and identifying the most important technical decisions.

Then:

1. Produce a concise requirements specification.
2. Propose the system architecture and dependency graph.
3. Define the Rust workspace and module boundaries.
4. Explain the model adapter and inference abstraction.
5. Define the database schema and migration strategy.
6. Specify the security and permission model.
7. Create a phased implementation plan with acceptance criteria.
8. Identify technical risks, platform limitations, and unresolved decisions.
9. Implement the first functional vertical slice rather than generating the entire codebase in one unverified operation.
10. Run the relevant tests and report the actual results.

After the first vertical slice works, continue implementing the remaining phases incrementally.

Maintain a development checklist and update it after each completed task.

At every stage, deliver working code, explain how to run it, report which tests were executed, and identify the next concrete implementation step.

Your final objective is a secure, extensible, production-grade local-first LLM platform with a Rust core, CLI-first workflow, unified model orchestration, controlled multi-agent execution, optional Flutter interfaces, and a reliable path toward training, automation, and multi-user deployment.
