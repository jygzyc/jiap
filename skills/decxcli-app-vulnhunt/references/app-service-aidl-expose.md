# Service - AIDL - Exposure

An exported service exposes an AIDL Binder interface to external apps. If Binder methods lack caller validation, the service can perform sensitive work or return protected data for an untrusted caller.

**Risk: HIGH**

## Exploit Prerequisites

The service is externally bindable and its AIDL methods reach sensitive reads, writes, commands, or state changes without a strong caller check.

**Android Version Scope:** Relevant across Android versions. This is an app-side IPC authorization flaw.

## Bypass Conditions / Uncertainties

- If the service enforces a non-bypassable manifest permission or strict caller/signature validation, reject the finding
- If the AIDL methods expose only harmless public data, reject the finding
- If a referenced permission is defined outside the current APK, keep the finding conditional unless its protection and ownership are clear

## Visible Impact

Visible impact must be concrete, such as:

- reading tokens, account data, or private records
- triggering internal actions with the victim app's privileges
- mutating app state that should have stayed private

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. decx ard get-aidl -P <port>
3. decx code class-source "<ServiceClass>" -P <port>
4. Identify the returned AIDL Stub and inspect each exposed method
5. Confirm whether Binder caller validation or permission enforcement exists
```

## Key Code Patterns

- `onBind()` returns an AIDL Stub unconditionally
- AIDL methods expose sensitive reads or actions without caller checks

```java
public class ExportedService extends Service {
    private final IMyService.Stub binder = new IMyService.Stub() {
        @Override
        public String getToken(String userId) {
            return TokenManager.getToken(userId);
        }
    };

    @Override
    public IBinder onBind(Intent intent) {
        return binder;
    }
}
```

## Secure Pattern

```java
@Override
public IBinder onBind(Intent intent) {
    int callingUid = Binder.getCallingUid();
    if (!isTrustedCaller(callingUid)) {
        return null;
    }
    return binder;
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + parcel mismatch | crafted parcel reaches the Binder surface | [[app-intent-parcel-mismatch]] |
| + intent redirect | AIDL output or action feeds a redirect chain | [[app-activity-intent-redirect]] |
| + Messenger abuse | same service exposes multiple unauthenticated IPC paths | [[app-service-messenger-abuse]] |

## Related

[[app-service]]
[[app-intent-parcel-mismatch]]
[[app-activity-intent-redirect]]
[[app-service-messenger-abuse]]
