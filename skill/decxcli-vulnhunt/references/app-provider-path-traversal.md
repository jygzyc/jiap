# Provider - File - Path Traversal

A provider builds file paths from attacker-controlled URI segments without confining access to an expected root. This can expose arbitrary file read or write through `openFile()`-style APIs.

**Risk: HIGH**

## Exploit Prerequisites

The provider is externally reachable and derives file paths from URI path segments, `getLastPathSegment()`, decoded values, or equivalent attacker-controlled path material without a safe canonical-root check.

**Android Version Scope:** Relevant across Android versions.

## Bypass Conditions / Uncertainties

- If canonical-path validation pins the file to a fixed allowed root and write modes are not exposed beyond that root, reject the finding
- Simple substring checks against `..` are usually bypassable, especially after URI decoding
- Writable modes only increase severity if they can actually overwrite meaningful files
- If the traversed target set has no meaningful data or execution impact, downgrade or reject

## Visible Impact

Visible impact must be concrete, such as:

- reading shared preferences, databases, or private XML files
- overwriting app-private files through a writable descriptor
- reaching a library or plugin path that changes code-loading behavior

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. decx code class-source "<ProviderClass>" -P <port>
3. Inspect openFile / openAssetFile / openTypedAssetFile
4. Trace URI segments into File construction and mode selection
5. Confirm canonical-root enforcement is missing or bypassable
6. Confirm the reachable file set has real security value
```

## Key Code Patterns

- direct `File` construction from URI path material
- decoded path segments bypassing naive filters

```java
@Override
public ParcelFileDescriptor openFile(Uri uri, String mode) {
    String filename = uri.getPathSegments().get(0);
    File file = new File(getContext().getFilesDir(), filename);
    return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY);
}
```

## Secure Pattern

```java
String canonicalPath = file.getCanonicalPath();
String allowedRoot = getContext().getFilesDir().getCanonicalPath();
if (!canonicalPath.startsWith(allowedRoot + File.separator)
        && !canonicalPath.equals(allowedRoot)) {
    throw new SecurityException("Path traversal detected");
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + activity path traversal | attacker broadens file access across surfaces | [[app-activity-path-traversal]] |
| + URI grant abuse | traversed content is later delegated to the attacker | [[app-intent-uri-permission]] |
| + FileProvider misconfig | over-broad file sharing magnifies traversal impact | [[app-provider-fileprovider-misconfig]] |
| + WebView file access | traversed data is loaded into a trusted renderer | [[app-webview-file-access]] |

## Related

[[app-provider]]
[[app-activity-path-traversal]]
[[app-intent-uri-permission]]
[[app-provider-fileprovider-misconfig]]
[[app-webview-file-access]]
