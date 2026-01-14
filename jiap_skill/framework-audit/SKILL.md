# JIAP Framework Auditor

**Scope**: Android Framework (`framework.jar`, `services.jar`), System Server, and OEM modifications.
**Focus**: Privilege escalation, Binder IPC security, and denial-of-service vectors.

## Analysis Framework

### Phase 1: Target Acquisition

1. **Interface Mapping**:
   - **Primary Tool**: `get_system_service_impl(interface_name)`
     - Example: `android.os.IPowerManager` → `com.android.server.power.PowerManagerService`
   - **Discovery**: If interface unknown, first `get_class_source(interface_name)`.

2. **Method Profiling**:
   - Identify public methods in Stub/Proxy implementation.
   - Focus on methods accepting Binder transaction data from untrusted callers.

---

## Specialized Vulnerability Protocols

### 1. Permission Bypass ("Forgotten Check")
*Reference: CVE-2023-40094 (Keyguard Unlock)*

**Case Study**: `keyguardGoingAway()` in `IKeyguardService` allowed any app to unlock device.

**Detection Protocol**:
1. **Locate Check**: Search for `enforceCallingPermission(...)` or `checkCallingPermission(...)`.
2. **Verify Ordering**:
   - Find `Binder.clearCallingIdentity()` calls.
   - **CRITICAL**: Permission check MUST occur BEFORE `clearCallingIdentity`.
   - **Vulnerable Pattern**: `clearCallingIdentity()` → [privileged work] → `enforceCallingPermission()`
3. **Check System Restriction**:
   - Does method validate `Binder.getCallingUid()` against system UIDs (1000)?

**Reporting Template**:
```
[VULNERABLE] Method: X()
Risk: HIGH
Cause: Missing permission check allows unprivileged callers to [action].
Evidence: [Code snippet]
Mitigation: Add enforceCallingPermission(PERMISSION_NAME, "action") at entry.
```

### 2. Identity Confusion (UID/PID Spoofing)
*Reference: Trusting `int uid` arguments instead of Binder identity.*

**Case Study**: Method taking `int uid` parameter and using it for access control decisions.

**Detection Protocol**:
1. **Identify Arguments**: Methods accepting `int uid` or `int pid` as parameters.
2. **Analyze Usage**:
   - **Vulnerable**: Uses argument `uid` directly in:
     - Permission checks: `checkPermission(uid, ...)`
     - Data access: `mDb.getData(uid, ...)`
   - **Secure**: Validates against caller:
     - `if (uid != Binder.getCallingUid()) throw new SecurityException()`
     - `if (UserHandle.getAppId(uid) != UserHandle.getAppId(Binder.getCallingUid()))`
3. **PendingIntent Check**: Ensure base identity isn't confused when creating/sending PendingIntents.

**Reporting Template**:
```
[VULNERABLE] Method: X(int uid)
Risk: HIGH
Cause: Trusts user-supplied UID for access control. Attacker can spoof identity.
Evidence: Uses uid parameter without Binder.getCallingUid() validation.
Mitigation: Compare uid parameter against Binder.getCallingUid() before access.
```

### 3. Side Channel & Error Oracles
*Reference: CVE-2021-0321 (Package Existence Leak)*

**Case Study**: `getPackageUid()` returned `INVALID_UID` (silent) vs `SecurityException` (for non-matching UID), leaking package installation status.

**Detection Protocol**:
1. **Identify Early Returns**: Methods returning immediately based on state.
2. **Check Permission Consistency**:
   - Does method perform permission checks on ALL code paths?
   - **Vulnerable**: One path returns data/error WITHOUT permission check, another path throws `SecurityException`.
3. **Information Leakage**:
   - Can sensitive data be inferred from:
     - Different error types (`Exception` vs `null` vs `INVALID_UID`)
     - Timing differences
     - Boolean return values

**Reporting Template**:
```
[VULNERABLE] Method: X()
Risk: MEDIUM
Cause: Information leak via error oracle. Different behaviors on [state A] vs [state B] leak sensitive info.
Evidence: Returns INVALID_UID without check, throws SecurityException otherwise.
Mitigation: Ensure consistent permission enforcement on all return paths.
```

### 4. Data Exposure (Sensitive Info Access)
*Reference: APIs returning foreground app, usage stats, or location.*

**Detection Protocol**:
1. **Check Data Sensitivity**: Does method return:
   - `RunningAppProcessInfo` (Foreground app detection)?
   - Location coordinates, WiFi BSSIDs?
   - Usage statistics (app launch counts)?
2. **Verify Protection**:
   - **Permission Check**: Is caller required to have appropriate permission?
   - **System-Only**: Is method restricted to `SystemUID` or `ShellUID`?
   - **Blur/Obfuscation**: Is sensitive data anonymized?

**Reporting Template**:
```
[ISSUE] Method: X() returns Sensitive Data
Risk: MEDIUM
Cause: Returns [data type] which may violate user privacy.
Evidence: Returns [data structure] without sufficient restrictions.
Mitigation: Add permission requirement (e.g., REAL_GET_TASKS) or restrict to system callers.
```

### 5. System Denial of Service (DoS)
*Reference: Unbounded loops/allocations triggered by unprivileged callers.*

**Detection Protocol**:
1. **Loop Analysis**:
   - Search for `for (int i=0; i<size; i++)` where `size` comes from Binder call.
   - Check for hardcoded limits (e.g., `if (size > 1000) throw`).
2. **Allocation Analysis**:
   - Search for `new byte[size]`, `new int[size]` with user-controlled `size`.
   - Check for OOM protection.
3. **Binder Death Handling**:
   - Does service catch `RemoteException`?
   - Are callback registrations properly unlinked to prevent memory leaks?

**Reporting Template**:
```
[VULNERABLE] Method: X(int size)
Risk: HIGH (DoS)
Cause: Unbounded allocation/iteration allows unprivileged caller to crash service.
Evidence: Allocates new byte[size] without limit check.
Mitigation: Add strict bounds validation: if (size > MAX_SIZE) throw.
```

---

## Tool Reference

| Tool | Usage | Context |
|-------|---------|----------|
| `get_system_service_impl(interface)` | **PRIMARY** | Maps `IInterface` → `*Service` class |
| `get_method_source(signature)` | **CORE** | Deep dive into implementation logic |
| `search_method(name)` | **DISCOVERY** | Find methods across all services |
| `get_method_xref(signature)` | **FLOW** | Trace how methods are called |
| `get_sub_classes(class)` | **HIERARCHY** | Find service implementations |

## Reporting Guidelines

When completing analysis, provide:

1. **Summary**: Target service, methods analyzed, and key findings.
2. **Risk Matrix**:
   - **High**: Privilege escalation, data theft, system crash.
   - **Medium**: Information leakage, privacy violation.
   - **Low**: Minor logic errors, unlikely exploitable.
3. **Evidence**: Code snippets with line numbers (or method signatures).
4. **Mitigation**: Concrete code changes (add permission check, validate argument, etc.).
