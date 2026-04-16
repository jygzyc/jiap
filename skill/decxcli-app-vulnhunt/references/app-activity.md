# Activity - Overview - Security Review

Activities are the most common Android entrypoint. Exported activities, deep links, activity-result handlers, and task-stack interactions routinely become the first hop of a vulnerability chain.

## Risk Catalog

| Risk | Rating | Reference |
|------|--------|-----------|
| Exported Activity access control failure | MEDIUM | [[app-activity-exported-access]] |
| Intent redirect | HIGH | [[app-activity-intent-redirect]] |
| Fragment injection | MEDIUM | [[app-activity-fragment-injection]] |
| Path traversal | HIGH | [[app-activity-path-traversal]] |
| PendingIntent abuse | HIGH | [[app-activity-pendingintent-abuse]] |
| `setResult()` data leak | MEDIUM | [[app-activity-setresult-leak]] |
| Task hijack / StrandHogg-style abuse | MEDIUM | [[app-activity-task-hijack]] |
| Clickjacking | LOW | [[app-activity-clickjacking]] |
| Lifecycle misuse | MEDIUM | [[app-activity-lifecycle]] |

## Analysis Flow

```text
1. decx ard exported-components -P <port>
   -> list exported activities and deep-link handlers
2. decx code class-source "<ActivityClass>" -P <port>
   -> inspect onCreate / onNewIntent / activity-result handlers
3. Check external inputs:
   -> getIntent().get*Extra()
   -> getIntent().getData()
   -> getClipData()
   -> onActivityResult() / ActivityResultLauncher callbacks
4. Check sensitive actions:
   -> startActivity / startService / sendBroadcast
   -> setResult
   -> file read/write helpers
   -> WebView host initialization
5. Confirm whether caller validation, signature checks, package allowlists, or target allowlists exist
```

## Key Trace Patterns

- Nested Intent extraction followed by immediate forwarding
- Dynamic class loading or fragment class selection from extras
- Path or URI values flowing into file APIs
- Result callbacks trusting scanner, browser, picker, or third-party activity output
- PendingIntent objects accepted from or returned to untrusted callers

## Related

[[app-intent]]
[[app-service]]
[[app-webview]]
[[risk-rating]]
