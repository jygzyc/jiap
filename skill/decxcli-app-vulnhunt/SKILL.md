---
name: decxcli-app-vulnhunt
description: Android app vulnerability hunting skill built on DECX CLI + JADX. Use for APK attack-surface enumeration, exported component and deep-link triage, WebView and IPC flow tracing, exploitability screening, bilingual reporting, and handoff to decxcli-poc.
metadata:
  requires:
    bins: ["decx"]
---

# DECX CLI - Android App Vulnerability Hunting

Use this skill for APK app-layer vulnerability hunting.

Scope:

- In scope: exported component review, deep links, WebView, IPC, exploitability triage, risk rating, report drafting
- Out of scope: framework/service hunting, PoC construction, runtime confirmation
- Framework work belongs to `decxcli-framework-vulnhunt`
- PoC and runtime validation belong to `decxcli-poc`

Ceiling:

- Highest allowed state: `statically-supported`
- Never claim `poc-validated`, `runtime-validated`, or equivalent

## Hard Rules

### Commands

- Every session-backed `decx code` and `decx ard` command must include `-P <port>`
- adb-backed commands such as `system-services`, `perm-info`, and `framework collect/process` do not use `-P <port>`
- Method signatures must use full form: `"package.Class.method(paramType):returnType"`
- Never use `...` in signatures
- Quote package names, classes, methods, and file paths
- If uncertain, check `--help`

### Routing

- Use this skill only for APK/app sessions whose surface starts from Activities, Services, Providers, Receivers, deep links, WebViews, scan flows, or app Binder surfaces
- If the request is about `system_server`, framework jars, vendor services, Binder service implementations, AIDL service families, or OEM framework logic, switch to `decxcli-framework-vulnhunt`

### Context

- Hand off at 60% context usage
- Keep structured summaries, not raw source dumps
- Outside final reporting, do not paste large outputs

### Persistence

Recommended work dir:

```text
.decx-analysis/<target-name>/
```

Recommended artifacts:

- `recon.json`
- `coverage.json`
- `findings.json`
- `report.md`
- `resume.json`
- `poc-handoff.json`

Templates:

- `assets/recon-template.json`
- `assets/coverage-template.json`
- `assets/findings-template.json`
- `assets/resume-template.json`
- `assets/poc-handoff-template.json`

### States

| State | Meaning | Allowed |
|------|---------|---------|
| `candidate` | suspicious path, evidence incomplete | Yes |
| `statically-supported` | static evidence proves reachability, control, bypassability, and visible impact | Yes |
| `rejected` | explicit blocking evidence or no real impact | Yes |
| `poc-validated` | PoC-confirmed | No |
| `runtime-validated` | runtime-confirmed | No |

State rules:

- Unclear reachability, controllability, bypass conditions, or impact: `candidate`
- Explicit non-bypassable guard, unreachable sink, non-controllable source, or no real impact: `rejected`
- Impact not mappable to `references/risk-rating.md`: do not report

### Coverage

- Never silently drop an externally reachable surface
- Every exported component, deep-link handler, dynamic receiver, externally triggerable WebView/native bridge path, AIDL/Binder entry, URI-grant chain, `PendingIntent` chain, and scan-driven entrypoint must appear in `coverage.json`
- `rejected` requires explicit evidence-backed blocking reason
- "Not selected", "too many targets", or "unlikely" are never valid rejection reasons
- If evidence is incomplete, keep the target as `candidate` and record the exact missing proof

### Evidence

Every reported finding must answer:

1. `reachable`
2. `controllable`
3. `bypassConditions`
4. `impactEvidence`
5. `ratingRationale`

## Workflow

```text
App VulnHunt Progress
- [ ] Phase 1: Prepare target and confirm session
- [ ] Phase 2: Build full attack-surface inventory
- [ ] Phase 3: Write first-pass coverage verdicts for every target
- [ ] Phase 4: Deep trace every non-rejected target
- [ ] Phase 5: Finalize exploitability and residual risk set
- [ ] Phase 6: Generate final report
- [ ] Handoff: Pass minimal finding set to decxcli-poc
```

### Phase 1 - Prepare

Goal: open the APK and confirm DECX is healthy.

```bash
decx process open "<apk-path>" -P <port>
decx process status -P <port>
```

Do not close the session automatically. If needed, tell the user:

```bash
decx process close "<name>"
```

### Phase 2 - Recon

Goal: produce a complete external-surface inventory in `recon.json`.

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

Recon requirements:

1. Collect the full permission inventory:
   - declared `<permission>`
   - `<uses-permission>`
   - component `android:permission`, `readPermission`, `writePermission`
   - receiver send/receive permissions
   - provider read/write permissions and grant flags
2. Resolve each permission and record `protectionLevel`, owner, and usage
3. Compare every reachable component or IPC path against that inventory
4. Treat `signature` and `signatureOrSystem` as protected by default, but keep tracing when the app forwards, proxies, re-grants, reuses victim identity, exposes a weaker alternate path, or ownership is uncertain
5. Group targets by Activity, Service, Provider, Receiver, WebView, AIDL/Binder, Deep Link, scan-driven entrypoint, `PendingIntent`, and URI-grant chain
6. Assign every surface a stable `targetId`
7. Initialize every target as `candidate`

