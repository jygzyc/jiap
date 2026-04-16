# Activity - Lifecycle - Misuse

Lifecycle misuse happens when sensitive resources or secrets are handled in the wrong activity callbacks. This can leave protected state alive after backgrounding, recreation, or delayed cleanup.

**Risk: MEDIUM**

## Exploit Prerequisites

The activity starts a sensitive capability such as camera, microphone, location, or account state handling, but cleans it up too late or persists sensitive state across recreation in an unsafe way.

**Android Version Scope:** Relevant across Android versions. Specific extraction paths vary by platform behavior, but the core lifecycle mistake is app-side.

## Bypass Conditions / Uncertainties

- If the issue only concerns ordinary UI state and no sensitive resource or secret is involved, reject the finding
- If the sensitive resource is reliably stopped in `onPause()` / `onStop()` before attacker-relevant backgrounding, reject the finding
- If saved state contains only harmless UI metadata, reject the finding
- Do not claim background spying or secret extraction unless the downstream effect is visible and realistic

## Visible Impact

Visible impact must be concrete, such as:

- continued background camera, microphone, or location use after the activity is no longer foregrounded
- sensitive tokens or credentials persisted in saved state and recoverable through a realistic follow-on path

## Attack Flow

```text
1. decx code class-source "<ActivityClass>" -P <port>
2. Inspect:
   -> onCreate / onResume for resource start
   -> onPause / onStop / onDestroy for cleanup
   -> onSaveInstanceState for persisted state
3. Confirm the sensitive resource or secret outlives the expected trust boundary
4. Confirm the outcome is visible and meaningful
```

## Key Code Patterns

- resource started in `onResume()` but only released in `onDestroy()`
- tokens or secrets persisted in saved state

```java
@Override
protected void onResume() {
    super.onResume();
    camera = Camera.open(0);
    camera.startPreview();
}

@Override
protected void onDestroy() {
    camera.release();
}
```

```java
@Override
protected void onSaveInstanceState(Bundle outState) {
    outState.putString("auth_token", authToken);
}
```

## Secure Pattern

```java
@Override
protected void onPause() {
    super.onPause();
    if (camera != null) {
        camera.stopPreview();
        camera.release();
        camera = null;
    }
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + task hijack | victim activity is backgrounded while sensitive behavior continues | [[app-activity-task-hijack]] |
| + exported activity access | attacker drives a sensitive activity into an unsafe lifecycle state | [[app-activity-exported-access]] |

## Related

[[app-activity]]
[[app-activity-task-hijack]]
[[app-activity-exported-access]]
