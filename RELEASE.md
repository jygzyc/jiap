# DECX v2.6.0

### Changes

- CLI: refactor `installDecxServer` return type from tuple to discriminated union `InstallDecxServerResult`, improving type safety and readability.

- CLI: add dependency injection to `installDecxServer` via `InstallDecxServerOptions` (custom `fetch`, `downloadWithProgress`, install paths, logger), enabling unit testing without network calls.

- CLI: extract `executeSelfInstall` as a testable entry point for `self install`, with manager dependency injection.

- CLI: add installer unit tests (`installer.test.ts`) covering asset selection, version normalization, and download/install flows.

- CLI: bump version to 2.6.0.

- Plugin: update description to reflect DECX (Decompiler + X) branding.

- Skills (vulnhunt): consolidate SKILL.md, add coverage template, remove deprecated shortlist template.

- Skills (poc): replace monolithic `poc-template.zip` with split `poc-template-app/` and `poc-template-server/` templates; add deep-link WebView sink (URL parameter injection) pattern; refactor all PoC references to follow unified contract.

- CI: update build workflow.
