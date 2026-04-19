# Module Security Analysis Report

## Basic Information

| Field | Value |
|------|-------|
| Target App | `com.target.app` |
| Target Version | `x.x.x` |
| Android Version | `xx (API xx)` |
| Analysis Date | `YYYY-MM-DD` |

---

## Attack-Surface Coverage Summary

| Metric | Value |
|------|-------|
| Total Surfaces | `0` |
| `statically-supported` | `0` |
| `candidate` | `0` |
| `rejected` | `0` |
| Coverage Complete | `true / false` |

> Summarize the totals from `coverage.json` and state explicitly whether every externally reachable surface was accounted for.
> Also state the current trace focus: source component, issue-type hypotheses, chains already traced, and the next targets to analyze.

---

## Issue 1: [Risk] Vulnerability Title

### 1. Vulnerability Analysis

#### Background

> Briefly describe the flaw, the currently traced exploit path, and the security boundary that fails.

#### Full Call Chain

> Provide the full function-signature chain from the victim component entrypoint to sink.
> Start from the target app's exported component or Binder-exposed method.
> Do not start with attacker actions such as `AttackerApp.*`, `bindService`, `startActivity`, `sendBroadcast`, or `ContentResolver.*`.
> Put those steps only in `Attack Path`.

```text
com.target.EntryActivity.onCreate(android.os.Bundle):void  (entry)
  -> getIntent().getParcelableExtra("forward_intent")
  -> startActivity(nestedIntent)
    -> com.target.InternalActivity.onCreate(android.os.Bundle):void
      -> handleIntent(intent)
        -> vulnerableOperation(data)
```

For bound-service / AIDL issues, use this shape instead:

```text
com.target.VulnService.onBind(android.content.Intent):android.os.IBinder  (entry)
  -> return mBinder
    -> com.target.IService$Stub.deleteFile(java.lang.String):void
      -> com.target.FileHelper.delete(java.lang.String):void
```

#### Code Analysis

> Use numbered evidence points tied directly to the conclusion.

1. **The component is externally reachable and not meaningfully protected**

```xml
<service
    android:name="com.target.VulnService"
    android:exported="true">
    <intent-filter>
        <action android:name="com.target.action"/>
    </intent-filter>
</service>
```

2. **The service does not validate the caller identity**

```java
@Override
public IBinder onBind(Intent intent) {
    if (checkSelfPermission("com.target.INTERNAL") != PERMISSION_GRANTED) return null;
    return mBinder; // flawed: checks self-permission, not caller identity
}
```

3. **The sensitive operation lacks authorization**

```java
private void deleteFile(ContentValues values, String filepath) {
    mHandler.sendMessage(mHandler.obtainMessage(MSG_DELETE, filepath));
}
```

#### Bypass Conditions / Uncertainties

> State every condition under which an observed protection can still be bypassed, or explain why the finding remains a `candidate`.

### 2. Attack Path

#### Target Surface

| Field | Value |
|------|-------|
| Target Package | `com.target` |
| Target Class | `com.target.VulnService` |
| Action / URI | `com.target.ACTION` / `content://com.target.provider/data` |
| IPC Surface | `com.target.IService` (AIDL) / Messenger `msg.what=1` |

#### Exploitation Steps

> Describe only the steps that a third-party attacker app can realistically perform.
> `AttackerApp.bindService(...)` belongs here, not in `Full Call Chain`.

1. `bindService` to `com.target.VulnService`
2. Obtain the `IService` AIDL interface
3. Call `deleteFile(filepath="/data/data/com.target/databases/secret.db")`
4. Verify the file is deleted

### 3. Visible Impact

> State the real observable consequence.

### 4. Rating Rationale

> Map the impact to `references/risk-rating.md` and justify the selected rating.

### 5. Remediation

> Provide concrete fixes.

---

## Residual Candidate Surfaces

> List every target that remains `candidate` at the end. These are not confirmed vulnerabilities, but they are also not proven safe.

| Target ID | Type | Current State | Missing Proof |
|-----------|------|---------------|---------------|
| `activity-forward-01` | `Activity` | `candidate` | Missing the full cross-component chain from external input to internal sink |

---

## Issue 2: [Risk] Next Vulnerability Title

Use the same structure.
