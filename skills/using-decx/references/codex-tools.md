# Codex Tool Mapping For DECX Skills

DECX skills are written to be portable across agent hosts. When following them in Codex, use these equivalents:

| Skill wording | Codex equivalent |
|---|---|
| invoke a skill | use the installed skill system |
| dispatch a subagent | `spawn_agent` |
| wait for a subagent result | `wait_agent` |
| send more context to a subagent | `send_input` |
| close a finished subagent | `close_agent` |
| keep a task checklist | `update_plan` when available |
| run a command | use the native shell command tool |

## DECX Notes

- Session-backed `decx code` and `decx ard` commands need `-P <port>`.
- `decx ard system-services` and `decx ard perm-info` are adb-backed and do not use `-P <port>`.
- If subagents are unavailable in the host, run the same DECX phases sequentially and keep the same artifact contract.
