# Framework Service - Identity - Confusion

A framework service trusts caller-supplied identity fields such as `userId` or `uid`. If it does not derive identity from Binder, an attacker can act for another user or app.

**Risk: HIGH to CRITICAL**

## Exploit Prerequisites

The service accepts identity-related parameters and uses them for authorization or data selection without verifying them against `Binder.getCallingUid()` / `UserHandle.getUserId()`.

**Android Version Scope:** Version-specific. Rate only the concrete service implementation under review.

## Bypass Conditions / Uncertainties

- If the service derives identity exclusively from Binder and treats caller-supplied IDs as untrusted hints, reject the finding
- Cross-user impact requires multi-user, work-profile, or similar user separation to matter
- Escalate toward `CRITICAL` only when the confused identity leads to privileged or cross-user compromise

## Visible Impact

Visible impact must be concrete, such as:

- reading another user's data
- modifying another profile's settings
- performing a privileged action as `system` or another user context

## Attack Flow

```text
1. decx ard system-service-impl "<Interface>" -P <port>
2. Inspect Binder methods that accept userId / uid / package identity fields
3. Confirm whether authorization uses the caller-supplied value instead of Binder-derived identity
4. Confirm the resulting cross-user or privilege effect is meaningful
```

## Key Code Patterns

```java
public void deleteUserFile(int userId, String filename) {
    File file = new File(getUserDir(userId), filename);
    file.delete();
}
```

## Secure Pattern

```java
int callingUid = Binder.getCallingUid();
int callingUserId = UserHandle.getUserId(callingUid);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + missing permission enforcement | fake identity crosses an already-weak permission boundary | [[framework-service-permission-missing]] |
| + clear-calling-identity misuse | cached or cleared identity is applied incorrectly | [[framework-service-clear-identity]] |
| + framework data leak | fake user selection returns another user's data | [[framework-service-data-leak]] |

## Related

[[framework-service]]
[[framework-service-permission-missing]]
[[framework-service-clear-identity]]
[[framework-service-data-leak]]
