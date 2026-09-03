# Native Semgrep (Android)

This crate runs **Semgrep-style** Android rules **natively** (no Semgrep binary). Matching uses:

1. **SSA / value-flow** (`native:` hints in YAML) — reliable on DEX.
2. **Java token patterns** — Semgrep-like `$META`, `$_`, `...`, `pattern-either` on decompiled method bodies.
3. **XML / regex** — MASTG manifest rules on plaintext `AndroidManifest.xml`.

## Default rule set

Built-in rules (`--scan-semgrep` with no `--semgrep-rules`) skip `android.*` and `androidx.*` platform classes to reduce false positives from the SDK/libraries.

| Source | Path |
|--------|------|
| General Android rules (WebView, IPC, SSL) | `rules/semgrep/android/general.yml` (4 rules) |
| [OWASP MASTG](https://github.com/OWASP/mastg/tree/master/rules) Android | `rules/semgrep/android/mastg/` |

### General Android rules

| Rule ID | Class |
|---------|--------|
| `android.webview.loadurl-from-intent` | Intent/Uri → `WebView.loadUrl` |
| `android.webview.js-interface-added` | `addJavascriptInterface` |
| `android.intent.redirect` | `getParcelableExtra` → `startActivity` / IPC |
| `android.ssl.bypass-handler-proceed` | `onReceivedSslError` → `proceed()` |

### OWASP MASTG

See [`android/mastg/README.md`](android/mastg/README.md). Includes crypto, WebView, deeplinks, biometric, network TLS, storage, permissions, and related MASVS checks.

## CLI

```bash
# Default: general Android + all MASTG rules
cargo run --release --bin dex-decompile -- -i app.apk --scan-semgrep

# General rules only
cargo run --release --bin dex-decompile -- -i classes.dex --scan-semgrep \
  --semgrep-rules rules/semgrep/android/general.yml

# MASTG directory only
cargo run --release --bin dex-decompile -- -i classes.dex --scan-semgrep \
  --semgrep-rules rules/semgrep/android/mastg

# Decoded manifest (XML MASTG rules)
cargo run --release --bin dex-decompile -- -i AndroidManifest.xml --scan-semgrep
```

## Custom YAML

Rules may include Semgrep `pattern` / `patterns` / `pattern-either` / `pattern-regex` plus an optional `native:` block:

```yaml
native:
  kind: source_sink   # or invoke | method_invoke
  sources: [getStringExtra]
  sinks: [loadUrl]
  methods: [addJavascriptInterface]
  method_name: onReceivedSslError   # for method_invoke
```

## Library

```rust
use dex_decompiler::{builtin_android_rules, parse_dex, scan_dex_semgrep, scan_xml_semgrep};

let dex = parse_dex(&bytes)?;
let rules = builtin_android_rules();
let findings = scan_dex_semgrep(&dex, &rules);
let xml_findings = scan_xml_semgrep(manifest_xml, "AndroidManifest.xml", &rules);
```
