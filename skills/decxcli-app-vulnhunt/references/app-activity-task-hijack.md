# Activity - Task - Hijack

Task hijack issues let an attacker insert a misleading activity into the victim's task flow. The goal is usually credential phishing or approval of attacker-controlled UI that appears trusted.

**Risk: MEDIUM**

## Exploit Prerequisites

The target activity participates in task-stack behavior that lets an attacker-controlled activity blend into or take over the visible task flow, and the resulting UI confusion leads to a real security-relevant action.

**Android Version Scope:** Historically strongest on older Android versions, but still worth reviewing in custom task-affinity and phishing-prone flows on modern Android.

## Bypass Conditions / Uncertainties

- If the issue only changes task appearance without enabling credential theft, approval spoofing, or another real security effect, reject it
- Newer Android mitigations and background-start restrictions may reduce exploitability; reflect that in rating and conditions
- If the target UI already binds every sensitive action to explicit reauthentication, reject the finding

## Visible Impact

Visible impact must be concrete, such as:

- phishing credentials from a spoofed login screen
- tricking the user into confirming a sensitive in-app operation
- intercepting a trusted activity-result or workflow step

## Attack Flow

```text
1. decx ard app-manifest -P <port>
2. Inspect task-affinity, launch mode, reparenting, and recents behavior
3. Confirm whether the resulting UI confusion enables a real protected action
4. Rate only the actual security effect, not the task quirk by itself
```

## Key Code Patterns

- empty or overly permissive task affinity
- reparenting or task flags that make phishing-style UI confusion easier

```xml
<activity
    android:name=".LoginActivity"
    android:exported="true"
    android:taskAffinity=""
    android:launchMode="singleTask"
    android:allowTaskReparenting="true" />
```

## Secure Pattern

```xml
<activity
    android:name=".LoginActivity"
    android:taskAffinity="com.example.myapp"
    android:launchMode="singleTask"
    android:excludeFromRecents="false" />
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + clickjacking | phishing flow becomes more convincing | [[app-activity-clickjacking]] |
| + fragment injection | attacker-controlled internal-looking UI is loaded inside a trusted host | [[app-activity-fragment-injection]] |
| + WebView bridge | spoofed login or payment UI steals credentials | [[app-webview-js-bridge]] |

## Related

[[app-activity]]
[[app-activity-clickjacking]]
[[app-activity-fragment-injection]]
[[app-webview-js-bridge]]
