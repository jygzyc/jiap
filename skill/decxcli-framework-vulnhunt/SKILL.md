---
name: decxcli-framework-vulnhunt
description: Android framework vulnerability hunting skill built on DECX CLI + JADX. Use for framework JAR, system_server, Binder service, AIDL, vendor service, and OEM framework static hunting, exploitability triage, bilingual reporting, and handoff to decxcli-poc.
metadata:
  requires:
    bins: ["decx"]
---

# DECX CLI - Android Framework Vulnerability Hunting

## Overview

Use this skill for framework and Binder-service vulnerability hunting.

Scope:

- In scope: framework collection/opening, Binder service enumeration, AIDL and Stub tracing, permission-gate review, framework exploitability triage, report drafting
- Out of scope: app component recon, exported APK surface enumeration, PoC construction, runtime confirmation
- App APK hunting belongs to `decxcli-app-vulnhunt`
- PoC and runtime validation belong to `decxcli-poc`

Conclusion ceiling:

- Highest allowed state: `statically-supported`
- Never claim `poc-validated`, `runtime-validated`, `verified exploitable`, or equivalent

Command reference lives in `decxcli`.

## Non-Negotiable Rules

### Command Rules

- Every session-backed `decx code` command and every session-backed `decx ard` command must include `-P <port>`
- adb-backed `decx ard` commands such as `system-services`, `perm-info`, and `framework collect` / `framework process` do not use `-P <port>`; use `--serial` / `--adb-path` when needed
- `decx ard framework run` and `decx ard framework open` use `-P <port>` only when they open a DECX session
- Method signatures must use the full format: `"package.Class.method(paramType):returnType"`
- Never use `...` in signatures
- Quote package names, class names, method signatures, interfaces, Binder names, and file paths
- If a command is uncertain, check `--help` instead of guessing

### Routing Rules

- Use this skill when the user explicitly mentions framework hunting, framework JARs, `system_server`, Binder services, AIDL interface implementations, vendor services, OEM framework code, or privileged manager/service families
- Do not begin with `app-manifest`, exported components, or app deep links
- If the user is clearly asking about an APK entry surface such as exported Activities, Providers, Receivers, app Services, or WebView hosts, switch to `decxcli-app-vulnhunt`

### Context Rules

- Every subagent must watch context usage
- Hand off at 60% context usage
- Main agent keeps only structured summaries, not raw source dumps
- Outside final reporting, do not paste large command outputs or large code blocks

### Persistence Rules

- Intermediate analysis results may be persisted to local files so work can survive interruption, context handoff, or session restarts
- Prefer JSON for machine-readable phase outputs and Markdown for final human-readable reports
- Keep persisted artifacts small and structured; save parsed results, selected targets, findings, and resume metadata, not raw source dumps
- Before restarting recon or tracing after an interruption, check whether a recent persisted artifact can be loaded and continued
- If a persisted artifact conflicts with the current framework artifact, device build, or session target, treat it as stale and rebuild it instead of forcing reuse

Recommended working directory:

```text
.decx-analysis/<target-name>/
```

Recommended phase artifacts:

- `recon.json`
- `shortlist.json`
- `findings.json`
- `report.md`
- `resume.json`
- `poc-handoff.json`

Template files to fill and save:

- `assets/recon-template.json`
- `assets/shortlist-template.json`
- `assets/findings-template.json`
- `assets/resume-template.json`
- `assets/poc-handoff-template.json`

### Output States

| State | Meaning | Allowed |
|------|---------|---------|
| `candidate` | suspicious path found, still missing evidence | Yes |
| `statically-supported` | static evidence supports reachability, control, bypassability, and visible impact | Yes |
| `rejected` | unreachable, uncontrollable, blocked, or not impactful | Yes |
| `poc-validated` | PoC-confirmed | No |
| `runtime-validated` | runtime-confirmed | No |

Downgrade rules:

- If reachability, controllability, bypass conditions, or visible impact is unclear: downgrade to `candidate`
- If a non-bypassable guard exists or impact is not real: mark `rejected`
- If impact cannot be mapped to `references/risk-rating.md`: do not report it

### Evidence Rules

Every reported finding must explicitly answer:

1. `reachable`
2. `controllable`
3. `bypassConditions`
4. `impactEvidence`
5. `ratingRationale`

Write the exact condition instead of vague language:

- "Bypassable if the gating permission is `normal`/`dangerous`, attacker-defined, or not provably signature-bound"
- "Visible impact: attacker can invoke a privileged Binder path that returns another user's data"
- "HIGH because an untrusted local app can reach a system-privileged service action"

## Workflow

Track progress with:

```text
Framework VulnHunt Progress
- [ ] Phase 1: Prepare framework target and confirm session
- [ ] Phase 2: Enumerate Binder / framework attack surface
- [ ] Phase 3: Trace per-service flows
- [ ] Phase 4: Trace required cross-service or manager chains
- [ ] Phase 5: Filter by exploitability
- [ ] Phase 6: Generate final report
- [ ] Handoff: Pass minimal finding set to decxcli-poc
```

### Phase 1 - Prepare Target

Goal: open the correct framework target and confirm DECX is healthy.

