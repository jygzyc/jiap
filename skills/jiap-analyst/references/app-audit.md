# App Security Audit Workflow

**Scope**: User-space Android Applications (APKs, System Apps)
**Focus**: IPC vulnerabilities, exported components, WebViews

## Core Principle: Exploitability Only

**Report ONLY findings with demonstrable exploitability.**

### Vulnerability = Code Flaw + Exploit Path

Before reporting, verify:
1. **Reachability**: Can attacker trigger this path?
2. **Control**: Can attacker influence critical data?
3. **Impact**: Does it cause security breach?

If ANY answer is NO → **Do NOT report**.

---

## 1. Intent Redirection

**Vulnerability**: Unvalidated Intent extra reaches component start methods

**Detection**:
```
search_class_key("getParcelableExtra")  // Find extras
get_method_xref("startActivity")        // Trace to start methods
get_method_xref("startService")
get_method_xref("sendBroadcast")
```

**Exploitability Check**:
- Attacker can pass malicious Intent via exported component
- No allowlist validation or safe restriction
- Can access internal components or sensitive operations

**Report IF**:
- ✅ Intent extra reaches `startActivity()`/`startService()`/`sendBroadcast()` without allowlist
- ✅ Attacker controls which component starts
- ❌ Validate against allowlist → **NOT A VULNERABILITY**
- ❌ Restricts to internal package only → **NOT A VULNERABILITY**

## 2. Deep Link Data Abuse

**Vulnerability**: User-controlled data from deep links reaches sensitive sinks

**Detection**:
```
get_deep_links()                   // Get all deep links
search_method("getData")           // Find data extraction
get_method_xref("loadUrl")         // WebView URL loading
get_method_xref("exec")            // Command execution
get_method_xref("rawQuery")        // SQL queries
```

**Exploitability Check**:
- Data from `getData()` or `getQueryParameter()` reaches sensitive sink
- No sanitization or allowlist validation
- Sink is exploitable (SQL, file, exec, WebView URL)

**Report IF**:
- ✅ User-controlled data reaches `webView.loadUrl()` → XSS/Phishing
- ✅ User-controlled data reaches `new File()` → Path traversal
- ✅ User-controlled data reaches `rawQuery()` → SQL injection
- ❌ Data validated or sanitized → **NOT A VULNERABILITY**
- ❌ Input not attacker-controllable → **NOT A VULNERABILITY**

## 3. WebView Remote Code Execution

**Vulnerability**: JavaScript interface exposes dangerous APIs

**Detection**:
```
search_method("addJavascriptInterface")  // Find JS interfaces
```

**Exploitability Check**:
- WebView loads untrusted content (HTTP, dynamic URLs, user-provided content)
- Exposed methods can execute dangerous operations (exec, startActivity, file access)
- Protection bypassed (SDK < 17, or methods not properly annotated)

**Report IF**:
- ✅ SDK < 17 AND exposes methods → **CRITICAL** (attacker calls any public method)
- ✅ SDK ≥ 17 BUT exposes dangerous APIs (`Runtime.exec()`, `startActivity()`) → **HIGH**
- ❌ Only exposes safe methods (data getters, UI helpers) → **NOT A VULNERABILITY**
- ❌ WebView only loads trusted local content → **NOT A VULNERABILITY**

## 4. SQL Injection

**Vulnerability**: User-controllable input reaches SQL query without parameterization

**Detection**:
```
search_method("rawQuery")           // Find raw queries
search_method("execSQL")
get_method_xref("android.database.sqlite.SQLiteDatabase.rawQuery")
```

**Exploitability Check**:
- User input reaches SQL query via string concatenation
- Input source is exposed (exported activity, deep link, content provider)
- Attacker can control the query

**Report IF**:
- ✅ User-controllable input reaches `rawQuery()` via concatenation AND input source is exposed → **HIGH**
- ❌ Uses parameterized queries (`?` placeholders) → **NOT A VULNERABILITY**
- ❌ Input not attacker-controllable (internal methods only) → **NOT A VULNERABILITY**

## Reporting Template

```
[SEVERITY] Finding Title
Risk: HIGH/MEDIUM/LOW
Component: class.method()
Cause: Brief description
Evidence:
  // Code snippet
Exploit Path:
  1. Attacker does X
  2. Input reaches Y
  3. Triggers Z
Impact: Security breach
Mitigation: Fix
```
