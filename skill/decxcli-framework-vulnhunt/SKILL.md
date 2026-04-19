---
name: decxcli-framework-vulnhunt
description: Use when hunting Android framework vulnerabilities in a processed final framework bundle, `system_server`, Binder services, AIDL implementations, vendor services, or OEM framework code.
metadata:
  requires:
    bins: ["decx"]
---

# DECX CLI - Android Framework Vulnerability Hunting

## Overview

Use this skill for static framework and Binder-service vulnerability hunting.

- In scope: processed final framework bundle analysis, Binder service enumeration, AIDL and Stub tracing, permission-gate review, exploitability triage, report drafting
- Out of scope: APK entry-surface hunting, PoC construction, runtime confirmation, proactive analysis of split jars under `source/`
- Use `decxcli-app-vulnhunt` for APK surfaces and `decxcli-poc` for PoC or runtime validation
- Highest allowed conclusion: `statically-supported`
- Never claim `poc-validated`, `runtime-validated`, `verified exploitable`, or equivalent
- Command reference lives in `decxcli`

## Non-Negotiable Rules

### Scope And Routing

- Analyze exactly one framework code target per hunt: the processed final framework bundle such as `framework_<oem>_<vendor>.jar` or a user-provided equivalent final bundle
- Do not proactively open, trace, compare, or switch to split framework jars under `source/` as separate DECX targets
- If the only available path points to `source/` or another split jar, stop and ask for the processed final bundle
- Use this skill for framework hunting, `system_server`, Binder services, AIDL implementations, vendor services, OEM framework code, and privileged manager or service families
- If the request is really about exported Activities, Providers, Receivers, app Services, deep links, or WebView hosts, switch to `decxcli-app-vulnhunt`

### Command Rules

- Every session-backed `decx code` command and every session-backed `decx ard` command must include `-P <port>`
- adb-backed `decx ard` commands such as `system-services`, `perm-info`, `framework collect`, and `framework process` do not use `-P <port>`; use `--serial` and `--adb-path` when needed
- `decx ard framework run` and `decx ard framework open` use `-P <port>` only when they open a DECX session
- Method signatures must use the full format: `"package.Class.method(paramType):returnType"`
- Never use `...` in signatures
- Quote package names, class names, method signatures, interfaces, Binder names, and file paths
- If a command is uncertain, check `--help` instead of guessing

### Context, Persistence, And Output

- Hand off at 60% context usage; keep only structured summaries, not raw source dumps
- Outside final reporting, do not paste large command outputs or large code blocks
- Persist small structured artifacts so work can survive interruption or session restarts
- Use `.decx-analysis/<target-name>/`
- Recommended artifacts: `recon.json`, `shortlist.json`, `findings.json`, `report.md`, `resume.json`, `poc-handoff.json`
- Fill them from `assets/recon-template.json`, `assets/shortlist-template.json`, `assets/findings-template.json`, `assets/resume-template.json`, and `assets/poc-handoff-template.json`
- If a persisted artifact conflicts with the current framework artifact, device build, or session target, treat it as stale and rebuild it

### Evidence And States

| State | Meaning | Allowed |
|------|---------|---------|
| `candidate` | suspicious path found, still missing evidence | Yes |
| `statically-supported` | static evidence supports reachability, control, bypassability, and visible impact | Yes |
| `rejected` | unreachable, uncontrollable, blocked, or not impactful | Yes |

Every reported finding must answer:

- `reachable`
- `controllable`
- `bypassConditions`
- `impactEvidence`
- `ratingRationale`

Decision rules:

- If reachability, controllability, bypass conditions, or visible impact is unclear: keep or downgrade to `candidate`
- If a non-bypassable guard exists or impact is not real: mark `rejected`
- If impact cannot be mapped to `references/risk-rating.md`: do not report it

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

Goal: open the final framework bundle and confirm DECX is healthy.

Fastest path:

```bash
decx ard framework run --serial <serial> -P <port>
```

Artifact-retaining path:

```bash
decx ard framework collect --serial <serial>
decx ard framework process <oem> --serial <serial>
decx ard framework open -P <port> --serial <serial>
```

