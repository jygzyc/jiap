# DECX v2.3.0

### Changes

- Server: improve version resolution — use Gradle task to generate `version.properties` into build directory instead of writing in `processResources`, and add `Implementation-Version` manifest attribute as fallback when `version.properties` is unavailable.

- Skill (decxcli-vulnhunt): rewrite all 52 vulnerability reference documents with restructured format and clearer guidance. Add 2 new references: `app-provider-batch-abuse` and `app-webview-scan-result-inject`.

- Skill (decxcli-vulnhunt): add bilingual report templates (zh/en) for both full reports and single-issue reports (`report-template-en.md`, `report-template-zh.md`, `report-target-vuln-en.md`, `report-target-vuln-zh.md`).

- Skill (decxcli-poc): rewrite PoC reference documents and scripts (`setup-poc.mjs`, `check-env.mjs`) for improved clarity and maintainability.

- Skill (decxcli): update SKILL.md with refined command reference, session management rules, and output guidelines.