Must-consider surfaces:

- Deep Links and App Links
- mutable `PendingIntent`
- `ClipData` and URI grants
- WebView `intent://`, custom schemes, `postMessage`, message ports, file chooser callbacks, QR/scan result handlers, browser-to-native bridges
- `ActivityResultLauncher` and legacy `onActivityResult()`
- Provider `call()`, `applyBatch()`, `bulkInsert()`, `openTypedAssetFile()`, FileProvider grants

Output rules:

- JSON only
- No raw command output
- Persist `recon.json` before Phase 3
- `recon.json` is inventory, not a filter result

### Phase 3 - First-Pass Coverage

Goal: write one evidence-backed coverage row for every `targetId`, centered on the traced source component, likely issue type, chain progress, and next follow-up targets.

Required row fields:

- `targetId`
- `targetType`
- `entryPoint`
- `entryPermission`
- `sourceComponent`
- `possibleIssueTypes`
- `analysisDepth`
- `status`
- `reason`
- `nextAction`
- `needsCrossComponentTrace`
- `evidenceRefs`
- `analyzedAttackChains`
- `nextCandidateTargets`

First-pass rules:

- `rejected` only if blocking evidence is explicit:
  - non-bypassable signature, UID, or cryptographic guard
  - source is not attacker-controlled
  - sink is unreachable from the external entry
  - no visible impact exists
- `candidate` if:
  - the path is suspicious but incomplete
  - the chain crosses trust boundaries
  - the path depends on `PendingIntent`, URI grants, results, Binder handles, or WebView/browser handoff
- `statically-supported` only if the full static chain is already complete

Coverage rules:

- Persist every target to `coverage.json`
- `coverage.json.targets` must follow one stable schema
- `coverage.json` must exhaustively cover `recon.json.targetInventory`

Overview mapping:

| Target type | Overview file | Common entrypoints |
|------------|---------------|--------------------|
| Activity | `references/app-activity.md` | `onCreate`, `onNewIntent`, result callbacks |
| Service | `references/app-service.md` | `onBind`, `onStartCommand`, `handleMessage` |
| Provider | `references/app-provider.md` | `query`, `insert`, `update`, `delete`, `openFile`, `call`, `applyBatch`, `bulkInsert` |
| Receiver | `references/app-broadcast.md` | `onReceive` |
| Intent flow | `references/app-intent.md` | forwarding, parcel, grant, result-return paths |
| WebView host | `references/app-webview.md` | host init, URL loaders, bridge registration, result handlers |

Subagents:

- Component scout: choose likely vuln patterns and entry methods
- Chain subagent: analyze one method at a time
- Coverage steward: verify inventory-to-coverage completeness before Phase 4

Coverage steward contract:

- Input: `recon.json.targetInventory`, `coverage.json.targets`
- Checks:
  - every `targetId` appears exactly once
  - every row has `sourceComponent`, `possibleIssueTypes`, `status`, `reason`, `analysisDepth`, `nextAction`, and `evidenceRefs`
  - every `rejected` row names the blocking condition
  - every `candidate` row names the missing proof
- Output:
  - `inventorySummary.fullyAccountedFor = true` only if all checks pass
  - refreshed `traceSummary` and `inventorySummary`

Tracing command:

```bash
decx code method-source "<currentMethod>" -P <port>
```

Method labels:

- `SOURCE`
- `SINK`
- `SAFE`
- `PASS_THROUGH`
- `DEAD_END`

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

Common hard guards:

- exact package-signature or UID allowlist
- integrity or HMAC tied to unforgeable material
- strict typed parsing plus explicit class allowlist
- `checkSignatures`, `enforceCallingPermission`, `checkCallingPermission`

Trace only tainted-data-relevant calls. Skip logging, pure UI, and obvious dead ends.

Minimum finding fields:

- `vulnType`
- `risk`
- `status`
- `entryPoint`
- `source`
- `sink`
- `callChain`
- `traceSummary`
- `reachable`
- `controllable`
- `guardsChecked`
- `bypassConditions`
- `impactEvidence`
- `ratingRationale`

### Phase 4 - Deep Trace

Goal: fully trace every non-`rejected` target that still lacks a complete chain.

Mandatory deep-trace conditions:

- cross-component chain
- `PendingIntent`, URI grants, results, or scan flows cross trust boundaries
- WebView or browser content turns into native actions
- Binder or Messenger unlocks the real sink
- first-pass verdict is `candidate`

Rules:

- Reuse `nextMethods`
- Inherit the current `chain`
- Keep the same finding schema
- Update `coverage.json` after each deep trace
- Do not stop because there are many targets
- Continue until every non-`rejected` target is either:
  - `statically-supported`
  - `candidate` with explicit missing proof
  - `rejected` with explicit blocking evidence

Persistence:

- Persist `coverage.json` before and during deep tracing
- Persist `findings.json` incrementally
- On resume, load `recon.json`, `coverage.json`, `findings.json`, and `resume.json` first

### Phase 5 - Exploitability Filter

