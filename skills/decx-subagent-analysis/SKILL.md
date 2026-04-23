---
name: decx-subagent-analysis
description: Controller skill for DECX analyses that should be split into recon, trace, review, or PoC subagents while keeping one DECX session and one artifact workspace.
metadata:
  requires:
    bins: ["decx"]
---

# DECX Subagent Analysis

Use this skill only when the task benefits from splitting work into independent phases or targets.

This skill is the controller layer. It does not replace:

- `decxcli`
- `decxcli-app-vulnhunt`
- `decxcli-framework-vulnhunt`
- `decxcli-poc`

It coordinates them.

For reusable dispatch patterns and Codex-oriented subagent notes, see `references/subagent-dispatch.md`.

## When To Use

Use this skill when at least one of these is true:

- the task has a clear recon phase and a separate deep-trace phase
- multiple independent targets can be traced in parallel
- one agent should review evidence produced by another
- a supported finding needs a separate PoC step

Do not use this skill for simple one-shot lookups or a single direct DECX command. Use `decxcli` for that.

## Controller Responsibilities

- Choose the correct underlying DECX skill first.
- Open or reuse exactly one DECX session for the active target.
- Create and maintain one work directory under `.decx-analysis/<target-name>/`.
- Dispatch narrow subagent tasks with explicit write boundaries.
- Review subagent outputs before moving to the next phase.
- Keep final control of session close and final answer.

## Skill Routing

- For APK surfaces, use `decxcli-app-vulnhunt`.
- For framework, Binder, AIDL, `system_server`, or OEM service work, use `decxcli-framework-vulnhunt`.
- For PoC implementation, use `decxcli-poc`.
- For general source navigation outside hunting, use `decxcli`.

## Shared Rules

- Every session-backed `decx code` and `decx ard` command must include `-P <port>`.
- `decx ard system-services` and `decx ard perm-info` are adb-backed and do not use `-P <port>`.
- If a command is missing, rejected, or unclear, run the nearest `--help` command before retrying.
- Keep one DECX session per target unless the user explicitly changes target.
- Keep raw source small; store structured outputs in artifacts instead of pasting large dumps.
- Do not let subagents close the DECX session unless the assigned task is the final PoC cleanup step.

## Default Artifact Layout

Use:

```text
.decx-analysis/<target-name>/
```

Common files:

- `recon.json`
- `coverage.json`
- `shortlist.json`
- `findings.json`
- `report.md`
- `resume.json`
- `poc-handoff.json`

Only create the files needed by the active path.

## Dispatch Model

### Recon Agent

Use one recon agent first when the target surface is still being enumerated.

Allowed work:

- enumerate attack surface
- write `recon.json`
- write `coverage.json` for app hunts or `shortlist.json` for framework hunts

Do not let the main agent duplicate Phase 2 DECX commands unless the recon agent is unavailable.

### Trace Agent

Use one trace agent per independent target or chain.

Good splits:

- one exported component
- one WebView host
- one Binder service family
- one method chain

Bad splits:

- multiple agents writing the same finding block
- multiple agents tracing the same chain from different starting points without coordination

### Review Agent

Use one review agent after recon or trace work produced enough evidence to judge completeness or reportability.

Allowed work:

- verify coverage completeness
- check evidence quality
- identify unsupported claims
- suggest downgrade from `statically-supported` to `candidate` or `rejected`

### PoC Agent

Use one PoC agent only after there is one active supported finding and the user wants a PoC.

Allowed work:

- normalize the finding
- re-verify the DECX path
- build or update one `poc-<target>` project

## Required Dispatch Contract

Every subagent task must include:

- target type
- session port
- active underlying DECX skill
- allowed files to read
- allowed files to write
- exact stop condition
- expected return shape

Use prompts like:

```text
Role: decx-recon
Target: APK app hunt
Port: <port>
Skill: decxcli-app-vulnhunt
Write only: .decx-analysis/<target>/recon.json
Stop when: full attack-surface inventory is written
Return: files written, targets found, unresolved questions
```

## Phase Order

Default order:

1. controller opens or reuses the session
2. controller creates `.decx-analysis/<target-name>/`
3. recon agent writes inventory artifacts
4. trace agents analyze independent retained targets
5. review agent checks coverage or evidence quality
6. controller updates `findings.json`, `resume.json`, and `report.md`
7. optional PoC agent runs from one supported finding
8. controller closes the DECX session only when no downstream work remains

## Fallback

If the host does not support subagents, keep the same phase order and artifact contract, but execute the phases sequentially in the main agent.
