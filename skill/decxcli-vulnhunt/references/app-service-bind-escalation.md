# Service - Binder - Privilege Abuse

A bound service exposes methods that external callers can invoke after binding. If those methods run with the victim app's permissions or trust assumptions, the binder becomes a privilege-escalation surface.

**Risk: HIGH**

## Exploit Prerequisites

The service is externally bindable and its binder-exposed methods perform privileged operations or access protected data without validating the caller.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- If the service enforces a strong manifest permission or strict caller/signature validation, reject the finding
- If the exposed methods do not cross a real permission or data boundary, reject the finding
- Permissions defined outside the current APK remain conditional until ownership and level are proven

## Visible Impact

Visible impact must be concrete, such as:

- reading or deleting files the attacker app could not access directly
- sending SMS, placing calls, or triggering other dangerous actions through the victim app
- querying internal balances, orders, or account state

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. decx code class-source "<ServiceClass>" -P <port>
3. Inspect onBind() and the returned binder class
4. Enumerate sensitive binder methods
5. Confirm whether caller validation exists before the privileged action
```

## Key Code Patterns

- exported bindable service with no strong permission
- binder methods that read files or perform privileged device/app actions

```java
public class FileManagerService extends Service {
    public class FileManagerBinder extends Binder {
        public String readFile(String path) {
            return FileUtils.read(path);
        }
    }
}
```

## Secure Pattern

```java
public String readFile(String path) {
    int callingUid = Binder.getCallingUid();
    if (!isAllowedUid(callingUid)) {
        throw new SecurityException("Unauthorized caller");
    }
    return FileUtils.read(path);
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + AIDL exposure | same binder surface exposes even more methods | [[app-service-aidl-expose]] |
| + parcel mismatch | binder method consumes crafted parcel data | [[app-intent-parcel-mismatch]] |
| + mutable `PendingIntent` | service creates victim-identity actions | [[app-intent-pendingintent-escalation]] |

## Related

[[app-service]]
[[app-service-aidl-expose]]
[[app-intent-parcel-mismatch]]
[[app-intent-pendingintent-escalation]]
