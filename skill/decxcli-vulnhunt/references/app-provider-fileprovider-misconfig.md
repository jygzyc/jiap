# Provider - FileProvider - Misconfiguration

`FileProvider` is meant to share a narrow file subset through `content://` URIs. Over-broad path mappings or unsafe grant flows can turn it into arbitrary file access.

**Risk: HIGH**

## Exploit Prerequisites

The app exposes over-broad FileProvider paths, exports the provider, or returns grant-bearing URIs through attacker-reachable flows such as `setResult()`, redirects, or externally triggered sharing logic.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- `exported="false"` alone is not sufficient if attacker-reachable grant flows still return usable URIs
- `<root-path>` and similarly broad mappings are inherently dangerous; treat them as strong evidence when the URI is attacker-reachable
- If grants are always issued only to trusted internal targets validated by exact package/signature, reject the finding

## Visible Impact

Visible impact must be concrete, such as:

- reading app-private XML, database, or cache files
- writing or overwriting files through a returned writable URI
- combining private file access with another trusted parser or loader

## Attack Flow

```text
1. decx ard app-manifest -P <port>
2. Locate FileProvider declarations and path XML resources
3. Inspect whether paths include root-level or overly broad mappings
4. Trace URI creation and `FLAG_GRANT_*` propagation into exported or externally reachable flows
5. Confirm the resulting URI can expose real private files
```

## Key Code Patterns

- `<root-path>` or broad external-path mappings
- direct return of grant-bearing URI through result or redirect flows

```xml
<paths>
    <root-path name="root" path="" />
</paths>
```

```java
result.setData(fileUri);
result.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
setResult(RESULT_OK, result);
```

## Secure Pattern

```xml
<paths>
    <cache-path name="shared_cache" path="shared/" />
    <files-path name="exports" path="exports/" />
</paths>
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + `setResult()` leak | caller receives a private readable URI | [[app-activity-setresult-leak]] |
| + intent redirect | attacker drives a trusted grant flow to the wrong target | [[app-activity-intent-redirect]] |
| + parcel mismatch | privileged redirect path reaches an over-broad FileProvider | [[app-intent-parcel-mismatch]] |

## Related

[[app-provider]]
[[app-activity-setresult-leak]]
[[app-activity-intent-redirect]]
[[app-intent-parcel-mismatch]]
[[app-intent-uri-permission]]
