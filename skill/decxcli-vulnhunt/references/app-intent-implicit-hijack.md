# Intent - Routing - Implicit Hijack

An app sends an implicit Intent without binding it to a trusted receiver. Another app can claim the action and intercept the payload or workflow.

**Risk: MEDIUM**

## Exploit Prerequisites

The app sends an implicit activity, service, or broadcast Intent that carries sensitive data or triggers a sensitive action, and the attacker can register a matching exported handler.

**Android Version Scope:** Historically strongest on Android 10 to 13 routing behavior, but still relevant whenever sensitive data is sent implicitly.

## Bypass Conditions / Uncertainties

- If the Intent is made explicit with `setPackage()` or `setComponent()`, reject the finding
- If the Intent carries no sensitive data and does not trigger a meaningful action, reject the finding
- If Android-version routing rules prevent the intended hijack path, downgrade or reject accordingly

## Visible Impact

Visible impact must be concrete, such as:

- leaking tokens or passwords
- capturing an internal command or workflow step
- receiving a URI grant or sensitive app state meant for another component

## Attack Flow

```text
1. Trace startActivity, startService, bindService, and sendBroadcast calls
2. Identify Intents with no explicit package or component
3. Inspect payload fields and flags
4. Confirm a malicious matching component could intercept the flow
```

## Key Code Patterns

```java
Intent intent = new Intent("com.app.ACTION_LOGIN");
intent.putExtra("password", userPassword);
sendBroadcast(intent);
```

## Secure Pattern

```java
Intent intent = new Intent(this, TargetActivity.class);
intent.putExtra("password", userPassword);
startActivity(intent);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + broadcast data leak | hijacked broadcast reveals internal data | [[app-broadcast-local-leak]] |
| + mutable `PendingIntent` | hijacked routing yields victim-identity handles | [[app-intent-pendingintent-escalation]] |
| + URI-grant abuse | intercepted Intent carries delegated file access | [[app-intent-uri-permission]] |

## Related

[[app-intent]]
[[app-broadcast-local-leak]]
[[app-intent-pendingintent-escalation]]
[[app-intent-uri-permission]]
