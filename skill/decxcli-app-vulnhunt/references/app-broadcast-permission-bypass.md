# Broadcast - Permission - Bypass

A broadcast surface appears protected by a custom permission. The protection fails when the permission is weak enough for an attacker to obtain, predefine, or otherwise satisfy.

**Risk: MEDIUM**

## Exploit Prerequisites

The receiver or broadcast path relies on a custom permission that is `normal`, `dangerous`, attacker-definable, or otherwise not strongly signature-bound to the target trust boundary.

**Android Version Scope:** Relevant across Android versions. This is a permission-design flaw in broadcast protection logic.

## Bypass Conditions / Uncertainties

- `signature` and non-bypassable same-signer permissions should reject the finding
- A permission defined outside the current APK is uncertain until you can prove who owns it and what `protectionLevel` it uses
- If the broadcast path remains harmless even when the permission is bypassed, reject the finding

## Visible Impact

Visible impact must be concrete, such as:

- sending commands to a receiver that performs sensitive work
- intercepting a supposedly protected broadcast
- triggering internal state changes or downstream component launches

## Attack Flow

```text
1. decx ard app-manifest -P <port>
2. Inspect receiver declarations and custom permission definitions
3. Confirm the referenced permission is weak or attacker-obtainable
4. Confirm the receiver handles a security-relevant action
```

## Key Code Patterns

- `android:permission` present but backed by a `normal` permission
- exported receiver with no strong permission at all

```xml
<permission
    android:name="com.example.PERMISSION_RECEIVE"
    android:protectionLevel="normal" />
```

## Secure Pattern

```xml
<permission
    android:name="com.example.PERMISSION_RECEIVE"
    android:protectionLevel="signature" />
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + intent redirect | weakly protected broadcast reaches an internal redirect path | [[app-activity-intent-redirect]] |
| + service command injection | receiver forwards attacker data into a service action | [[app-service-intent-inject]] |
| + dynamic receiver abuse | weak static permission mirrors a weak runtime trust boundary | [[app-broadcast-dynamic-abuse]] |

## Related

[[app-broadcast]]
[[app-activity-intent-redirect]]
[[app-service-intent-inject]]
[[app-broadcast-dynamic-abuse]]
