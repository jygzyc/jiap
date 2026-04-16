---
name: decxcli-app-vulnhunt
description: Android app vulnerability hunting skill built on DECX CLI + JADX. Use for APK attack-surface enumeration, exported component and deep-link triage, WebView and IPC flow tracing, exploitability screening, bilingual reporting, and handoff to decxcli-poc.
metadata:
  requires:
    bins: ["decx"]
---

# DECX CLI - Android App Vulnerability Hunting

## Overview

Use this skill for app-layer vulnerability hunting against APK targets.

Scope:

- In scope: exported component enumeration, deep-link review, WebView and IPC tracing, static exploitability triage, risk-rating guidance, report drafting
- Out of scope: framework/service hunting, PoC construction, runtime confirmation
- Framework/service hunting belongs to `decxcli-framework-vulnhunt`
- PoC and runtime validation belong to `decxcli-poc`

Conclusion ceiling:

- Highest allowed state: `statically-supported`
- Never claim `poc-validated`, `runtime-validated`, `verified exploitable`, or equivalent

Command reference lives in `decxcli`.

## Non-Negotiable Rules

### Command Rules

- Every session-backed `decx code` command and every session-backed `decx ard` command must include `-P <port>`
- adb-backed `decx ard` commands such as `system-services`, `perm-info`, and `framework collect` / `framework process` do not use `-P <port>`
- Method signatures must use the full format: `"package.Class.method(paramType):returnType"`
- Never use `...` in signatures
- Quote package names, class names, method signatures, and file paths
- If a command is uncertain, check `--help` instead of guessing

### Routing Rules

- Use this skill only when the target is an APK/app session and the attack surface starts from Activities, Services, Providers, Receivers, deep links, WebViews, scan flows, or app Binder surfaces
- If the request is about `system_server`, framework JARs, vendor services, Binder service implementations, AIDL service families, or OEM framework logic, switch to `decxcli-framework-vulnhunt`
- Do not begin framework hunts with `app-manifest`, exported components, or app deep links

### Context Rules

- Every subagent must watch context usage
- Hand off at 60% context usage
- Main agent keeps only structured summaries, not raw source dumps
- Outside final reporting, do not paste large command outputs or large code blocks

### Persistence Rules

- Intermediate analysis results may be persisted to local files so work can survive interruption, context handoff, or session restarts
- Prefer JSON for parsed recon results, shortlisted targets, findings, and resume metadata
- Keep persisted artifacts structured and compact; do not save raw source dumps unless the user explicitly asks
- Before repeating recon after an interruption, check whether a recent persisted artifact can be loaded and continued

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

Do not write vague phrases like:

- "may be bypassable"
- "could lead to privilege escalation"
- "might leak sensitive data"

Write the exact condition instead:

- "Bypassable if the permission is `normal`/`dangerous`, attacker-defined, or not provably signature-bound"
- "Visible impact: attacker can read account rows through the exported provider"
- "HIGH because this exposes app-sandbox data to an untrusted local app"

## Workflow

Track progress with:

```text
App VulnHunt Progress
- [ ] Phase 1: Prepare target and confirm session
- [ ] Phase 2: Enumerate app attack surface
- [ ] Phase 3: Trace per-target flows
- [ ] Phase 4: Trace required cross-component chains
- [ ] Phase 5: Filter by exploitability
- [ ] Phase 6: Generate final report
- [ ] Handoff: Pass minimal finding set to decxcli-poc
```

### Phase 1 - Prepare Target

Goal: open the APK target and confirm DECX is healthy.

```bash
decx process open "<apk-path>" -P <port>
decx process status -P <port>
```

Do not close the session automatically. Tell the user they can close it with:

```bash
decx process close "<name>"
```

### Phase 2 - Recon

Goal: enumerate externally reachable app surfaces and return a minimal structured shortlist.

Execution model:

- Create one recon subagent
- Main agent does not run Phase 2 `decx` commands

Recon commands:

```bash
decx ard exported-components -P <port>
decx ard app-deeplinks -P <port>
decx ard app-receivers -P <port>
decx ard app-manifest -P <port>
decx ard get-aidl -P <port>
decx ard strings -P <port>
```

