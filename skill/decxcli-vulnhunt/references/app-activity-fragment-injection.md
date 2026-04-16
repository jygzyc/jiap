# Activity - Fragment - Injection

Fragment injection occurs when an exported activity loads a fragment selected by an external caller. This turns an internal fragment into an externally reachable surface.

**Risk: MEDIUM**

## Exploit Prerequisites

The activity is externally reachable and reads a fragment class name or fragment identifier from attacker-controlled input, then loads it without a strict allowlist.

**Android Version Scope:** Relevant across Android versions. The bug is in the app's dynamic fragment-loading logic.

## Bypass Conditions / Uncertainties

- If only a fixed allowlist of fragment classes is accepted, reject the finding
- If the injected fragment does not expose a real protected action or sensitive UI state, reject the finding
- If the activity is not externally reachable, keep the issue only as a chain element rather than a standalone finding

## Visible Impact

Visible impact must be concrete, such as:

- opening an internal admin or settings fragment
- showing a sensitive account-management fragment
- loading a WebView-bearing fragment that exposes a trusted bridge

## Attack Flow

```text
1. decx code class-source "<ActivityClass>" -P <port>
2. Find dynamic fragment loading:
   -> Fragment.instantiate
   -> FragmentTransaction.replace / add
3. Confirm the fragment selector comes from Intent extras or another untrusted input
4. Confirm the selected fragment performs a sensitive action or exposes sensitive state
```

## Key Code Patterns

- fragment class name read from extras
- no exact-class allowlist before instantiation

```java
String fragmentName = getIntent().getStringExtra("fragment");
if (fragmentName != null) {
    Fragment f = Fragment.instantiate(this, fragmentName);
    getSupportFragmentManager().beginTransaction()
        .replace(R.id.container, f).commit();
}
```

## Secure Pattern

```java
private static final Set<String> ALLOWED_FRAGMENTS = Set.of(
    "com.example.app.HomeFragment",
    "com.example.app.ProfileFragment",
    "com.example.app.SettingsFragment"
);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + exported activity access | exported host becomes directly reachable | [[app-activity-exported-access]] |
| + task hijack | phishing flow gains a trusted internal fragment view | [[app-activity-task-hijack]] |
| + WebView bridge | injected fragment exposes sensitive native bridge methods | [[app-webview-js-bridge]] |

## Related

[[app-activity]]
[[app-activity-exported-access]]
[[app-activity-task-hijack]]
[[app-webview-js-bridge]]