Preferred end-to-end collection path:

```bash
decx ard framework run --serial <serial> -P <port>
```

Stepwise collection path when you need artifact retention or output-directory control:

```bash
decx ard framework collect --serial <serial>
decx ard framework process <oem> --serial <serial>
decx ard framework open -P <port> --serial <serial>
```

If the user already provides a framework JAR, use:

```bash
decx ard framework open "<framework-jar-path>" -P <port>
decx process status -P <port>
```

Preparation rules:

- Start here before any code tracing
- Prefer `framework run` when the user wants the fastest route from device to an analyzable DECX session
- Prefer `collect` -> `process` -> `open` when the user wants to preserve artifacts or inspect intermediate outputs
- Reuse the generated `framework_<oem>_<vendor>.jar` session name when possible so later tracing stays stable
- If no device is connected and no framework JAR is provided, stop and ask for one of those two inputs instead of guessing
- `framework collect` and `framework process` are adb/local-artifact operations, not session-backed DECX reads, so do not add `-P <port>` to them
- If the user wants interruption-safe analysis, create the working directory early and keep phase outputs there

Do not close the session automatically. Tell the user they can close it with:

```bash
decx process close "<name>"
```

### Phase 2 - Recon

Goal: enumerate the framework attack surface and return a minimal structured shortlist.

Execution model:

- Create one recon subagent
- Main agent does not run Phase 2 `decx` commands

Framework recon entry:

- Start from one of these anchors:
  - `decx ard system-services --serial <serial> --grep <keyword>`
  - `decx ard system-service-impl "<interface>" -P <port>`
  - `decx code implement "<interface>" -P <port>`
  - `decx code search-method "<methodName>" -P <port>` only if the Binder entrypoint name is still unknown
- Build the shortlist around Binder service name, AIDL interface, Stub implementation, manager facade, and privileged sink families instead of app components
- When the request names a concrete service family such as package, activity, telecom, notification, or OEM vendor service, use that family name as the first `--grep` filter before broadening scope
- Treat the generated framework artifact as the primary codebase under review; live-device `system-services` and `perm-info` data are support evidence, not substitutes for code tracing

Framework-service recon notes:

- `system-services` returns structured JSON:
  - `total`
  - `services[]`
  - per service: `index`, `name`, `interfaces`
- Use `--grep` to narrow by service family or interface keyword before selecting framework-service targets
- Do not treat `system-services` output as raw text; consume the JSON fields directly
- Resolve permission levels with:

```bash
decx ard perm-info "<permission>" --serial <serial>
```

- `perm-info` returns one structured JSON object such as:
  - `permission`
  - `package`
  - `label`
  - `description`
  - `protectionLevel`
- Do not quote or reason over shell-grep snippets when a parsed `perm-info` object is available

Recon output:

- JSON only
- No raw command output
- Only `needsAnalysis: true` targets move to Phase 3
- If interruption-safe analysis is needed, persist the parsed recon result to `recon.json` before moving to Phase 3
- Fill `recon.json` from `assets/recon-template.json`

### Phase 3 - Per-Service Analysis

Goal: upgrade each retained framework target to `statically-supported` or downgrade it to `rejected`.

Overview mapping:

| Target type | Overview file | Common entrypoints |
|------------|---------------|--------------------|
| Framework service | `references/framework-service.md` | Binder-exposed methods, Stub `onTransact`, service impl methods, privileged manager calls |

Execution notes:

- Start with `decx ard system-services --serial <serial> --grep <keyword>` to map the runtime service surface
- Use the returned `interfaces[]` to choose the most relevant Binder contract before tracing code
- Use `decx ard system-service-impl "<interface>" -P <port>` to resolve the implementation behind one selected interface
- If a framework method path is permission-gated, pair the code trace with `decx ard perm-info "<permission>" --serial <serial>` and reason from the parsed JSON object
- Prefer exact interface names from `system-services.interfaces[]`; do not invent or normalize Binder names by hand

Subagent split:

- Service scout: reads `references/framework-service.md` plus target source and chooses likely vuln patterns and entry methods
- Chain subagent: analyzes one method at a time

Chain command:

```bash
decx code method-source "<currentMethod>" -P <port>
```

Method labels:

- `SOURCE`: attacker-controlled Binder or manager input enters here
- `SINK`: privileged service action happens here
- `SAFE`: non-bypassable guard exists here
- `PASS_THROUGH`: keep tracing
- `DEAD_END`: no further value

Common sources:

- AIDL / Binder params
- `Parcel` fields decoded from transactions
- manager facade arguments forwarded into a service
- attacker-controlled package names, UIDs, user IDs, Intents, URIs, or Bundles

Common sinks:

- privileged file, settings, package, account, telecom, notification, or user-state operations
- cross-user reads/writes
- privileged activity/service launches
- identity transitions around `clearCallingIdentity`
- hidden API or system-only operations exposed to an untrusted caller

Common non-bypassable guards:

- `enforceCallingPermission`, `checkCallingPermission`, `enforceCallingOrSelfPermission`
- exact signature or UID enforcement
- immutable allowlists tied to trusted platform packages
- explicit caller-user ownership checks that cannot be attacker-influenced

