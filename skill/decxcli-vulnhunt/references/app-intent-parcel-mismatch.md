# Intent - Parcel - Mismatch

Parcel mismatch happens when serialization and deserialization do not agree on object structure. In security checks, that mismatch can make a safe-looking object pass validation while a different payload is used later.

**Risk: HIGH**

## Exploit Prerequisites

This requires a security-sensitive Bundle or Intent validation path, plus a custom Parcelable or equivalent structure whose write/read logic is asymmetric. The attacker must be able to supply the crafted parcel through AIDL, Binder, or an exported component.

**Android Version Scope:** Most relevant to older framework and OEM code paths. Historically common in Android 10 to 11 era redirect and Bundle-check flows, but still worth checking in custom app or vendor parcelables.

## Bypass Conditions / Uncertainties

- A `checkKeyIntent()`-style guard is bypassable only if the validated object and the later-consumed object can diverge because of parcel read/write asymmetry
- If the Parcelable is framework-defined and version-matched, do not assume mismatch without explicit evidence
- If the finding depends on a custom permission or Binder gate defined outside the current APK, keep it as `candidate` unless the bypass condition is concrete
- Reject the finding if the same canonicalized object instance is both validated and consumed with no attacker-controlled reparse

## Visible Impact

Only report when the downstream effect is visible, such as:

- launching an unintended privileged component
- smuggling a malicious extra or nested Intent past a guard
- reaching a privileged file or account operation through a framework or vendor service

If the mismatch only causes deserialization failure or crash behavior, do not report it.

## Attack Flow

```text
1. decx code search-method "checkKeyIntent" -P <port>
2. decx code implement "android.os.Parcelable$Creator" -P <port>
3. Inspect writeToParcel / createFromParcel symmetry
4. Identify whether a validated field can differ from the later-consumed field
5. Confirm the post-validation sink is security-relevant
```

## Key Code Patterns

- validation on one extracted object, followed by later use of a differently parsed object
- custom Parcelable field order mismatch
- security decisions made before full Bundle normalization

```java
Bundle bundle = new Bundle();
bundle.putParcelable("intent", safeIntent);
bundle.putParcelable("extra_intent", maliciousIntent);

Intent checked = bundle.getParcelable("intent");
checkKeyIntent(checked); // safeIntent is checked
// later code consumes different parcel state or an unvalidated field
```

## Secure Pattern

```java
public Bundle sanitizeBundle(Bundle input) {
    Bundle safe = new Bundle();
    Set<String> allowedKeys = Set.of("key1", "key2");
    for (String key : input.keySet()) {
        if (allowedKeys.contains(key)) {
            safe.putParcelable(key, input.getParcelable(key));
        }
    }
    safe.setClassLoader(getClass().getClassLoader());
    return safe;
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + framework redirect | bypasses Intent validation and launches a privileged target | [[framework-service-intent-redirect]] |
| + AIDL exposure | delivers the crafted parcel through a Binder surface | [[app-service-aidl-expose]] |
| + ClassLoader injection | combines parser confusion with unsafe object loading | [[app-intent-classloader-inject]] |

## Related

[[app-intent]]
[[app-service-aidl-expose]]
[[app-intent-classloader-inject]]
[[framework-service-intent-redirect]]
