# Framework Vulnerability Research Workflow

**Scope**: Android Framework (`framework.jar`, `services.jar`), System Server, OEM mods
**Focus**: Privilege escalation, Binder IPC security, DoS

## Core Principle: Exploitability Only

**Report ONLY findings with demonstrable exploitability.**

### Vulnerability = Code Flaw + Exploit Path

Before reporting, verify:
1. **Reachability**: Can third-party app trigger this?
2. **Control**: Can unprivileged caller influence behavior?
3. **Impact**: Privilege escalation, data theft, system crash?

If ANY answer is NO → **Do NOT report**.

---

## Tool Quick Reference

| Tool | Purpose |
|------|---------|
| `get_system_service_impl(interface)` | Map AIDL interface to implementation |
| `get_method_source(signature)` | Read method logic |
| `search_method(name)` | Find methods across services |
| `get_method_xref(signature)` | Trace method usage |

## Reporting Template

```
[SEVERITY] Title
Risk: CRITICAL/HIGH/MEDIUM
Component: class.method()
Cause: Brief description
Evidence:
  // Code snippet
Exploit Path:
  1. Third-party app calls method
  2. Attacker passes X
  3. Triggers Y
  4. Achieves Z
Mitigation: Fix
```
