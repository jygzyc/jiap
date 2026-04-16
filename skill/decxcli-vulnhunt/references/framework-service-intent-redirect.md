# Framework Service - Intent - Redirect

A framework service forwards attacker-controlled Intent-like data into a privileged launch path. Without strict target validation, untrusted input reaches components or `PendingIntent` executions with elevated identity.

**Risk: HIGH**

## Exploit Prerequisites

The service receives externally influenced Intent data through Binder, notification objects, account/auth flows, or another IPC path, and then launches or executes it under a privileged identity.

**Android Version Scope:** Historically strongest on pre-Android 12 style redirect chains, but still worth reviewing in custom or vendor code paths.

## Bypass Conditions / Uncertainties

- If the framework path rebuilds a trusted Intent from scratch or validates the exact allowed target, reject the finding
- If newer platform defenses block the specific redirect primitive, downgrade or reject accordingly
- Do not report unless the downstream privileged target is real and meaningful

## Visible Impact

Visible impact must be concrete, such as:

- launching a non-exported privileged component
- reaching a root-path FileProvider or similar privileged file surface
- executing a privileged PendingIntent flow under system identity

## Attack Flow

```text
1. decx ard system-service-impl "<Interface>" -P <port>
2. Inspect Binder methods that receive Intent, Bundle, Notification, or PendingIntent inputs
3. Trace those inputs into startActivity, sendBroadcast, startService, or PendingIntent.send
4. Confirm the path runs under a privileged identity and lacks strict target validation
```

## Key Code Patterns

```java
if (notification.contentIntent != null) {
    notification.contentIntent.send();
}
```

## Secure Pattern

```java
PendingIntent safeIntent = buildTrustedContentIntent(tag, id);
if (safeIntent != null) {
    safeIntent.send();
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + app intent redirect | same redirect concept appears in app-layer components | [[app-activity-intent-redirect]] |
| + parcel mismatch | validation is bypassed before the privileged launch | [[app-intent-parcel-mismatch]] |
| + FileProvider misconfig | privileged redirect reaches an over-broad file surface | [[app-provider-fileprovider-misconfig]] |
| + clear-calling-identity misuse | privileged identity context broadens the redirect impact | [[framework-service-clear-identity]] |

## Related

[[framework-service]]
[[app-activity-intent-redirect]]
[[app-intent-parcel-mismatch]]
[[app-provider-fileprovider-misconfig]]
[[framework-service-clear-identity]]
