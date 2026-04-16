# Activity - Export - Access Control Failure

An exported activity is directly reachable by other apps. It is a vulnerability only when that entry point exposes a protected capability or data behind an app-internal trust boundary.

**Risk: MEDIUM**

## Exploit Prerequisites

The activity is exported and either lacks meaningful permission protection or relies only on UI flow assumptions. The activity must expose a sensitive screen, perform a privileged app action, or return sensitive data.

**Android Version Scope:** Relevant across all Android versions. Android 12 reduced accidental exports by requiring explicit `android:exported`, but intentionally exported sensitive activities remain risky.

## Bypass Conditions / Uncertainties

- Exported alone is not enough; reject the finding if the activity performs no sensitive action
- If protection relies on a custom permission defined outside the current APK, treat it as bypassable only when the permission is attacker-obtainable, not provably signature-bound, or owned by another app
- If the activity still performs a strict in-code caller, session, or signature check before the sensitive action, reject the finding

## Visible Impact

Visible impact must be specific, such as:

- opening an internal account, settings, admin, or payment screen
- triggering a dangerous operation already authorized for the victim app
- bypassing an app login or gate and exposing user data

If the activity is exported but only shows harmless public content, do not report it.

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. Identify activities with names or logic suggesting admin, account, debug, settings, payment, or file access
3. decx code class-source "<ActivityClass>" -P <port>
4. Confirm whether the activity directly performs or exposes a protected action
5. Confirm whether meaningful permission, caller, or session checks exist
```

## Key Code Patterns

- exported activity with no `android:permission`
- app authentication enforced only on another entry screen
- exported activity directly triggering actions that use the victim app's granted permissions

```xml
<activity
    android:name=".PWList"
    android:exported="true" />
```

```java
public class SensitiveActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Intent callIntent = new Intent(Intent.ACTION_CALL);
        callIntent.setData(Uri.parse("tel:10086"));
        startActivity(callIntent);
    }
}
```

## Secure Pattern

```xml
<permission
    android:name="com.app.permission.ACCESS_PWLIST"
    android:protectionLevel="signature" />

<activity
    android:name=".PWList"
    android:exported="true"
    android:permission="com.app.permission.ACCESS_PWLIST" />
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + intent redirect | exported entry becomes the first hop into a non-exported target | [[app-activity-intent-redirect]] |
| + fragment injection | exported host loads attacker-selected internal UI | [[app-activity-fragment-injection]] |
| + clickjacking | user is tricked into approving the sensitive action | [[app-activity-clickjacking]] |

## Related

[[app-activity]]
[[app-activity-intent-redirect]]
[[app-activity-fragment-injection]]
[[app-activity-clickjacking]]
[[app-intent]]
