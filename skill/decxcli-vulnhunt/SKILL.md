---
name: decxcli-vulnhunt
description: Android vulnerability hunting skill built on DECX CLI + JADX. Optimized for low-context attack-surface enumeration, static call-chain tracing, exploitability triage, and risk-rating guidance for component, IPC, WebView, and framework-service bugs.
metadata:
  requires:
    bins: ["decx"]
---

# DECX CLI - Android Vulnerability Hunting

## Overview

Use this skill for low-context Android vulnerability hunting.

Scope:

- In scope: attack-surface enumeration, static flow tracing, exploitability triage, risk-rating guidance, report drafting
- Out of scope: exploit proof, final PoC construction, runtime confirmation
- PoC and runtime validation belong to `decxcli-poc`

Conclusion ceiling:

- Highest allowed state: `statically-supported`
- Never claim `poc-validated`, `runtime-validated`, `verified exploitable`, or equivalent

Command reference lives in `decxcli`.

## Non-Negotiable Rules

### Command Rules

- Every `decx code` and `decx ard` command must include `-P <port>`
- Method signatures must use the full format: `"package.Class.method(paramType):returnType"`
- Never use `...` in signatures
- Quote package names, class names, method signatures, and file paths
- If a command is uncertain, check `--help` instead of guessing

### Context Rules

- Every subagent must watch context usage
- Hand off at 60% context usage
- Main agent keeps only structured summaries, not raw source dumps
- Outside final reporting, do not paste large command outputs or large code blocks

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
VulnHunt Progress
- [ ] Phase 1: Open APK and confirm session
- [ ] Phase 2: Enumerate attack surface
- [ ] Phase 3: Trace per-target flows
- [ ] Phase 4: Trace required cross-component chains
- [ ] Phase 5: Filter by exploitability
- [ ] Phase 6: Generate final report
- [ ] Handoff: Pass minimal finding set to decxcli-poc
```

### Phase 1 - Environment

Goal: open the APK and confirm DECX is healthy.

```bash
decx process open "<apk-path>" -P <port>
decx process status -P <port>
```

Do not close the session automatically. Tell the user they can close it with:

```bash
decx process close "<name>" -P <port>
```

### Phase 2 - Recon

Goal: enumerate externally reachable surfaces and return a minimal structured shortlist.

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

1. Resolve each declared permission and read its `protectionLevel`
2. Exclude `signature` and `signatureOrSystem` components from third-party attack paths unless the app forwards or re-grants access
3. Group targets by Activity, Service, Provider, Receiver, WebView, AIDL/Binder, framework service, Deep Link, and scan-driven entrypoint
4. Initialize every retained target as `candidate`

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
| `signature` / `signatureOrSystem` | protected |
| unknown / defined in another app | uncertain until bypass conditions are explicit |

Recon output:

- JSON only
- No raw command output
- Only `needsAnalysis: true` targets move to Phase 3

Minimum recon fields:

- `appInfo`
- `components`
- `aidlInterfaces`
- `deepLinkEntries`
- `scanEntrypoints`
- `summary`

### Phase 3 - Per-Target Analysis

Goal: upgrade each retained target to `statically-supported` or downgrade it to `rejected`.

Overview mapping:

| Target type | Overview file | Common entrypoints |
|------------|---------------|--------------------|
| Activity | `references/app-activity.md` | `onCreate`, `onNewIntent`, result callbacks |
| Service | `references/app-service.md` | `onBind`, `onStartCommand`, `handleMessage` |
| Provider | `references/app-provider.md` | `query`, `insert`, `update`, `delete`, `openFile`, `call`, `applyBatch`, `bulkInsert` |
| Receiver | `references/app-broadcast.md` | `onReceive` |
| WebView host | `references/app-webview.md` | host init, URL loaders, bridge registration, result handlers |
| Framework service | `references/framework-service.md` | Binder-exposed methods |

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
- framework APIs require downstream reachability confirmation
- `PendingIntent`, URI grants, activity results, or scan results cross trust boundaries

Rules:

- Reuse `nextMethods` from the previous phase
- Inherit the existing `chain`
- Keep the same minimal finding schema

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
- Keep the finding at `candidate` unless the exact bypass condition is stated
- Acceptable wording:
  - "Bypassable if the custom permission is `normal`/`dangerous`, attacker-defined, or not signature-bound to the target"
  - "Not bypassable if the permission is confirmed `signature` and owned by the same signer"

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

Self-check:

- [ ] Title and metadata complete
- [ ] Each finding contains required subsections
- [ ] Call chains use full signatures
- [ ] Code analysis ties directly to the security conclusion
- [ ] Bypass conditions, visible impact, and rating rationale are explicit

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

Phase-specific context:

- Phase 2: parsed component list and remaining recon commands
- Phase 3: `vulnTypesDone`, `vulnTypesPending`, current `chain`, finished findings
- Phase 4: cross-component chain progress
- Phase 6: completed and pending report sections

## Handoff To `decxcli-poc`

Pass only the minimal static finding:

```json
{
  "component": "com.example.MyActivity",
  "componentType": "Activity",
  "vulnType": "Intent Redirect",
  "status": "statically-supported",
  "risk": "HIGH",
  "entryPoint": "com.example.MyActivity.onCreate(android.os.Bundle):void",
  "source": "getIntent().getParcelableExtra(\"forward_intent\")",
  "sink": "startActivity(intent)",
  "callChain": [
    "com.example.MyActivity.onCreate(android.os.Bundle):void",
    "com.example.MyActivity.extractNestedIntent(android.content.Intent):android.content.Intent",
    "com.example.MyActivity.startForwardActivity(android.content.Intent):void"
  ],
  "guardsChecked": [
    "No caller UID or package validation",
    "No signature permission protection",
    "No target component allowlist"
  ],
  "bypassConditions": [
    "Custom permission is unverified or attacker-definable",
    "Action-only filtering is bypassable with an explicit component"
  ],
  "impactEvidence": "Attacker can reach a non-exported internal screen that exposes account data",
  "ratingRationale": "HIGH because this exposes protected app data to an untrusted local app",
  "exploitPrerequisites": [
    "Target component is exported",
    "Attacker can send an explicit Intent"
  ],
  "nextAction": "decxcli-poc"
}
```

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
- `references/framework-service.md`
- `references/risk-rating.md`