This phase is exploitability triage based on the traced chain, not exploitation proof.

Quick rejection checks:

| Condition | Check |
|----------|-------|
| `signature` / `signatureOrSystem` permission | manifest `protectionLevel` |
| exact signature enforcement | `checkSignatures`, signature compare |
| hard package / UID allowlist | immutable trusted allowlist |
| root / system-only path | privileged-only API or environment |
| non-bypassable guard | integrity, cryptographic, or strict type guard |

Three-factor gate:

1. Reachable
2. Controllable
3. Impactful

Decision rules:

- Reachable + controllable + impactful + explicit bypass conditions + impact evidence: `statically-supported`
- Explicit blocking reason: `rejected`
- Suspicious but incomplete: `candidate`

Residual-risk rules:

- If a target remains suspicious after full tracing, keep it as `candidate`
- The final residual set must be visible in both `coverage.json` and the report
- The report must distinguish:
  - proven blocked
  - statically supported vulnerabilities
  - residual `candidate` targets

### Phase 6 - Report

Goal: generate the final Markdown report using only `statically-supported` findings, while still surfacing residual `candidate` targets in the coverage summary section.

Execution model:

- Create one reporting subagent
- Main agent does not draft the full report body

Reporting steps:

1. Read `assets/report-template.md`
2. Read `references/risk-rating.md`
3. Pick language:
   - `zh` -> `assets/report-template-zh.md`
   - `en` -> `assets/report-template-en.md`
   - `both` -> Chinese first, then English
4. Re-fetch only key component-entry, Binder-bridge, source, sink, and missing-guard locations with `decx code method-source "<method>" -P <port>`
5. Fill the selected template strictly

Mandatory report content:

- `Bypass Conditions / Uncertainties`
- `Visible Impact`
- `Rating Rationale`
- attack-surface coverage summary
- unresolved residual `candidate` targets

Report rules:

- Describe the traced chain directly; do not foreground the analysis method unless you need to explain uncertainty
- Never write "verified", "fully exploited", or "confirmed exploitable in practice"
- If impact is conditional, state the condition directly
- Follow the template strictly
- Do not add extra sections, reorder sections, insert JSON, or include rejected findings
- In `Full Call Chain`, start from the victim app's externally reachable component entrypoint or Binder-exposed method, not from attacker actions
- Never start `Full Call Chain` with `AttackerApp.*`, `bindService`, `startActivity`, `sendBroadcast`, `ContentResolver.*`, adb steps, or PoC driver actions
- Put third-party trigger steps only under `Attack Path -> Exploitation Steps`
- For bound-service and AIDL cases, prefer `VictimService.onBind(Intent):IBinder -> I*.Stub.method(...) -> guarded or vulnerable internal method`
- Every `Full Call Chain` node must be backed by fetched code evidence; if the exact bridge from entrypoint to sink is not proven, keep the target as `candidate` instead of inventing a step
- Every report issue must show the code location of the missing guard, not just describe the condition in prose
- Before finalizing each issue, run this self-check:
  - first call-chain node is in the victim package and is an externally reachable entrypoint
  - attacker actions appear only in `Attack Path`
  - Binder/AIDL reports show both the component entrypoint and the exposed Stub method when both are part of the reachable chain
  - the missing guard location appears in `Code Analysis`

## Handoff

At 60% context usage, hand off immediately:

```json
{
  "handoff": true,
  "phase": "<current phase>",
  "component": "<current component or null during recon>",
  "traceSummary": {
    "sourceComponent": "<current victim entry component or Binder bridge>",
    "possibleIssueTypes": ["<current likely issue types>"],
    "analyzedAttackChains": ["<chains already traced>"],
    "nextCandidateTargets": ["<next targets to analyze>"]
  },
  "port": 31234,
  "done": "<completed work>",
  "next": "<entry instruction for the next subagent>",
  "context": "<minimum context needed to continue>"
}
```

If file-backed continuation is enabled, persist the same state to `resume.json`.

Phase context:

- Phase 2: parsed component list, current source component family, and remaining recon commands
- Phase 3: `coverageDone`, `coveragePending`, current source component, likely issue types, and proven rejection reasons
- Phase 4: cross-component chain progress and next candidate targets
- Phase 6: completed and pending report sections

Resume:

1. Load `resume.json`
2. Verify `target`, `artifactPath`, `sessionName`, and `port`
3. Load `recon.json`, `coverage.json`, and `findings.json`
4. Reconfirm the DECX session with `decx process status -P <port>`
5. Continue from `lastCompletedPhase` and `nextAction`

## Handoff To `decxcli-poc`

Pass only the minimal finding packet plus the current trace focus.

- Fill `poc-handoff.json` from `assets/poc-handoff-template.json`
- Keep one finding per handoff file unless the user explicitly asks for a batch handoff

Never pass:

- large raw source blocks
- full recon output
- unrelated components
- findings below `statically-supported`

## References

- `references/app-activity.md`
- `references/app-intent.md`
- `references/app-broadcast.md`
- `references/app-provider.md`
- `references/app-service.md`
- `references/app-webview.md`
- `references/risk-rating.md`
