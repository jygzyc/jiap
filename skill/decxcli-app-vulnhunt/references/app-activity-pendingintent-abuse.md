# Activity - PendingIntent - Abuse

An exported activity accepts a `PendingIntent` from an untrusted caller or creates a mutable one from attacker-influenced state. In both cases, the attacker may trigger actions under the victim app's identity.

**Risk: HIGH**

## Exploit Prerequisites

The activity is externally reachable and either:

- executes a `PendingIntent` supplied by an untrusted caller, or
- creates a mutable `PendingIntent` with attacker-fillable fields such as action, component, data, or sensitive extras

**Android Version Scope:** Still relevant on modern Android. Newer platform rules reduce some unsafe mutable cases, but do not eliminate activity-level misuse of externally reachable `PendingIntent` flows.

## Bypass Conditions / Uncertainties

- If the activity validates the origin and intended use of the incoming `PendingIntent`, reject the finding
- If the created `PendingIntent` is immutable and fully pinned to a safe target, reject the finding
- If the `PendingIntent` execution path does not reach a meaningful protected action, reject the finding
- If protection depends on a permission defined outside the current APK, keep bypassability conditional unless ownership and `protectionLevel` are known

## Visible Impact

Visible impact must be concrete, such as:

- placing a call or sending a message through the victim app
- launching an internal or protected component
- invoking a privileged workflow under the victim identity

## Attack Flow

```text
1. decx code class-source "<ActivityClass>" -P <port>
2. Trace:
   -> PendingIntent.getActivity / getService / getBroadcast
   -> getParcelableExtra(...) for incoming PendingIntent objects
   -> PendingIntent.send()
3. Confirm whether the target and semantics remain attacker-controllable
4. Confirm the executed action is security-relevant
```

## Key Code Patterns

- incoming `PendingIntent` executed without trust validation
- mutable `PendingIntent` created from an incomplete base Intent

```java
PendingIntent pi = getIntent().getParcelableExtra("callback");
pi.send();
```

```java
Intent intent = new Intent();
PendingIntent pi = PendingIntent.getActivity(
    this,
    0,
    intent,
    PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_MUTABLE
);
```

## Secure Pattern

```java
Intent intent = new Intent(this, TargetActivity.class);
PendingIntent pi = PendingIntent.getActivity(
    this,
    0,
    intent,
    PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + intent redirect | victim identity reaches an internal non-exported target | [[app-activity-intent-redirect]] |
| + service command injection | victim identity reaches a protected service-side action | [[app-service-intent-inject]] |
| + `setResult()` leak | executed action returns sensitive result data | [[app-activity-setresult-leak]] |
| + task hijack | phishing flow convinces the user to drive a victim-identity action | [[app-activity-task-hijack]] |

## Related

[[app-activity]]
[[app-activity-intent-redirect]]
[[app-intent-pendingintent-escalation]]
[[app-service-intent-inject]]
[[app-activity-setresult-leak]]
[[app-activity-task-hijack]]
