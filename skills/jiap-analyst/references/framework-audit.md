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

## 1. Permission Bypass

**Vulnerability**: Missing or misordered permission check on privileged operation

**Detection**:
```
search_class_key("enforceCallingPermission")
search_class_key("checkCallingPermission")
get_method_source("com.android.server.Service.method")
```

**Check Order**: Permission check MUST be BEFORE `Binder.clearCallingIdentity()`

**Vulnerable Pattern**:
```java
public void sensitiveOperation() {
    long token = Binder.clearCallingIdentity();  // Cleared FIRST
    try {
        privilegedWork();  // Already executed
    } finally {
        Binder.restoreCallingIdentity(token);
    }
    enforcePermission();  // Check AFTER - too late
}
```

**Report IF**:
- ✅ No permission check on privileged Binder-accessible method → **CRITICAL**
- ✅ Permission check AFTER `clearCallingIdentity()` → **HIGH**
- ❌ Check BEFORE `clearCallingIdentity()` → **NOT A VULNERABILITY**
- ❌ Uses wrong but still restrictive permission → **NOT A VULNERABILITY**

## 2. Identity Confusion (UID Spoofing)

**Vulnerability**: Method trusts caller-supplied `uid` parameter instead of actual Binder caller

**Detection**:
```
search_method("int uid")
get_method_source("methodWithUid")
```

**Vulnerable Pattern**:
```java
public Data getDataForUser(int uid) {
    return mDatabase.getDataForUid(uid);  // Trusts parameter blindly
}
```

**Report IF**:
- ✅ Method accepts `int uid`/`int pid` parameter AND uses it for access control WITHOUT validating against `Binder.getCallingUid()` → **HIGH**
- ❌ Validates `uid == Binder.getCallingUid()` or similar → **NOT A VULNERABILITY**
- ❌ Method not exposed via Binder → **NOT A VULNERABILITY**

## 3. Side Channel (Information Leak)

**Vulnerability**: Different return paths leak sensitive information without permission

**Detection**:
```
get_method_source("methodReturningData")
```

**Vulnerable Pattern**:
```java
public PackageInfo getPackageInfo(String pkg) {
    if (pkg == null) return null;  // No permission check
    enforcePermission(QUERY);     // Permission required only on this path
    return getPackage(pkg);
}
```

**Report IF**:
- ✅ Different return paths (null/exception vs data) reveal sensitive state (package exists, user present) AND observable without permission → **MEDIUM**
- ✅ Information reveals user presence or sensitive configuration → **HIGH**
- ❌ Consistent permission on ALL paths → **NOT A VULNERABILITY**
- ❌ Information not sensitive (non-PII data) → **NOT A VULNERABILITY**
- ❌ Requires precise timing not practically exploitable → **NOT A VULNERABILITY**

## 4. Data Exposure

**Vulnerability**: Returns sensitive data without proper permission

**Detection**:
```
search_method("getRunningAppProcesses")
search_method("getRecentTasks")
search_method("getLastLocation")
search_class_key("RunningAppProcessInfo")
```

**Report IF**:
- ✅ Returns sensitive data (foreground app, location, PII) without proper permission → **HIGH**
- ✅ Returns sensitive data but requires signature-level permission (hard for attackers) → **MEDIUM**
- ❌ Requires system-level permission → **NOT A VULNERABILITY**
- ❌ Data is anonymized/redacted → **NOT A VULNERABILITY**
- ❌ Data is not sensitive (generic stats) → **NOT A VULNERABILITY**

## 5. Race Condition

**Vulnerability**: Check-use TOCTOU race on security-critical state

**Detection**:
```
get_method_source("method")
// Look for:
if (isAllowed()) { performAction(); }
```

**Report IF**:
- ✅ Check-use race on security-critical state (permission, lock) AND attacker can influence between check and use → **HIGH**
- ✅ Race causes crash or unexpected behavior attacker can trigger → **MEDIUM**
- ❌ State check and use are atomic/synchronized → **NOT A VULNERABILITY**
- ❌ Race only affects non-critical functionality → **NOT A VULNERABILITY**
- ❌ Timing window extremely narrow/improbable → **NOT A VULNERABILITY**

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
