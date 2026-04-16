# Framework Service - Data - Leak

A framework service returns privileged or cross-app data to an untrusted caller. This happens when permission enforcement or caller validation is missing, late, or inconsistent.

**Risk: HIGH**

## Exploit Prerequisites

The Binder-exposed method returns sensitive system, device, user, or cross-app data and lacks a strong caller check.

**Android Version Scope:** Version-specific.

## Bypass Conditions / Uncertainties

- If the returned data is already public or harmless, reject the finding
- If a non-bypassable permission or caller check protects the method, reject the finding
- Device identifiers, account state, installed-app metadata, and cross-user records are all valid sensitive outputs when access should have been restricted

## Visible Impact

Visible impact must be concrete, such as:

- leaking device identifiers
- leaking precise location or account state
- revealing other users' or other apps' private data

## Attack Flow

```text
1. decx ard system-service-impl "<Interface>" -P <port>
2. Inspect return-bearing Binder methods
3. Confirm the returned fields are sensitive
4. Confirm there is no strong caller or permission gate
```

## Key Code Patterns

```java
public Bundle getDeviceInfo() {
    Bundle info = new Bundle();
    info.putString("imei", getImei());
    return info;
}
```

## Secure Pattern

```java
getContext().enforceCallingPermission(
    "android.permission.READ_PRIVILEGED_DEVICE_INFO",
    "Requires privileged device-info access");
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + implicit Intent hijack | leaked package/component info improves targeting | [[app-intent-implicit-hijack]] |
| + framework redirect | leaked privileged targets enable redirect chains | [[framework-service-intent-redirect]] |
| + identity confusion | fake identity returns another user's data | [[framework-service-identity-confusion]] |

## Related

[[framework-service]]
[[app-intent-implicit-hijack]]
[[framework-service-intent-redirect]]
[[framework-service-identity-confusion]]
