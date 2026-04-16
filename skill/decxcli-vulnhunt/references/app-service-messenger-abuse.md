# Service - Messenger - Abuse

A service exposes a `Messenger` interface that accepts attacker-controlled messages. If the handler trusts message identity or contents, the service becomes an IPC command surface.

**Risk: HIGH**

## Exploit Prerequisites

The service is externally bindable, returns a `Messenger` binder, and `handleMessage()` reaches sensitive reads, commands, or state changes from untrusted message fields.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- If the handler verifies the caller identity or restricts access with a strong permission, reject the finding
- If `msg.what`, `replyTo`, and the Bundle fields do not reach a security-relevant effect, reject the finding
- Serialization-related impact requires the message payload to carry a meaningful object or parser confusion path

## Visible Impact

Visible impact must be concrete, such as:

- returning sensitive data through `replyTo`
- executing an internal command
- changing privileged app state

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. decx code class-source "<ServiceClass>" -P <port>
3. Confirm onBind() returns Messenger.getBinder()
4. Inspect handleMessage() dispatch on msg.what and Bundle contents
5. Confirm a sensitive branch is reachable from attacker-controlled message fields
```

## Key Code Patterns

- external message dispatch with no caller validation

```java
class IncomingHandler extends Handler {
    @Override
    public void handleMessage(Message msg) {
        switch (msg.what) {
            case MSG_GET_DATA:
                Message reply = Message.obtain(null, MSG_DATA_RESPONSE);
                reply.obj = sensitiveData;
                msg.replyTo.send(reply);
                break;
        }
    }
}
```

## Secure Pattern

```java
int callingUid = Binder.getCallingUid();
if (!isAllowedUid(callingUid)) {
    return;
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + bound-service abuse | alternative binder path reaches the same logic | [[app-service-bind-escalation]] |
| + parcel mismatch | malicious objects reach the handler Bundle | [[app-intent-parcel-mismatch]] |
| + intent redirect | message fields are forwarded into a component launch | [[app-activity-intent-redirect]] |

## Related

[[app-service]]
[[app-service-bind-escalation]]
[[app-intent-parcel-mismatch]]
[[app-activity-intent-redirect]]
