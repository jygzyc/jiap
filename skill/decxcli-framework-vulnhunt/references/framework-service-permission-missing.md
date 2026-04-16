# Framework Service - Permission - Missing Enforcement

A framework service performs a privileged action without enforcing the required caller permission. The issue also applies when enforcement is too late or too weak to protect the action.

**Risk: CRITICAL**

## Exploit Prerequisites

The Binder-exposed service method performs a privileged operation and lacks `enforceCallingPermission()` or an equivalent non-bypassable gate before the operation.

**Android Version Scope:** Version-specific. Do not generalize beyond the code being analyzed.

## Bypass Conditions / Uncertainties

- If permission enforcement happens before the privileged action and is bound to the real caller, reject the finding
- `checkCallingOrSelfPermission()` is often weaker than required in framework code; treat self-grant behavior carefully
- Do not claim `CRITICAL` unless the privileged action is real and visible

## Visible Impact

Visible impact must be concrete, such as:

- installing or deleting packages
- modifying protected system settings
- capturing screen contents
- granting or bypassing privileged capabilities

## Attack Flow

```text
1. decx ard system-service-impl "<Interface>" -P <port>
2. decx code class-source "<ServiceImpl>" -P <port>
3. Match each Binder method against its privileged action
4. Confirm whether enforceCallingPermission or equivalent exists before the action
```

## Key Code Patterns

```java
public int getDeviceId() {
    return TelephonyManager.getDefault().getDeviceId();
}
```

```java
public void resetUserConfig(int userId) {
    resetConfig(userId);
    getContext().enforceCallingPermission(
        "android.permission.RESET_USER_CONFIG", null);
}
```

## Secure Pattern

```java
getContext().enforceCallingPermission(
    "android.permission.READ_PHONE_STATE",
    "getDeviceId requires READ_PHONE_STATE");
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + identity confusion | forged user identity crosses users after the missing permission gate | [[framework-service-identity-confusion]] |
| + clear-calling-identity misuse | cleared identity broadens the privileged window | [[framework-service-clear-identity]] |
| + framework redirect | privileged launch path becomes reachable | [[framework-service-intent-redirect]] |

## Related

[[framework-service]]
[[framework-service-identity-confusion]]
[[framework-service-clear-identity]]
[[framework-service-intent-redirect]]
