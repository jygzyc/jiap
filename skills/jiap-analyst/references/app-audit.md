# App Security Audit Workflow

**Scope**: User-space Android Applications (APKs, System Apps)
**Focus**: IPC vulnerabilities, exported components, WebViews, data flow analysis

## Core Principle: Taint-Based Verification

**Every vulnerability report MUST demonstrate a complete taint flow: Source → Propagation → [No Sanitizer] → Sink**

### The Taint Equation

```
Vulnerability = Attacker-Controlled Source
                + Data Propagation Path
                - Sanitizer (validation/cleansing)
                → Dangerous Sink
                = CONFIRMED VULNERABILITY
```

### Verification Framework

| Step | Question | How to Verify |
|------|----------|---------------|
| **1. Source** | Is data attacker-controlled? | Check `getIntent()`, `getData()`, deep links, exported components |
| **2. Path** | Does tainted data reach the sink? | Use `get_method_xref()` to trace call graph, `get_method_source()` to read code |
| **3. Sanitizer** | Is data validated/cleansed? | Look for allowlists, canonicalization, parameterization |
| **4. Sink** | Is the endpoint exploitable? | Command exec, SQL query, file access, component launch |
| **5. Reachability** | Can attacker trigger this? | Exported component? Deep link? Broadcast? |

**If ANY step is missing or blocked → DO NOT REPORT**

---

## Taint Analysis Methodology

### Step 1: Identify Sources (Entry Points)

Find where attacker-controlled data enters the app:

```bash
# Intent-based sources
search_method("getIntent", 1)
search_method("getStringExtra", 1)
search_method("getParcelableExtra", 1)
search_method("getExtras", 1)

# Deep link sources
search_method("getData", 1)
search_method("getQueryParameter", 1)
search_method("getPathSegments", 1)

# Content provider sources
search_method("query", 1)
search_method("insert", 1)
search_method("update", 1)

# Check if sources are actually reachable
get_exported_components(1)
get_deep_links(1)
get_dynamic_receivers(1)
```

**Source is valid ONLY IF:**
- Component is exported (`android:exported="true"`)
- OR deep link is registered (intent-filter with data scheme)
- OR broadcast receiver is dynamically registered without permission

### Step 2: Trace Taint Propagation

Follow the data flow from source to potential sinks:

```bash
# Find methods that process tainted data
get_method_xref("startActivity", 1)           # Intent redirection sinks
get_method_xref("startService", 1)
get_method_xref("sendBroadcast", 1)

# Examine method implementations
get_method_source("com.app.Activity.onCreate")
get_method_source("com.app.Helper.processData")

# Search for variable usage in source code
search_class_key("userInput")                   # Search for variable name
search_class_key("getIntent")                   # Find where intent is used

# Check field propagation (taint stored in fields)
search_class_key("this.input")                  
search_class_key("mUserData")
```

**Propagation Patterns to Track:**
- Direct assignment: `String x = source;`
- Field storage: `this.field = source;`
- Collection storage: `list.add(source);`
- Intent forwarding: `newIntent.putExtra("key", source);`
- String concatenation: `"prefix" + source`

### Step 3: Identify Sanitizers

Check if taint is properly sanitized before reaching sink:

| Sanitizer Type | Pattern | Verification Command |
|----------------|---------|---------------------|
| **Allowlist** | `if (ALLOWED.contains(input))` | `search_class_key("ALLOWED")` |
| **Exact Match** | `if ("expected".equals(input))` | `search_class_key("equals")` |
| **Path Canonicalization** | `file.getCanonicalPath().startsWith(base)` | `search_method("getCanonicalPath", 1)` |
| **Type Conversion** | `Integer.parseInt(input)` | Check if conversion is safe |
| **SQL Parameterization** | `rawQuery("...?", args)` | `search_class_key("Parameterized")` |
| **Intent Locking** | `intent.setComponent(knownSafe)` | `search_method("setComponent", 1)` |

**Incomplete Sanitization = No Sanitization:**
```java
// ❌ NOT sufficient - partial check
if (!input.contains("../")) {
    new File(base, input);  // Still vulnerable to other traversal patterns
}

// ✅ Proper sanitization
File f = new File(base, input);
if (!f.getCanonicalPath().startsWith(base.getCanonicalPath())) {
    throw new SecurityException();
}
```

### Step 4: Identify Sinks

Find dangerous execution points where tainted data ends:

```bash
# Component launch sinks
search_method("startActivity", 1)
search_method("startService", 1)
search_method("sendBroadcast", 1)

# Command execution sinks
search_method("Runtime.getRuntime", 1)
search_method("exec", 1)
search_method("ProcessBuilder", 1)

# SQL injection sinks
search_method("rawQuery", 1)
search_method("execSQL", 1)

# Path traversal sinks
search_method("FileInputStream", 1)
search_method("FileOutputStream", 1)
search_method("openFileOutput", 1)

# WebView sinks
search_method("loadUrl", 1)
search_method("loadData", 1)
search_method("evaluateJavascript", 1)

# Reflection sinks
search_method("Class.forName", 1)
search_method("getMethod", 1)
search_method("invoke", 1)
```

### Step 5: Connect Source → Sink

Use xref analysis to build the complete taint path:

```bash
# Example: Intent Redirection
# 1. Find source
search_method("getParcelableExtra", 1)
# Returns: ActivityA.onCreate() calls getParcelableExtra("forward")

# 2. Trace from source method to sink
get_method_xref("ActivityA.onCreate", 1)
# Shows: ActivityA calls Helper.forwardIntent()

# 3. Check helper method
get_method_source("Helper.forwardIntent")
# Shows: forwardIntent() calls startActivity()

# 4. Verify no sanitizer
search_class_key("ALLOWED_COMPONENTS")  # No results = no sanitizer

# 5. Confirm sink
get_method_xref("startActivity", 1)  # Verify the call exists

# Result: Source → forwardIntent → Sink without sanitizer = VULNERABLE
```

---

## Core Principle: Exploitability Only (Legacy)

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
