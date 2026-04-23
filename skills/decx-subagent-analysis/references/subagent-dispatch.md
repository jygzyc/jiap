# DECX Subagent Dispatch Reference

Use this reference when the controller needs a precise, repeatable subagent handoff.

## Dispatch Principles

- One subagent owns one narrow target or one narrow review pass.
- Do not let multiple subagents write the same finding block at the same time.
- The controller keeps session ownership and final judgment.
- Subagents return structured summaries, not raw command dumps.

## Required Dispatch Fields

Every DECX subagent task should include:

- role name
- target type
- DECX port
- active DECX skill
- files allowed to read
- files allowed to write
- stop condition
- return format

## Recommended Roles

- `decx-recon`: build `recon.json` and initial inventory artifacts
- `decx-trace`: trace one retained target or one method chain
- `decx-coverage`: verify `recon.json` and `coverage.json` alignment
- `decx-report`: build the final Markdown report from supported findings

## Prompt Skeleton

```text
Role: decx-trace
Target: one exported activity chain
Port: <port>
Skill: decxcli-app-vulnhunt
Read: .decx-analysis/<target>/recon.json, .decx-analysis/<target>/coverage.json
Write: .decx-analysis/<target>/coverage.json
Stop when: the assigned target has a justified state and updated trace summary
Return: files updated, state, call chain, missing proof, open questions
```

## Fallback

If the host cannot spawn subagents, reuse the same role boundaries in the main agent and execute them sequentially.