Recon rules:

1. Collect the full app permission inventory first:
   - declared `<permission>`
   - `<uses-permission>`
   - component-level `android:permission`, `readPermission`, `writePermission`
   - receiver send/receive permissions
   - provider read/write permissions and grant flags
2. Resolve each collected permission and record its `protectionLevel`, owner, and where it is used
3. Compare every externally reachable component and IPC path against that full permission inventory before filtering
4. Treat `signature` and `signatureOrSystem` as protected by default, but keep analyzing when the app forwards, proxies, re-grants, reuses victim identity, exposes a weaker alternate path, or when the permission ownership/binding is uncertain
5. Group targets by Activity, Service, Provider, Receiver, WebView, AIDL/Binder, Deep Link, and scan-driven entrypoint
6. Initialize every retained target as `candidate`

Modern surfaces that must be considered:

- Deep Links and App Links
- mutable `PendingIntent` flows
- `ClipData` and URI grants
- WebView `intent://`, custom schemes, `postMessage`, message ports, file chooser callbacks, QR/scan result handlers, browser-to-native bridges
- `ActivityResultLauncher` and legacy `onActivityResult()` flows
- Provider `call()`, `applyBatch()`, `bulkInsert()`, `openTypedAssetFile()`, FileProvider grants

Permission triage:

| protectionLevel | Triage |
|----------------|--------|
| `normal` | unprotected |
| `dangerous` | weak protection |
| `signature` / `signatureOrSystem` | protected by default; keep analyzing if there is forwarding, proxying, re-granting, victim-identity reuse, or uncertain ownership |
| unknown / defined in another app | uncertain until bypass conditions are explicit |

Permission analysis rules:

- Do not reason only from exported status; always compare the component or IPC path against the full permission inventory first
- For each retained target, record:
  - entry permission
  - downstream permissions checked later in code
  - whether the same capability is reachable through a weaker alternate path
- If a `signature` or `signatureOrSystem` gate exists, check whether:
  - the app later forwards attacker-controlled data into a protected internal path
  - a trusted caller can be induced to execute the path on the attacker's behalf
  - URI grants, `PendingIntent`, result returns, or Binder handles re-grant access after the initial gate
  - a second exported component reaches the same sink with weaker protection
- If those conditions are absent and the signature-bound gate is non-bypassable, reject the third-party attack path

Recon output:

- JSON only
- No raw command output
- Only `needsAnalysis: true` targets move to Phase 3
- If interruption-safe analysis is needed, persist the parsed recon result to `recon.json` before moving to Phase 3
- Fill `recon.json` from `assets/recon-template.json`

### Phase 3 - Per-Target Analysis

Goal: upgrade each retained target to `statically-supported` or downgrade it to `rejected`.

Overview mapping:

| Target type | Overview file | Common entrypoints |
|------------|---------------|--------------------|
| Activity | `references/app-activity.md` | `onCreate`, `onNewIntent`, result callbacks |
| Service | `references/app-service.md` | `onBind`, `onStartCommand`, `handleMessage` |
| Provider | `references/app-provider.md` | `query`, `insert`, `update`, `delete`, `openFile`, `call`, `applyBatch`, `bulkInsert` |
| Receiver | `references/app-broadcast.md` | `onReceive` |
| Intent flow | `references/app-intent.md` | explicit/implicit forwarding, parcel, grant, result-return paths |
| WebView host | `references/app-webview.md` | host init, URL loaders, bridge registration, result handlers |

Subagent split:

- Component scout: reads the overview file plus target source and chooses likely vuln patterns and entry methods
- Chain subagent: analyzes one method at a time

Chain command:

```bash
decx code method-source "<currentMethod>" -P <port>
```

Method labels:

- `SOURCE`: attacker-controlled input enters here
- `SINK`: sensitive action happens here
- `SAFE`: non-bypassable guard exists here
- `PASS_THROUGH`: keep tracing
- `DEAD_END`: no further value

Common sources:

