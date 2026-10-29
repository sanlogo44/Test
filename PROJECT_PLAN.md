# LLM Platform — Development Plan

This repository currently contains two starter archives:

- `llm-platform-starter.zip`
- `llm_platform_prompts.zip`

The source contents of these archives have not yet been reviewed, so this plan is a starting point rather than a claim that these features already exist.

## 1. Inspect and establish a baseline

- Extract both archives and document their directory structures.
- Identify the language, framework, entry points, dependencies, and configuration files.
- Check for secrets, credentials, local environment files, and generated artifacts before committing extracted files.
- Follow the starter's existing setup instructions and record the commands needed to install, run, and test it.

## 2. Make the smallest end-to-end path work

- Configure one supported model provider using environment variables.
- Add a minimal health check and one example request.
- Validate input and return clear errors for missing configuration, provider failures, and timeouts.
- Keep API keys on the server; never expose them in frontend code or commit them to Git.

## 3. Organize prompts

- Keep prompts in version-controlled files with clear names and descriptions.
- Document each prompt's inputs and expected output.
- Add a small set of representative test cases for expected behavior and edge cases.
- Avoid storing secrets or personal data in prompt examples.

## 4. Add tests and developer tooling

- Add unit tests for configuration, validation, and prompt handling.
- Add a smoke test for the main request path.
- Add a repeatable lint/test command and a CI workflow once the project's language and package manager are confirmed.
- Document known limitations and troubleshooting steps.

## 5. Suggested implementation order

1. Inventory the archives and choose the actual application entry point.
2. Get the starter running unchanged.
3. Fix setup or runtime blockers.
4. Implement one reliable model request end to end.
5. Add prompt tests and error handling.
6. Improve documentation and automate checks.

## Definition of done

- A new developer can follow the README from a clean checkout.
- Required configuration is documented and secrets are not committed.
- The main request path has a reproducible smoke test.
- Test and run commands are documented, with any unverified steps clearly marked.

## Current status

Planning only. The archive contents and runtime have not yet been verified. Update this document as implementation work is completed.
