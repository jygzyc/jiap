---
name: using-decx
description: Use when starting any DECX-related conversation. Establishes how to find and use DECX skills before any DECX action, including clarifying questions.
metadata:
  requires:
    bins: ["decx"]
---

If you were dispatched as a subagent to execute a specific DECX task, skip this skill.

If you think there is even a 1% chance a DECX skill might apply to what you are doing, you MUST invoke the relevant DECX skill first.

If a DECX skill applies, use it. Do not freehand around it.

## Instruction Priority

DECX skills override default behavior, but user instructions still take precedence:

1. User instructions and repo instructions such as `AGENTS.md`
2. DECX skills
3. Default system behavior

## How To Access Skills

Use your host skill tool to invoke the skill directly.

- In Codex, use the skill system and invoke the installed DECX skill.
- If a host injects a DECX bootstrap hook, treat it only as a reminder to use the skill.
- Do not rely on hooks for correctness. DECX rules live in the skills.

## Platform Adaptation

DECX skills are written in a host-neutral style.

- For Codex tool mapping and subagent equivalents, see `references/codex-tools.md`.
- Keep this skill focused on routing and rules; use references for heavier platform details.

# Using DECX Skills

## The Rule

Invoke the relevant DECX skill before any DECX response or action.

That includes:

- clarifying questions
- command discovery
- codebase exploration
- session checks
- choosing app versus framework analysis

If the skill turns out not to apply after reading it, you can switch. The skill check still comes first.

## DECX Skill Routing

- Use `decxcli` for APK/DEX/JAR navigation, source lookup, xrefs, manifests, resources, and session management.
- Use `decxcli-app-vulnhunt` for APK attack-surface and vulnerability hunting.
- Use `decxcli-framework-vulnhunt` for Binder, AIDL, `system_server`, vendor, and OEM framework work.
- Use `decxcli-poc` only after you already have one supported finding.
- Use `decx-subagent-analysis` when the work should be split into recon, trace, review, or PoC phases.

## DECX Red Flags

These thoughts mean stop and use the correct DECX skill first:

- "I just need to inspect the APK quickly"
- "I need one command before I decide"
- "Let me check the session first"
- "This is only a question, not analysis yet"
- "I can search the codebase before deciding"
- "I remember how the skill works already"

## DECX Rules

- If a command is missing, rejected, or unclear, run the nearest `--help` command before retrying.
- Session-backed `decx code` and `decx ard` commands need `-P <port>`.
- `decx ard system-services` and `decx ard perm-info` are adb-backed and use `--serial` / `--adb-path` instead of `-P <port>`.
- Keep one DECX session per target unless the user explicitly changes target.
- Keep artifacts under `.decx-analysis/<target-name>/` when work may continue later.
- Close the DECX session only when the current workflow is done and no downstream DECX work remains.

## Subagent Policy

- When the host supports subagents and the task has independent phases, dispatch them through `decx-subagent-analysis`.
- The main agent stays the controller: it opens or reuses the session, assigns narrow tasks, reviews returned artifacts, and handles final verification.
- If subagents are unavailable, run the same phases sequentially and keep the same artifact contract.
