# Framework Vulnerability Research Workflow

**Scope**: Android Framework (`framework.jar`, `services.jar`), System Server, and OEM modifications
**Focus**: Privilege escalation, Binder IPC security, and denial-of-service vectors

## Phase 1: Target Acquisition

### Step 1: Interface Mapping

The primary method for framework analysis is mapping AIDL interfaces to their implementations.

```
get_system_service_impl("android.os.IPowerManager")
```

**Example Results**:
- `android.os.IPowerManager` → `com.android.server.power.PowerManagerService`
- `android.app.IActivityManager` → `com.android.server.am.ActivityManagerService`

### Step 2: Method Profiling

```
get_class_info("com.android.server.power.PowerManagerService")
```

Identify public methods in Stub/Proxy implementation. Focus on methods that:
- Accept Binder transaction data from untrusted callers
- Perform privileged operations
- Access sensitive data

### Step 3: Interface Discovery (If Unknown)

```
get_class_source("android.os.ITargetService")
search_method("asInterface")
get_implement("android.os.ITargetService")
```

---

## Vulnerability Detection Protocols

### 1. Permission Bypass ("Forgotten Check")

**Reference**: CVE-2023-40094 (Keyguard Unlock)

**Description**: `keyguardGoingAway()` in `IKeyguardService` allowed any app to unlock device due to missing permission check.

**Detection Steps**:

1. **Locate Permission Checks**:
   ```
   search_class_key("enforceCallingPermission")
   search_class_key("checkCallingPermission")
   search_class_key("enforceCallingOrSelfPermission")
   ```

2. **Verify Ordering** (CRITICAL):
   ```
   get_method_source("com.android.server.Service.targetMethod")
   ```
   
   Find `Binder.clearCallingIdentity()` calls and verify:
   - Permission check **MUST** occur **BEFORE** `clearCallingIdentity()`
   
   **Vulnerable Pattern**:
   ```java
   public void sensitiveOperation() {
       long token = Binder.clearCallingIdentity();  // Identity cleared FIRST
       try {
           // Privileged work here
       } finally {
           Binder.restoreCallingIdentity(token);
       }
       // Permission check AFTER - TOO LATE!
       enforceCallingPermission(PERMISSION, "message");
   }
   ```

   **Secure Pattern**:
   ```java
   public void sensitiveOperation() {
       enforceCallingPermission(PERMISSION, "message");  // Check FIRST
       long token = Binder.clearCallingIdentity();
       try {
           // Privileged work here
       } finally {
           Binder.restoreCallingIdentity(token);
       }
   }
   ```

3. **Check System Restriction**:
   - Does method validate `Binder.getCallingUid()` against system UIDs (1000)?
   - Is there a `Binder.getCallingPid()` check?

**Vulnerability Criteria**:
- **CRITICAL**: No permission check at all on privileged operation
- **HIGH**: Permission check after `clearCallingIdentity()`
- **MEDIUM**: Check exists but uses wrong permission

**Reporting Template**:
```
[CRITICAL] Permission Bypass in PowerManagerService.forceReboot()
Risk: CRITICAL
Component: com.android.server.power.PowerManagerService.forceReboot()
Cause: Missing permission check allows unprivileged callers to reboot device
Evidence:
  public void forceReboot() {
      // No enforceCallingPermission() call
      mPowerManagerInternal.reboot(false);
  }
Mitigation: Add enforceCallingPermission(REBOOT, "forceReboot") at method entry
```

---

### 2. Identity Confusion (UID/PID Spoofing)

**Reference**: CVE-2020-0108

**Description**: Method trusts `int uid` parameter instead of validating Binder caller identity.

**Detection Steps**:

1. **Identify Risky Signatures**:
   ```
   search_method("int uid")
   search_method("int callingUid")
   search_method("int pid")
   ```
   Look for methods accepting `uid` or `pid` as parameters.

2. **Analyze Usage**:
   ```
   get_method_source("com.android.server.Service.methodWithUid")
   ```

   **Vulnerable Pattern**:
   ```java
   public Data getDataForUser(int uid) {
       // Trusts caller-supplied uid directly
       return mDatabase.getDataForUid(uid);  // VULNERABLE
   }
   ```

   **Secure Pattern**:
   ```java
   public Data getDataForUser(int uid) {
       // Validates against actual caller
       if (uid != Binder.getCallingUid()) {
           throw new SecurityException("UID mismatch");
       }
       return mDatabase.getDataForUid(uid);
   }
   ```

3. **Check Validation**:
   - `uid != Binder.getCallingUid()` → Secure
   - `UserHandle.getAppId(uid) != UserHandle.getAppId(Binder.getCallingUid())` → Secure
   - No validation → Vulnerable

4. **PendingIntent Check**:
   Ensure base identity isn't confused when creating/sending PendingIntents.

**Reporting Template**:
```
[HIGH] Identity Confusion in DataService.getDataForUser()
Risk: HIGH
Component: com.android.server.data.DataService.getDataForUser(int uid)
Cause: Trusts user-supplied UID for access control. Attacker can spoof identity.
Evidence:
  public Data getDataForUser(int uid) {
      return mDatabase.getDataForUid(uid);  // No Binder.getCallingUid() check
  }
Mitigation: Compare uid parameter against Binder.getCallingUid() before access
```

