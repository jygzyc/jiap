# Broadcast - Dynamic Receiver - Abuse

A runtime-registered receiver may accept broadcasts more broadly than the developer expects. If it trusts attacker-controlled actions or extras, the broadcast becomes a command or data injection surface.

**Risk: MEDIUM**

## Exploit Prerequisites

The app dynamically registers a receiver that accepts external broadcasts and processes actions, extras, URLs, or control flags without verifying the sender or restricting the registration scope.

**Android Version Scope:** Most relevant to pre-Android 14 behavior, though dynamic receiver misuse still matters wherever exported runtime registration or weak sender validation remains.

## Bypass Conditions / Uncertainties

- If the receiver is registered as `RECEIVER_NOT_EXPORTED` or otherwise constrained to internal/system senders, reject the finding
- If the receiver accepts arbitrary broadcasts but only performs benign work, reject the finding
- If protection depends on a custom permission defined outside the APK, keep the finding conditional unless bypassability is explicit

## Visible Impact

Visible impact must be concrete, such as:

- triggering a command dispatcher
- injecting a URL into a WebView flow
- causing unauthorized writes or state changes

## Attack Flow

```text
1. decx ard app-receivers -P <port>
2. decx code search-method "registerReceiver" -P <port>
3. Inspect onReceive() logic and registration flags
4. Confirm whether attacker-controlled extras reach a sensitive action
```

## Key Code Patterns

- broad IntentFilter
- untrusted extras directly consumed in `onReceive()`

```java
registerReceiver(new BroadcastReceiver() {
    @Override
    public void onReceive(Context context, Intent intent) {
        String cmd = intent.getStringExtra("command");
        executeCommand(cmd);
    }
}, filter);
```

## Secure Pattern

```java
registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + service command injection | broadcast becomes the first hop into a service action | [[app-service-intent-inject]] |
| + URI-grant abuse | broadcast delivers attacker-controlled grant-bearing intents | [[app-intent-uri-permission]] |
| + WebView URL bypass | broadcast injects navigation into a trusted WebView host | [[app-webview-url-bypass]] |

## Related

[[app-broadcast]]
[[app-service-intent-inject]]
[[app-intent-uri-permission]]
[[app-webview-url-bypass]]