Upgrade rules:

- Source and sink present but chain incomplete: keep `candidate`
- Source, sink, chain, bypass conditions, and visible impact all established: upgrade to `statically-supported`
- Non-bypassable guard, unreachable entry, or no real impact: `rejected`

Persistence guidance:

- Persist the retained shortlist before deep tracing if Phase 2 took meaningful effort
- Fill `shortlist.json` from `assets/shortlist-template.json`
- Persist findings incrementally in `findings.json` after each service family or confirmed finding
- Fill `findings.json` from `assets/findings-template.json`
- When resuming, load `recon.json`, `shortlist.json`, `findings.json`, and `resume.json` first, then continue from `lastCompletedPhase` instead of repeating completed work

Minimum finding fields:

- `vulnType`
- `risk`
- `status`
- `serviceName`
- `interface`
- `entryPoint`
- `source`
- `sink`
- `callChain`
- `reachable`
- `controllable`
- `guardsChecked`
- `bypassConditions`
- `impactEvidence`
- `ratingRationale`

### Phase 4 - Cross-Service Analysis

Continue only when needed:

- chain crosses Binder boundary, manager facade, or internal service helper
- framework APIs require downstream reachability confirmation
- identity, grant, or user-selection state crosses trust boundaries

Rules:

- Reuse `nextMethods` from the previous phase
- Inherit the existing `chain`
- Keep the same minimal finding schema

### Phase 5 - Exploitability Filter

This phase is static exploitability triage, not exploitation proof.

Quick rejection checks:

| Condition | Check |
|----------|-------|
| `signature` / `signatureOrSystem` permission | `perm-info.protectionLevel` |
| exact signature enforcement | `checkSignatures`, platform-signer compare |
| hard package / UID allowlist | immutable trusted allowlist |
| root / system-only path | privileged-only API or environment |
| non-bypassable guard on source-to-sink path | permission, ownership, identity, cryptographic, or strict type guard |

Framework-specific rules:

- If `perm-info` confirms `signature` or `signatureOrSystem`, treat the path as protected unless the app forwards access or re-grants capability
- If the gating permission is missing from runtime resolution, custom, or not provably signature-bound, keep the finding at `candidate` unless bypass conditions are made explicit
- If `system-services` shows no matching Binder service on the connected device, downgrade runtime-reachability confidence for that framework path

Three-factor gate:

1. Reachable
2. Controllable
3. Impactful

Decision rules:

- All three factors plus bypass conditions plus impact evidence present: `statically-supported`
- Any factor missing: `rejected`

### Phase 6 - Report

Goal: generate the final Markdown report using only `statically-supported` findings.

Execution model:

- Create one reporting subagent
- Main agent does not draft the full report body

Reporting steps:

1. Read `assets/report-template.md`
2. Read `references/risk-rating.md`
3. Pick language mode:
   - `zh` -> `assets/report-template-zh.md`
   - `en` -> `assets/report-template-en.md`
   - `both` -> Chinese first, then English
4. Re-fetch only key source, sink, and missing-guard locations with `decx code method-source "<method>" -P <port>`
5. Fill the selected template strictly

Mandatory report content:

- `Bypass Conditions / Uncertainties`
- `Visible Impact`
- `Rating Rationale`

Report wording rules:

- Use wording like "Static analysis supports this vulnerability chain"
- Never write "verified", "fully exploited", or "confirmed exploitable in practice"
- If impact is conditional, state the condition directly

Format rules:

- Follow `assets/report-template.md`
- Do not add extra sections, reorder sections, insert JSON, or include rejected findings

## Handoff Protocol

At 60% context usage, hand off immediately:

```json
{
  "handoff": true,
  "phase": "<current phase>",
  "component": "<current service or null during recon>",
  "port": 31234,
  "done": "<completed work>",
  "next": "<entry instruction for the next subagent>",
  "context": "<minimum context needed to continue>"
}
```

If file-backed continuation is enabled, also persist the same handoff state to `resume.json` using `assets/resume-template.json`.

Phase-specific context:

- Phase 2: parsed service list and remaining recon commands
- Phase 3: `vulnTypesDone`, `vulnTypesPending`, current `chain`, finished findings
- Phase 4: cross-service chain progress
- Phase 6: completed and pending report sections

Resume procedure:

1. Load `resume.json`
2. Verify `target`, `artifactPath`, `sessionName`, `port`, and `serial` still match the current analysis target
3. Load the referenced intermediate artifacts such as `recon.json`, `shortlist.json`, and `findings.json`
4. Reconfirm the DECX session with `decx process status -P <port>`
5. Continue from `lastCompletedPhase` and `nextAction` instead of re-running completed phases

## Handoff To `decxcli-poc`

Pass only the minimal static finding.

- Fill `poc-handoff.json` from `assets/poc-handoff-template.json`
- Keep only one finding per handoff file unless the user explicitly asks for a batch handoff

Never pass:

- large raw source blocks
- full recon output
- unrelated services
- findings below `statically-supported`

## References

Use the overview files as the entry layer:

- `references/framework-service.md`
- `references/risk-rating.md`