---

### 3. Side Channel & Error Oracles

**Reference**: CVE-2021-0321 (Package Existence Leak)

**Description**: `getPackageUid()` returned different responses (silent vs exception) leaking package installation status.

**Detection Steps**:

1. **Identify Early Returns**:
   ```
   get_method_source("com.android.server.pm.PackageManagerService.getPackageUid")
   ```
   Look for methods returning immediately based on state.

2. **Check Permission Consistency**:
   - Does method perform permission checks on **ALL** code paths?
   
   **Vulnerable Pattern**:
   ```java
   public int getPackageUid(String packageName) {
       PackageInfo info = mPackages.get(packageName);
       if (info == null) {
           return INVALID_UID;  // Path 1: Silent failure, no permission check
       }
       enforceCallingPermission(QUERY_PACKAGES);  // Path 2: Permission required
       return info.uid;
   }
   ```

3. **Information Leakage Vectors**:
   
   | Vector | Description | Detection |
   |--------|-------------|-----------|
   | Exception Type | Different exceptions reveal state | Compare catch blocks |
   | Return Values | `null` vs `INVALID_UID` vs actual data | Check return patterns |
   | Timing | Fast vs slow response | Rarely detectable in static analysis |
   | Boolean Returns | `true`/`false` leaks existence | Check return logic |

**Reporting Template**:
```
[MEDIUM] Side Channel in PackageManagerService.getPackageInfo()
Risk: MEDIUM
Component: com.android.server.pm.PackageManagerService.getPackageInfo()
Cause: Information leak via error oracle. Different behaviors reveal package state.
Evidence:
  if (pkg == null) return null;  // No permission check
  enforcePermission();           // Permission required only for existing packages
Impact: Attacker can determine which packages are installed
Mitigation: Ensure consistent permission enforcement on all return paths
```

---

### 4. Data Exposure (Sensitive Info Access)

**Reference**: APIs returning foreground app, usage stats, or location

**Detection Steps**:

1. **Identify Sensitive Returns**:
   ```
   search_method("getRunningAppProcesses")
   search_method("getRecentTasks")
   search_method("getLastLocation")
   search_class_key("RunningAppProcessInfo")
   ```

2. **Check Data Sensitivity**:
   - `RunningAppProcessInfo` → Foreground app detection
   - Location coordinates, WiFi BSSIDs → User tracking
   - Usage statistics → App behavior profiling

3. **Verify Protection**:
   
   | Protection | Check |
   |------------|-------|
   | Permission | `enforceCallingPermission()` present? |
   | System-Only | Restricted to `SystemUID` or `ShellUID`? |
   | Obfuscation | Is sensitive data anonymized/blurred? |

**Reporting Template**:
```
[MEDIUM] Data Exposure in ActivityManagerService.getRecentTasks()
Risk: MEDIUM
Component: com.android.server.am.ActivityManagerService.getRecentTasks()
Cause: Returns user's recent app history which may violate privacy
Evidence: Returns List<RecentTaskInfo> without sufficient restrictions
Impact: Attacker can profile user behavior by monitoring recent tasks
Mitigation: Add permission requirement (e.g., GET_TASKS) or restrict to system callers
```

---

### 5. Race Conditions in Binder Calls

**Detection Steps**:

1. **Identify State Checks Followed by Actions**:
   ```java
   if (isAllowed()) {      // Check
       performAction();     // Use - TOCTOU race
   }
   ```

2. **Look for Missing Synchronization**:
   - Is `synchronized` keyword used?
   - Are atomic operations used for state changes?

---

## Tool Quick Reference

| Tool | Purpose | Example |
|------|---------|---------|
| `get_system_service_impl(interface)` | **PRIMARY**: Map interface to implementation | Start here |
| `get_method_source(signature)` | **CORE**: Deep dive into logic | Vulnerability analysis |
| `get_class_info(class)` | List methods and fields | Method enumeration |
| `search_method(name)` | **DISCOVERY**: Find methods across services | Pattern hunting |
| `get_method_xref(signature)` | **FLOW**: Trace how methods are called | Call chain analysis |
| `get_sub_classes(class)` | **HIERARCHY**: Find implementations | Service variants |
| `get_implement(interface)` | Find interface implementations | Alternative services |

## Analysis Checklist

- [ ] Interface mapped to implementation class
- [ ] Public Binder methods enumerated
- [ ] Permission checks verified (present and correctly ordered)
- [ ] UID/PID parameter handling audited
- [ ] Error handling consistency checked
- [ ] Sensitive data returns identified
- [ ] Loop/allocation bounds verified
- [ ] Race conditions considered

## Reporting Guidelines

### Risk Matrix

| Severity | Criteria | Examples |
|----------|----------|----------|
| **CRITICAL** | Remote code execution, full privilege escalation | Missing permission on reboot |
| **HIGH** | Privilege escalation, data theft, system crash | Identity confusion, DoS |
| **MEDIUM** | Information leakage, privacy violation | Side channel, data exposure |
| **LOW** | Minor logic errors, unlikely exploitable | Edge case race conditions |

### Report Structure

1. **Summary**: Target service, methods analyzed, key findings count
2. **Findings**: Using templates above with evidence
3. **Risk Matrix**: Categorized by severity
4. **Mitigation Recommendations**: Concrete code changes