If the user already has the final bundle:

```bash
decx ard framework open "<framework-jar-path>" -P <port>
decx process status -P <port>
```

Rules:

- Start here before code tracing
- Prefer `framework run` for fastest device-to-session flow
- Prefer `collect` -> `process` -> `open` when artifact retention or output-directory control matters
- Reuse the generated `framework_<oem>_<vendor>.jar` session name when possible
- If no device is connected and no final framework bundle is provided, stop and ask for one
- `framework collect` and `framework process` are adb or local-artifact operations, not session-backed reads, so do not add `-P <port>`
- If interruption-safe analysis matters, create the working directory early and keep artifacts there
- Do not close the session automatically; tell the user they can close it with `decx process close "<name>"`

### Phase 2 - Recon

Goal: enumerate the framework attack surface and return a minimal structured shortlist.

Execution model:

- Create one recon subagent
- Main agent does not run Phase 2 `decx` commands

Start from one of these anchors:

- `decx ard system-services --serial <serial> --grep <keyword>`
- `decx ard system-service-impl "<interface>" -P <port>`
- `decx code implement "<interface>" -P <port>`
- `decx code search-method "<methodName>" -P <port>` only if the Binder entrypoint name is still unknown

Recon rules:

- Build the shortlist around Binder service name, AIDL interface, Stub implementation, manager facade, and privileged sink families instead of app components
- When the request names a concrete service family such as package, activity, telecom, notification, or OEM vendor service, use that family name as the first `--grep` filter before broadening scope
- Treat the open final framework bundle as the only codebase under review; `system-services` and `perm-info` are support evidence, not substitutes for code tracing
- `system-services` returns structured JSON with `total` and `services[]`; use `name` and `interfaces` directly rather than grepping raw text
- Resolve permission levels with `decx ard perm-info "<permission>" --serial <serial>` and reason from the parsed object instead of shell snippets

Recon output:

- JSON only
- No raw command output
- Only `needsAnalysis: true` targets move to Phase 3
- Persist `recon.json` before deep tracing when interruption-safe analysis matters

### Phase 3 - Per-Service Analysis

Goal: upgrade each retained framework target to `statically-supported` or downgrade it to `rejected`, while keeping the current source component, likely issue type, chain progress, and next follow-up targets explicit.

Core loop:

- Start with `decx ard system-services --serial <serial> --grep <keyword>` to map the runtime surface
- Use `interfaces[]` to choose the most relevant Binder contract
- Use `decx ard system-service-impl "<interface>" -P <port>` to resolve one selected implementation
- If a path is permission-gated, pair the code trace with `decx ard perm-info "<permission>" --serial <serial>`
- Prefer exact interface names from `system-services.interfaces[]`; do not invent or normalize Binder names by hand

Subagent split:

- Service scout: reads `references/framework-service.md` plus target source and selects likely vuln patterns and entry methods
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

- AIDL or Binder params
- `Parcel` fields decoded from transactions
- manager facade arguments forwarded into a service
- attacker-controlled package names, UIDs, user IDs, Intents, URIs, or Bundles

Common sinks:

- privileged file, settings, package, account, telecom, notification, or user-state operations
- cross-user reads or writes
- privileged activity or service launches
- identity transitions around `clearCallingIdentity`
- hidden API or system-only operations exposed to an untrusted caller

Common non-bypassable guards:

- `enforceCallingPermission`, `checkCallingPermission`, `enforceCallingOrSelfPermission`
- exact signature or UID enforcement
- immutable allowlists tied to trusted platform packages
- explicit caller-user ownership checks that cannot be attacker-influenced

Phase output:

- Persist the retained shortlist in `shortlist.json` if recon took meaningful effort
- Persist findings incrementally in `findings.json`
- Keep `shortlist.json` or `findings.json` `traceSummary` current with the source component, narrowed issue types, analyzed chains, and next candidate targets
- Minimum finding fields: `vulnType`, `risk`, `status`, `serviceName`, `interface`, `entryPoint`, `source`, `sink`, `callChain`, `traceSummary`, `reachable`, `controllable`, `guardsChecked`, `bypassConditions`, `impactEvidence`, `ratingRationale`

