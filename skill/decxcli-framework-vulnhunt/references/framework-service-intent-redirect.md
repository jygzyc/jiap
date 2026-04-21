# Framework Service - Intent - Redirect

A framework service forwards attacker-controlled Intent-like data into a privileged launch path. Without strict target validation, untrusted input reaches privileged launches or `PendingIntent` executions with elevated identity.

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

- launching a privileged internal target
- reaching a privileged file or resource access path
- executing a privileged PendingIntent flow under system identity

## Attack Flow

```text
1. decx ard system-service-impl "<Interface>" -P <port>
2. decx code class-context "<ServiceImpl>" -P <port>
   -> identify Binder methods that accept Intent, Bundle, Notification, or PendingIntent
3. decx code method-context "<binderMethod>" -P <port>
   -> callees show all downstream launch points (startActivity, sendBroadcast, startService, PendingIntent.send)
4. Inspect Binder methods that receive Intent, Bundle, Notification, or PendingIntent inputs
5. Trace those inputs into startActivity, sendBroadcast, startService, or PendingIntent.send
6. Confirm the path runs under a privileged identity and lacks strict target validation
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
| + sensitive data leak | leaked package or component metadata improves privileged launch targeting | [[framework-service-data-leak]] |
| + identity confusion | attacker-controlled identity or user context broadens redirect reach | [[framework-service-identity-confusion]] |
| + clear-calling-identity misuse | privileged identity context broadens the redirect impact | [[framework-service-clear-identity]] |

## Related

[[framework-service]]
[[framework-service-data-leak]]
[[framework-service-identity-confusion]]
[[framework-service-clear-identity]]