- `getIntent().get*Extra()`
- `getIntent().getData()`
- incoming `ClipData`
- activity-result and scan callbacks
- provider query results
- AIDL / Binder params
- `Messenger.handleMessage()` data

Common sinks:

- `startActivity()`, `startService()`, `sendBroadcast()`
- `PendingIntent.send()`
- `Runtime.exec()`, `ProcessBuilder`
- file read/write/delete/rename
- `setResult()`
- `WebView.loadUrl()`, `loadDataWithBaseURL()`, `evaluateJavascript()`
- `CookieManager.setCookie()`
- `execSQL()`

Common non-bypassable guards:

- exact package-signature or UID allowlist
- integrity or HMAC verification tied to unforgeable material
- strict typed parsing plus explicit class allowlist
- `checkSignatures`, `enforceCallingPermission`, `checkCallingPermission`

Upgrade rules:

- Source and sink present but chain incomplete: keep `candidate`
- Source, sink, chain, bypass conditions, and visible impact all established: upgrade to `statically-supported`
- Non-bypassable guard, unreachable entry, or no real impact: `rejected`

Trace only tainted-data-relevant calls. Skip logging, pure UI, and obvious utility dead ends.

Minimum finding fields:

- `vulnType`
- `risk`
- `status`
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

### Phase 4 - Cross-Component Analysis

Continue only when needed:

- chain crosses component boundaries
- `PendingIntent`, URI grants, activity results, or scan results cross trust boundaries

Rules:

- Reuse `nextMethods` from the previous phase
- Inherit the existing `chain`
- Keep the same minimal finding schema

Persistence guidance:

- Persist the retained shortlist before deep tracing if Phase 2 took meaningful effort
- Fill `shortlist.json` from `assets/shortlist-template.json`
- Persist findings incrementally in `findings.json` after each target family or confirmed finding
- Fill `findings.json` from `assets/findings-template.json`
- When resuming, load `recon.json`, `shortlist.json`, `findings.json`, and `resume.json` first, then continue from `lastCompletedPhase` instead of repeating completed work

### Phase 5 - Exploitability Filter

This phase is static exploitability triage, not exploitation proof.

Quick rejection checks:

| Condition | Check |
|----------|-------|
| `signature` / `signatureOrSystem` permission | manifest `protectionLevel` |
| exact signature enforcement | `checkSignatures`, signature compare |
| hard package / UID allowlist | immutable trusted allowlist |
| root / system-only path | privileged-only API or environment |
| non-bypassable guard on source-to-sink path | integrity, cryptographic, or strict type guard |

Three-factor gate:

1. Reachable
2. Controllable
3. Impactful

Uncertainty rules:

- If a permission is defined outside the current app and DECX cannot prove its level, do not assume protection
- If a component is protected by `signature` or `signatureOrSystem`, do not stop there; verify whether forwarding, proxy execution, grants, or alternate weaker paths still make the sink reachable
- Keep the finding at `candidate` unless the exact bypass condition is stated

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

Language rules:

- If user requests Chinese: `zh`
- If user requests English: `en`
- If user requests bilingual output: `both`
- Otherwise follow the user's current language

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
  "component": "<current component or null during recon>",
  "port": 31234,
  "done": "<completed work>",
  "next": "<entry instruction for the next subagent>",
  "context": "<minimum context needed to continue>"
}
```

If file-backed continuation is enabled, also persist the same handoff state to `resume.json` using `assets/resume-template.json`.

Phase-specific context:

- Phase 2: parsed component list and remaining recon commands
- Phase 3: `vulnTypesDone`, `vulnTypesPending`, current `chain`, finished findings
- Phase 4: cross-component chain progress
- Phase 6: completed and pending report sections

Resume procedure:

1. Load `resume.json`
2. Verify `target`, `artifactPath`, `sessionName`, and `port` still match the current analysis target
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
- unrelated components
- findings below `statically-supported`

## References

Use the overview files as the entry layer:

- `references/app-activity.md`
- `references/app-intent.md`
- `references/app-broadcast.md`
- `references/app-provider.md`
- `references/app-service.md`
- `references/app-webview.md`
- `references/risk-rating.md`