### Phase 4 - Cross-Service Analysis

Continue only when needed:

- the chain crosses a Binder boundary, manager facade, or internal service helper
- downstream reachability must be confirmed
- identity, grant, or user-selection state crosses trust boundaries

Rules:

- Reuse `nextMethods` from the previous phase
- Inherit the existing `chain`
- Keep the same minimal finding schema

### Phase 5 - Exploitability Filter

This phase is exploitability triage based on the traced chain, not exploitation proof.

Quick rejection checks:

| Condition | Check |
|----------|-------|
| `signature` or `signatureOrSystem` permission | `perm-info.protectionLevel` |
| exact signature enforcement | `checkSignatures`, platform-signer compare |
| hard package or UID allowlist | immutable trusted allowlist |
| root or system-only path | privileged-only API or environment |
| non-bypassable guard on source-to-sink path | permission, ownership, identity, cryptographic, or strict type guard |

Framework-specific rules:

- If `perm-info` confirms `signature` or `signatureOrSystem`, treat the path as protected unless access is forwarded or capability is re-granted
- If the gating permission is missing from runtime resolution, custom, or not provably signature-bound, keep the finding at `candidate` unless bypass conditions are explicit
- If `system-services` shows no matching Binder service on the connected device, downgrade runtime-reachability confidence

Three-factor gate:

1. Reachable
2. Controllable
3. Impactful

Decision:

- All three factors plus bypass conditions and impact evidence present: `statically-supported`
- Any factor missing: `rejected`

### Phase 6 - Report

Goal: generate the final Markdown report using only `statically-supported` findings.

Execution model:

- Create one reporting subagent
- Main agent does not draft the full report body

Reporting steps:

1. Read `assets/report-template.md`
2. Read `references/risk-rating.md`
3. Pick language mode: `zh` -> `assets/report-template-zh.md`, `en` -> `assets/report-template-en.md`, `both` -> Chinese first then English
4. Re-fetch only key source, sink, and missing-guard locations with `decx code method-source "<method>" -P <port>`
5. Fill the selected template strictly

Mandatory report content:

- `Bypass Conditions / Uncertainties`
- `Visible Impact`
- `Rating Rationale`

Report rules:

- Describe the traced chain directly; do not foreground the analysis method unless you need to explain uncertainty
- Never write "verified", "fully exploited", or "confirmed exploitable in practice"
- If impact is conditional, state the condition directly
- Follow the selected report template exactly; do not add extra sections, reorder sections, insert JSON, or include rejected findings

## Handoff And Resume

At 60% context usage, hand off immediately:

```json
{
  "handoff": true,
  "phase": "<current phase>",
  "component": "<current service or null during recon>",
  "traceSummary": {
    "sourceComponent": "<current Binder service, manager facade, or Stub>",
    "possibleIssueTypes": ["<current likely issue types>"],
    "analyzedAttackChains": ["<chains already traced>"],
    "nextCandidateTargets": ["<next services or helper chains to analyze>"]
  },
  "port": 31234,
  "done": "<completed work>",
  "next": "<entry instruction for the next subagent>",
  "context": "<minimum context needed to continue>"
}
```

If file-backed continuation is enabled, also persist the same state to `resume.json`.

Resume procedure:

1. Load `resume.json`
2. Verify `target`, `artifactPath`, `sessionName`, `port`, and `serial` still match
3. Load referenced intermediate artifacts such as `recon.json`, `shortlist.json`, and `findings.json`
4. Reconfirm the DECX session with `decx process status -P <port>`
5. Continue from `lastCompletedPhase` and `nextAction` instead of repeating completed work

## Handoff To `decxcli-poc`

Pass only the minimal finding packet plus the current trace focus.

- Fill `poc-handoff.json` from `assets/poc-handoff-template.json`
- Keep only one finding per handoff file unless the user explicitly asks for a batch handoff
- Never pass large raw source blocks, full recon output, unrelated services, or findings below `statically-supported`

## References

- `references/framework-service.md`
- `references/risk-rating.md`
