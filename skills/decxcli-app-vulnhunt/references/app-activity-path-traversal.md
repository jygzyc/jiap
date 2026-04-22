# Activity - File - Path Traversal

An exported activity accepts an untrusted path or URI and uses it in file operations without confinement. This can let an attacker read or write files outside the intended directory.

**Risk: HIGH**

## Exploit Prerequisites

The activity is externally reachable and attacker-controlled path data reaches file open, read, write, rename, delete, or share logic.

**Android Version Scope:** Relevant across Android versions. This is a path-validation failure in activity-side file handling.

## Bypass Conditions / Uncertainties

- Reject the finding if the input is reduced to a safe filename and canonical-path validation enforces a fixed allowed root
- `startsWith()` checks are only safe after canonicalization against a fixed root; otherwise treat them as bypassable
- If the path reaches only app-controlled temporary files with no sensitive value, reject the finding

## Visible Impact

Visible impact must be concrete, such as:

- arbitrary read of app-private files
- overwrite or deletion of app data
- disclosure of tokens, databases, or configuration files

## Attack Flow

```text
1. decx ard exported-components -P <port>
2. decx code class-source "<ActivityClass>" -P <port>
3. Trace getStringExtra / getData into File, FileInputStream, FileOutputStream, openFileInput, openFileOutput, renameTo, delete
4. Confirm whether canonical-path or filename allowlisting blocks traversal
5. Confirm the targeted file set has real security value
```

## Key Code Patterns

- attacker path used directly in file constructors
- traversal sequences not normalized or constrained

```java
String filePath = getIntent().getStringExtra("file_path");
FileInputStream fis = new FileInputStream(filePath);
```

## Secure Pattern

```java
String fileName = getIntent().getStringExtra("file_name");
if (fileName == null || fileName.contains("/") || fileName.contains("\\")) {
    finish();
    return;
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + provider traversal | broadens file access through another surface | [[app-provider-path-traversal]] |
| + URI grants | stolen file or content URI is handed to the attacker | [[app-intent-uri-permission]] |
| + WebView file access | traversed file is rendered inside a trusted WebView context | [[app-webview-file-access]] |

## Related

[[app-activity]]
[[app-provider-path-traversal]]
[[app-intent-uri-permission]]
[[app-webview-file-access]]
