# Activity - Result - setResult() Data Leak

An exported activity returns unfiltered sensitive data or URI grants through `setResult()`. This lets an external caller receive data or delegated access that should not cross the app boundary.

**Risk: MEDIUM**

## Exploit Prerequisites

The attacker can launch the activity for result, and the activity returns sensitive fields, a trusted URI, or a grant-bearing Intent without validating the caller and sanitizing the returned payload.

**Android Version Scope:** Relevant across Android versions. This is a result-handling flaw in activity return logic.

## Bypass Conditions / Uncertainties

- If the returned Intent is rebuilt from scratch and contains only public data, reject the finding
- If caller validation is strict and non-bypassable, reject the finding
- URI-grant impact requires the returned Intent to contain a usable `content://` URI plus grant flags or an equivalent delegated access path

## Visible Impact

Visible impact must be concrete, such as:

- leaking account data or tokens to the caller
- returning a private `content://` URI with read or write access
- disclosing file picker output that was intended only for trusted internal callers

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. decx code class-source "<ActivityClass>" -P <port>
3. Trace setResult and result-construction logic
4. Confirm whether the activity returns attacker-supplied intents, sensitive fields, or URI grants
5. Confirm the returned data is visible and security-relevant
```

## Key Code Patterns

- returning incoming `data` directly
- returning `FLAG_GRANT_*` on a trusted content URI
- no `getCallingPackage()` or equivalent caller check

```java
@Override
protected void onActivityResult(int requestCode, int resultCode, Intent data) {
    setResult(RESULT_OK, data);
    finish();
}
```

## Secure Pattern

```java
Intent safeResult = new Intent();
safeResult.putExtra("result_key", data.getStringExtra("public_data"));
safeResult.setFlags(0);
setResult(RESULT_OK, safeResult);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + URI-grant abuse | caller receives private file access | [[app-intent-uri-permission]] |
| + FileProvider misconfig | returned URI points into an over-broad file surface | [[app-provider-fileprovider-misconfig]] |
| + intent redirect | redirect reaches the leaking activity first | [[app-activity-intent-redirect]] |

## Related

[[app-activity]]
[[app-intent-uri-permission]]
[[app-provider-fileprovider-misconfig]]
[[app-activity-intent-redirect]]
