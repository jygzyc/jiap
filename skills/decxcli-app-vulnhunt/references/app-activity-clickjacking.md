# Activity - UI Trust - Clickjacking

Clickjacking tricks the user into tapping a sensitive control through an overlay or obscured UI. The risk is real only when that coerced tap approves a protected action.

**Risk: LOW**

## Exploit Prerequisites

The attacker needs overlay capability, user interaction, and a target activity that exposes a sensitive confirmation flow such as permission approval, payment confirmation, or device-admin activation.

**Android Version Scope:** Relevant across Android versions. Newer Android releases reduce some background-launch cases, but the main app-side defense is still proper obscured-touch handling.

## Bypass Conditions / Uncertainties

- This is not a strong finding unless user deception leads to a real protected action
- If the target activity uses `filterTouchesWhenObscured` or robust obscured-touch handling on the sensitive control, reject the finding
- If multiple explicit user verifications still remain after the overlay step, keep the rating low

## Visible Impact

Visible impact must be an action the user did not intend to approve, for example:

- granting a dangerous permission
- approving device-admin activation
- confirming a financial or security-sensitive in-app action

## Attack Flow

```text
1. decx ard app-manifest -P <port>
2. decx code class-source "<ActivityClass>" -P <port>
3. Check whether sensitive buttons or views enforce obscured-touch protections
4. Confirm the approved action is actually security-relevant
```

## Key Code Patterns

- sensitive activity with no `android:filterTouchesWhenObscured="true"`
- no `onFilterTouchEventForSecurity()` override on sensitive controls

```java
btnGrantAdmin.setOnClickListener(v -> {
    startActivity(new Intent(DevicePolicyManager.ACTION_ADD_DEVICE_ADMIN));
});
```

## Secure Pattern

```xml
<activity
    android:name=".SettingsActivity"
    android:filterTouchesWhenObscured="true" />
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + task hijack | phishing UI becomes more convincing | [[app-activity-task-hijack]] |
| + exported activity access | user is tricked into driving a sensitive exported flow | [[app-activity-exported-access]] |
| + PendingIntent abuse | user unknowingly approves a victim-identity action | [[app-activity-pendingintent-abuse]] |

## Related

[[app-activity]]
[[app-activity-task-hijack]]
[[app-activity-exported-access]]
[[app-activity-pendingintent-abuse]]
