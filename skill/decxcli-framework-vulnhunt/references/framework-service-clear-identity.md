# Framework Service - Identity - clearCallingIdentity() Misuse

`Binder.clearCallingIdentity()` temporarily drops the caller's identity inside a framework service. If used too broadly or before authorization, attacker-triggered work may run with privileged identity.

**Risk: HIGH**

## Exploit Prerequisites

The framework service accepts attacker-triggerable input, calls `clearCallingIdentity()`, and performs sensitive work inside the cleared block without completing all required authorization checks beforehand.

**Android Version Scope:** Historically more common before newer helper patterns such as `Binder.withCleanCallingIdentity()`, but still relevant wherever manual clear/restore logic exists.

## Bypass Conditions / Uncertainties

- Reject the finding if all authorization and target validation are completed before identity is cleared
- Reject the finding if the cleared block contains only harmless or unavoidable low-risk work
- If `restoreCallingIdentity()` is correctly fenced and the privileged window is minimal, downgrade or reject depending on remaining impact

## Visible Impact

Visible impact must be concrete, such as:

- deleting or modifying protected system or app data
- running a privileged file, package, or settings operation
- launching a privileged redirect chain under system identity

## Attack Flow

```text
1. decx ard system-service-impl "<Interface>" -P <port>
2. decx code class-source "<ServiceImpl>" -P <port>
3. Trace clearCallingIdentity() and restoreCallingIdentity()
4. decx code method-cfg "<methodWithClearedIdentity>" -P <port>
   -> visualize the cleared block to identify all bypass paths
5. Inspect the code between them
6. Confirm whether sensitive work occurs there without prior strong authorization
```

## Key Code Patterns

- broad cleared block containing attacker-controlled branching or sensitive operations

```java
long token = Binder.clearCallingIdentity();
try {
    if (action.equals("delete_all")) {
        deleteAllUserData();
    }
} finally {
    Binder.restoreCallingIdentity(token);
}
```

## Secure Pattern

```java
enforceCallingPermission(android.Manifest.permission.MANAGE_USERS, "Need MANAGE_USERS");
Binder.withCleanCallingIdentity(() -> doPrivilegedOperation(action));
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + missing framework permission enforcement | privileged block contains no real caller gate | [[framework-service-permission-missing]] |
| + identity confusion | cached or attacker-influenced identity is used around the cleared block | [[framework-service-identity-confusion]] |
| + framework redirect | privileged identity launches an attacker-controlled target | [[framework-service-intent-redirect]] |
| + framework race condition | privilege window becomes unstable across concurrent requests | [[framework-service-race-condition]] |

## Related

[[framework-service]]
[[framework-service-permission-missing]]
[[framework-service-identity-confusion]]
[[framework-service-intent-redirect]]
[[framework-service-race-condition]]
