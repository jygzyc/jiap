# dex-decompiler

<p align="center"><img width="120" src="./.github/logo.png"></p>
<h2 align="center">DEX-DECOMPILER</h2>

A **DEX to Java decompiler** in pure Rust. It parses DEX files, disassembles Dalvik bytecode, and emits Java-like source with structured control flow.

## How the decompiler works

```
  ┌─────────────┐
  │  DEX bytes  │
  └──────┬──────┘
         │
         v  dex-parser
  ┌─────────────┐     ┌──────────────────┐
  │  DexFile    │────>│ header, strings, │
  │  (in-memory)│     │ types, methods,   │
  │             │     │ class_defs, code  │
  └──────┬──────┘     └──────────────────┘
         │
         v  dex-bytecode (decode_all)
  ┌─────────────┐
  │ Instruction │   Raw Dalvik: const/4, move, if-eqz, invoke-virtual, ...
  │   stream    │
  └──────┬──────┘
         │
         v  basic_blocks + CFG (MethodCfg)
  ┌─────────────┐     ┌─────────────────────────────────────────┐
  │  CFG       │────>│ Blocks, edges, loop headers,              │
  │  (per meth)│     │ instruction_offsets per block            │
  └──────┬──────┘     └─────────────────────────────────────────┘
         │
         v  instructions_to_ir (per block)
  ┌─────────────┐     ┌─────────────────────────────────────────┐
  │  IR        │────>│ Assign { dst, rhs }, Expr, Return, Raw    │
  │  (IrStmt)  │     │ VarId(reg, ver), Call, PendingResult       │
  └──────┬──────┘     └─────────────────────────────────────────┘
         │
         v  PassRunner (InvokeChain, SsaRename, DeadAssign)
  ┌─────────────┐     invoke+move-result+return → Return(Call)
  │  IR (clean) │     dead stores removed (with method-wide used regs)
  └──────┬──────┘
         │
         v  build_regions (if/else, while, switch)
  ┌─────────────┐     ┌─────────────────────────────────────────┐
  │  Region     │────>│ Tree: Block, If(cond,then,else),          │
  │  tree       │     │ Loop(header, body), Switch(cases, default)│
  └──────┬──────┘     └─────────────────────────────────────────┘
         │
         v  emit_region + codegen_ir_lines
  ┌─────────────┐
  │  Java-like  │     Class/method signatures, fields, bodies
  │  source     │     (type inference + var names from IR)
  └─────────────┘
```

**Value flow / tainting** (optional): from the same CFG and a per-instruction read/write map, reaching definitions and def-use/use-def are computed. Given a seed `(offset, reg)`, `value_flow_from_seed` returns all program points that **read** or **write** that value—including when the value is **returned**, **passed to a function** (invoke arg), or copied through moves. **API-source tainting**: `value_flow_from_api_sources(patterns)` treats every `move-result` that receives the return of a matching `invoke` (e.g. `FusedLocationProviderClient.getLastLocation()`) as a seed. **Multi-DEX**: the CLI accepts multiple `-i` inputs; taint mode searches for `CLASS#METHOD` in each DEX in order.

## Features

- **Pure Rust**: No JVM or external tools.
- **DEX parsing**: Full parsing of DEX format (header, string_ids, type_ids, proto_ids, field_ids, method_ids, class_defs, class_data_item, code_item) via [dex-parser](https://github.com/androguard/dex-parser/tree/main/dexparser-rs).
- **Disassembly**: Uses [dex-bytecode](https://github.com/androguard/dex-bytecode) for linear-sweep Dalvik instruction decoding and CFG (basic blocks).
- **Structured control flow**: if/else, `while (!cond)` and `while (true)` with break/continue, **for loops** (init; cond; update), **packed-switch / sparse-switch** → `switch (var) { case … default: … }`.
- **SSA-style IR**: Versioned vars, type inference (params, return, propagation), dead-assign pass with method-wide used regs.
- **Java emission**: Class and method signatures, field declarations, method bodies as Java-like source; optional raw DEX instruction listing as comments before each method.
- **Imports**: Per-class import block; short names in body (e.g. `java.lang.String` → `String`).
- **Try/catch**: From DEX try_item and encoded_catch_handler; body wrapped in `try { … } catch (Type e) { … }` with type names.
- **Enum**: Class extends `java.lang.Enum` and has static final fields of its own type → emitted as `enum Name { A, B, C; … }` (constants first, then `;`, then other members).
- **Annotations**: Class annotations from annotations_directory_item → `@Name` before the class.
- **Constructors**: Emitted as `ClassName(params)` (not `void <init>()`); parameterless `<init>()` in body → `super();`.
- **Anonymous Thread inlining**: Pattern `X.<init>(args);` + `X.start();` → inlined `new Thread() { public void run() { … } }.start();` with inner `run()` body and capture replacement (e.g. `val$o` → outer variable); **synchronized** blocks from monitor-enter/exit; unreachable exception-handler lines after `return;` stripped.
- **Library API**: Parse DEX, decompile classes/methods, **find_method**, get **per-method bytecode and CFG** (nodes/edges) for visualization or tooling.
- **Value flow / tainting**: Reaching definitions, def-use/use-def, propagation from seed or from API sources (e.g. `getLastLocation`).
- **Vulnerability detectors**: PendingIntent scan (`--scan-pending-intent`), full scan (`--scan-vulns`: intent spoofing, RCE, insecure logging, SQL injection, WebView, hardcoded-secrets, IPC), **native Semgrep** (`--scan-semgrep`: general Android rules + [OWASP MASTG](https://github.com/OWASP/mastg/tree/master/rules) via SSA/value-flow + Java/XML patterns).
- **Global taint solver (Mariana Trench–style)**: JSON models (sources / sinks / propagations / sanitizers), rules by kind, call-graph + interprocedural fixpoint, traces, JSON issue report (`--taint-solve`). Includes a port of MT’s **74 end-to-end cases** under `tests/data/mariana_trench/` (`cargo test --test mariana_trench_e2e`).
- **Progress**: With `--output-dir`, progress bar shows current class being decompiled.

## Simplifications

Method bodies and IR are simplified so output looks like idiomatic Java.

### IR passes (before emission)

- **InvokeChainPass**: `invoke(...); vN = <result>; return vN;` → `return method(args);`; `invoke(...); vN = <result>;` → `vN = method(args);`; `invoke(...); return;` left as call + return.
- **ConstructorMergePass**: `vN = new Foo();` + `vN.<init>(args);` → `vN = new Foo(args);` (when in same block).
- **SsaRenamePass**: SSA-style versioned variables.
- **DeadAssignPass**: Removes dead stores (with method-wide used regs).
- **ExprSimplifyPass**: `v0 = v0 + 1` → `v0++`, `v0 = v0 + x` → `v0 += x`; removes redundant self-assigns.

### Method-body simplifications (after emission)

- **Invoke + move-result + return**: `invoke(...); vN = <result>; return vN;` → `return method(args);`.
- **Invoke + move-result**: `invoke(...); vN = <result>;` → `vN = method(args);`.
- **Invoke + return void**: `invoke(...); return;` → `method(args); return;`.
- **Ternary**: `if (cond) { return a; } else { return b; }` → `return cond ? a : b;`.
- **String concatenation**: `new StringBuilder(); sb.append(a); sb.append(b); s = sb.toString();` → `s = a + b;` (and `return sb.toString();` → `return a + b;`).
- **Arithmetic**: `x + -N` → `x - N`.
- **Constructors**: In constructor bodies only, `receiver.<init>();` (no args) → `super();`.
- **Synchronized**: `try { /* monitor-enter(lock) */ … /* monitor-exit */ } catch (Throwable …)` → `synchronized (lock) { … }` (run after try/catch wrapping).
- **Unreachable code**: Lines after `return;` with greater indent are skipped until `}` or `} catch`.
- **Unreachable exception junk** (in inlined Thread run): After `return;`, lines containing `/* move-exception */`, `/* monitor-exit(...) */`, or `throw …;` are removed.


## Dependencies

- [dex-bytecode](https://github.com/androguard/dex-bytecode) – Dalvik disassembly and CFG (basic blocks, switch expansion).
- [dex-parser](https://github.com/androguard/dex-parser/tree/main/dexparser-rs) – DEX file parsing.

Both are pulled from GitHub in `Cargo.toml`; no local paths required.

## Run

```bash
cargo run --release --bin dex-decompile -- -i classes.dex
```
This builds (if needed) and runs the decompiler. See [Usage](#usage) below for more options.

**Speed:** For large DEX files, always use `--release` (e.g. `cargo run --release --bin dex-decompile -- -i classes.dex -d out`). With `-d`/`--output-dir`, decompilation is parallelized, files are written on the fly, and work is split into many chunks for better load balance.

## Usage

### CLI

```bash
# Decompile a DEX file to stdout
cargo run --bin dex-decompile -- -i classes.dex

# Decompile an APK (extracts all classes*.dex) into a package tree
cargo run --release --bin dex-decompile -- -i app.apk -d out

# Multi-DEX merge (or multiple -i DEX/APK paths) into one tree
cargo run --release --bin dex-decompile -- -i classes.dex -i classes2.dex -d out

# Decompile to a single Java file
cargo run --bin dex-decompile -- -i classes.dex -o Main.java

# Modes (jadx-like): restructure (default), simple, fallback
cargo run --bin dex-decompile -- -i app.apk -d out --decompilation-mode fallback

# Auto-deobfuscate short/invalid names; use DEX debug_info names when present
cargo run --bin dex-decompile -- -i app.apk -d out --deobf
cargo run --bin dex-decompile -- -i app.apk -d out --no-debug-info

# Decompile to a directory with package structure (e.g. out/com/example/MyClass.java)
cargo run --bin dex-decompile -- -i classes.dex -d out

# Same, with raw DEX instructions as comments in each method (for debugging).
# Output goes to the directory you pass with -d (e.g. out/), not to any other path.
cargo run --bin dex-decompile -- -i classes.dex -d out --show-bytecode

# Only decompile classes in a package (and subpackages)
cargo run --bin dex-decompile -- -i classes.dex -d out --only-package com.example

# Exclude packages (may be repeated; supports trailing . or .*)
cargo run --bin dex-decompile -- -i classes.dex -d out --exclude android. --exclude kotlin.

# Rename package, class, method, field, or variables in the decompiled output (may be repeated)
cargo run --bin dex-decompile -- -i classes.dex -d out \
  --rename-package com.old=com.new \
  --rename-class "com.old.Main=com.new.MainActivity" \
  --rename-method "com.old.Main#onCreate=myOnCreate" \
  --rename-field "com.old.Main#count=mCount" \
  --rename-variable "com.old.Main#onCreate:p0=context" \
  --rename-variable "com.old.Main#onCreate:result=out"

# Multi-DEX: taint mode searches for CLASS#METHOD in each file in order
cargo run --bin dex-decompile -- -i classes.dex -i classes2.dex --taint-method "com.example.Main#onCreate" --taint-api getLastLocation

# Data flow / tainting: show where value (offset, reg) is read/written in a method
cargo run --bin dex-decompile -- -i classes.dex --taint-method "com.example.Main#onCreate" --taint-offset 0x4 --taint-reg 0

# Taint returns of Android API methods (e.g. getLastLocation)
cargo run --bin dex-decompile -- -i app.dex --taint-method "com.example.Main#onCreate" --taint-api getLastLocation --taint-api getCurrentLocation

# Native Semgrep (general Android + OWASP MASTG rules; SSA/value-flow + Java/XML patterns, no Semgrep binary)
cargo run --release --bin dex-decompile -- -i classes.dex --scan-semgrep
cargo run --release --bin dex-decompile -- -i app.apk --scan-semgrep --semgrep-rules rules/semgrep/android/general.yml
cargo run --release --bin dex-decompile -- -i AndroidManifest.xml --scan-semgrep --semgrep-rules rules/semgrep/android/mastg
```

| Option | Short | Description |
|--------|--------|-------------|
| `--input` | `-i` | Input **DEX and/or APK/ZIP** path(s). APK yields all `classes*.dex`. Decompile merges multi-DEX into one tree. |
| `--output` | `-o` | Output Java file (single file); default: stdout. |
| `--output-dir` | `-d` | Output directory: one `.java` per class under package structure. Decompilation is parallelized for large DEX files. |
| `--decompilation-mode` | `-m` | `restructure` (default), `simple`, or `fallback`. |
| `--deobf` | | Auto-rename short / invalid / non-printable identifiers. |
| `--no-debug-info` | | Do not use DEX `debug_info` for local/parameter names. |
| `--only-package` | | Only decompile classes in this package (e.g. `com.example`). Subpackages included. |
| `--exclude` | | Exclude classes in this package (e.g. `android.`). Repeatable. |
| `--taint-method` | | Data flow: method as `CLASS#METHOD` (e.g. `com.example.Main#onCreate`). Use with `--taint-offset` and `--taint-reg`, or with `--taint-api`. |
| `--taint-offset` | | Instruction byte offset for taint seed (decimal or `0x` hex). Offsets are relative to the method's code start. |
| `--taint-reg` | | Register number for taint seed (e.g. `0` for v0). |
| `--taint-api` | | Taint returns of Android API methods (e.g. `getLastLocation`, `FusedLocationProviderClient.getLastLocation`). Repeatable; matches if method ref contains the pattern. |
| `--scan-pending-intent` | | Scan all methods for PendingIntent creation sites (PITracker-like). Reports whether the base Intent has modifiable fields set and whether the PendingIntent flows to a dangerous sink (e.g. Notification). See [PITracker (WiSec'22)](https://diaowenrui.github.io/paper/wisec22-zhang.pdf). |
| `--show-bytecode` | | Emit raw DEX instructions as comments before each method body and on statement lines (for debugging). Works with both stdout and `-d`/`--output-dir`: written `.java` files will contain the bytecode comments. |
| `--scan-vulns` | | Run all vulnerability detectors on every method: intent spoofing, RCE (dynamic code loading), insecure logging, SQL injection, WebView (unsafe URL + JavaScriptInterface), hardcoded-secrets review, IPC intent validation. Optional: use with `--taint-api` to add logging sources. |
| `--scan-semgrep` | | Native Semgrep-style Android rules (general Android + [OWASP MASTG](https://github.com/OWASP/mastg/tree/master/rules) by default). Uses SSA/value-flow and Java/XML pattern matching; no external Semgrep install. |
| `--semgrep-rules` | | Path to Semgrep YAML file or directory. Default: `rules/semgrep/android/general.yml` + `rules/semgrep/android/mastg/`. |
| `--taint-solve` | | Run the Mariana-Trench–style global taint solver (models, sanitizers, TITO propagations, rules, interprocedural traces). |
| `--taint-config` | | JSON config path (sources/sinks/propagations/sanitizers/rules). Default: embedded Android models. |
| `--taint-config-extra` | | Extra JSON merged on top of the base config. |
| `--taint-output` | | Write `IssueReport` JSON (tool/stats/issues with traces). |
| `--taint-max-iterations` | | Max interprocedural fixpoint iterations (default 8). |
| `--taint-include-framework` | | Also analyze android/androidx/java/kotlin packages (off by default). |
| `--rename-package` | | Rename package in output (`OLD=NEW`). Repeatable. |
| `--rename-class` | | Rename class (`OLD=NEW`, full class names). Repeatable. |
| `--rename-method` | | Rename method (`ClassName#methodName=NEW`). Repeatable. |
| `--rename-field` | | Rename field (`ClassName#fieldName=NEW`). Repeatable. |
| `--rename-variable` | | Rename variable in a method (`ClassName#methodName:oldVar=newVar`). Repeatable. |

When `--output-dir` is set, progress is shown per class. When `--taint-method` is set with either (`--taint-offset` and `--taint-reg`) or `--taint-api`, the tool prints value-flow (reads/writes) and exits without decompiling. When `--scan-pending-intent` is set, the tool scans every method for PendingIntent creation and prints a risk report. When `--taint-solve` is set, the global taint solver runs and optionally writes JSON via `--taint-output`. When `--scan-vulns` is set, the tool runs all detectors and prints one line per finding (category, class#method, sink offset, sink method). When `--scan-semgrep` is set, native Semgrep-style Android rules (general Android + OWASP MASTG by default) run over every method via SSA/value-flow, plus XML rules on plaintext manifests. When both `-o` and `-d` are omitted and neither taint nor scan is used, decompiled Java is printed to stdout.

### Global taint solver (Mariana Trench–style)

`dex-decompiler` includes its own taint solver inspired by [Mariana Trench](https://mariana-tren.ch/):

| MT concept | Implementation |
|---|---|
| Sources / sinks / kinds | JSON models (`TaintConfig`) |
| Propagations (TITO) | Modeled passthroughs (e.g. `StringBuilder.append`) |
| Sanitizers | Clear kinds at matching APIs |
| Rules | `source_kind` → `sink_kind` issue generation |
| Call graph | Built from invoke sites across methods / multi-DEX |
| Interprocedural | Fixpoint over parameter / return summaries |
| Traces / issues | `Issue` + `TraceFrame`; JSON `IssueReport` |

```bash
# Embedded Android defaults
cargo run --release -- -i classes.dex --taint-solve --taint-output issues.json

# Custom models
cargo run --release -- -i classes.dex --taint-solve \
  --taint-config configuration/my_models.json \
  --taint-output issues.json
```

Library entry points: `default_config()`, `solve_dex` / `solve_dexes`, `write_issues_json`.

### Emulator

You can run a specific method in the bytecode emulator and see console output, return value, and (with `--emulate-verbose`, `--emulate-progress`, or `--emulate-interactive`) per-step VM state. Useful for testing decompilation or tracing logic on a DEX (e.g. from [androguard test data](https://github.com/androguard/androguard/tree/master/tests/data)).

**Step-by-step execution** is supported for use in other tools or scripts: the library exposes `emu.step()` which returns a `StepResult` (instruction executed, state after, description, finished flag). Call it in a loop to drive the emulator one instruction at a time. From the CLI, use `--emulate-interactive` to run step-by-step and wait for Enter (or `q`+Enter to stop) after each step.

**Parameter types** for `--emulate-params`: use **semicolon `;`** to separate parameters (so array values can contain commas). Quote the value in the shell (e.g. `'...'`) so `;` is not interpreted as a command separator. In order of method parameters:

| Syntax | Type | Example |
|--------|------|--------|
| `42`, `-1`, `0` | `int` | `5;10` (two params) |
| `123L` | `long` | `0L;42L` |
| `"hello"`, `hello` | `String` | `"key";"value"` |
| `null` | null reference | `null` |
| `[1,2,3]`, `[I]1,2,3` | `int[]` | `[0,1,2,3,4]` |
| `[B]0,1,2`, `[byte]0,1,2` | `byte[]` | `[B]1,2,3,4,5` (key bytes) |

Arrays are passed by reference: the emulator pre-allocates them on the heap and passes `Ref(0)`, `Ref(1)`, … as the parameter values.

**Example: RC4.rc4_crypt (byte[] key, byte[] data)**

Original Java: [androguard/test/RC4.java](https://github.com/androguard/androguard/blob/master/tests/data/AndroguardTest/app/src/main/java/androguard/test/RC4.java) — `public static void rc4_crypt(byte[] key, byte[] data)`.

Using `testdata/classes4.dex` (or the androguard test DEX), the class may be under `androguard.test.RC4` or `tests.androguard.RC4` depending on the DEX; the emulator resolves by simple class name if the full name is not found.

```bash
# Two byte-array params: key (e.g. 5 bytes) and data (e.g. 4 bytes to encrypt in-place).
# Use ";" to separate params so array elements can use commas.
cargo run --release --bin dex-decompile -- -i testdata/classes4.dex \
  --emulate "androguard.test.RC4#rc4_crypt" \
  --emulate-params "[B]1,2,3,4,5;[B]0,0,0,0" \
  --emulate-max-steps 5000
```

If the method is in a different package (e.g. `tests.androguard.RC4`):

```bash
cargo run --release --bin dex-decompile -- -i testdata/androguard_test_classes.dex \
  --emulate "tests.androguard.RC4#rc4_crypt" \
  --emulate-params "[B]1,2,3,4,5;[B]0,0,0,0"
```

**More examples**

```bash
# Int and string params (semicolon separates params)
cargo run --release --bin dex-decompile -- -i app.dex \
  --emulate "pkg.Utils#parse" --emulate-params "42;\"hello\""

# Int array param (single param)
cargo run --release --bin dex-decompile -- -i app.dex \
  --emulate "pkg.ArrayTest#sum" --emulate-params "[1,2,3,4,5]"

# Verbose: print every instruction and VM state (registers, heap) after each step
cargo run --release --bin dex-decompile -- -i app.dex \
  --emulate "pkg.Main#foo" --emulate-params "1;2" --emulate-verbose --emulate-max-steps 50

# Progress bar: current instruction and register state
cargo run --release --bin dex-decompile -- -i app.dex \
  --emulate "pkg.Main#foo" --emulate-progress --emulate-max-steps 2000

# Interactive: after each step, wait for Enter (or q+Enter to stop)
cargo run --release --bin dex-decompile -- -i app.dex \
  --emulate "pkg.Main#foo" --emulate-interactive
```

| Option | Description |
|--------|-------------|
| `--emulate` | Method as `CLASS#METHOD` (e.g. `androguard.test.RC4#rc4_crypt`). Class can be full or simple name. |
| `--emulate-params` | Comma-separated values: scalars and arrays (see table above). |
| `--emulate-max-steps` | Step limit (default 10000). |
| `--emulate-verbose` | Print each instruction and VM state after every step. |
| `--emulate-progress` | Show a progress bar with current instruction and registers. |
| `--emulate-interactive` | Step-by-step: after each instruction, print state and wait for Enter (or `q`+Enter to stop). |

### Library

```rust
use dex_decompiler::{parse_dex, Decompiler, DecompilerOptions, CfgEdgeInfo, CfgNodeInfo, MethodBytecodeRow};

let data = std::fs::read("classes.dex")?;
let dex = parse_dex(&data)?;

// Decompile entire DEX or with filters
let options = DecompilerOptions {
    only_package: Some("com.example".into()),
    exclude: vec!["android.".into()],
    ..Default::default()
};
let decompiler = Decompiler::with_options(&dex, options);
let java = decompiler.decompile()?;

// Per-method bytecode and CFG (for graphs, web UI, etc.)
let encoded = /* EncodedMethod from class_data */;
let (rows, nodes, edges) = decompiler.get_method_bytecode_and_cfg(encoded)?;
// rows: Vec<MethodBytecodeRow> { offset, mnemonic, operands }
// nodes: Vec<CfgNodeInfo> { id, start_offset, end_offset, label }
// edges: Vec<CfgEdgeInfo> { from_id, to_id }
```

### Rename API

You can rename **package**, **class**, **method**, **field**, and **local variables** in the decompiled output by passing a `RenameMap` in `DecompilerOptions`. Keys for method/field/variable use the format `ClassName#memberName` (e.g. `com.example.Main#onCreate`). Variable renames are per method: key `ClassName#methodName` → map of old var name (e.g. `p0`, `result`, `i`) to new name.

```rust
use dex_decompiler::{parse_dex, Decompiler, DecompilerOptions, RenameMap};
use std::collections::HashMap;

let dex = parse_dex(&data)?;
let mut rename = RenameMap::default();
rename.package.insert("com.old".into(), "com.new".into());
rename.class.insert("com.old.Main".into(), "com.new.MainActivity".into());
rename.method.insert("com.old.Main#onCreate".into(), "myOnCreate".into());
rename.field.insert("com.old.Main#count".into(), "mCount".into());
let mut var_map = HashMap::new();
var_map.insert("p0".into(), "context".into());
rename.variable.insert("com.old.Main#onCreate".into(), var_map);

let options = DecompilerOptions {
    rename_map: Some(rename),
    ..Default::default()
};
let decompiler = Decompiler::with_options(&dex, options);
let java = decompiler.decompile()?;
```

When writing to a directory (`decompile_to_dir`), the output path uses the **renamed** class name (e.g. `com/new/MainActivity.java`).

### Value flow / data tainting

To see **where a specific value is read/written** in a method (data tainting), use value-flow analysis. The tracker follows the value when it is **returned**, **passed to a function** (invoke argument), or copied through moves:

```rust
use dex_decompiler::{parse_dex, Decompiler, ValueFlowAnalysisOwned};

let dex = parse_dex(&data)?;
let decompiler = Decompiler::new(&dex);
let encoded = /* EncodedMethod from class_data */;
let owned = decompiler.value_flow_analysis(encoded)?;

// Seed: value in register at instruction offset (relative to method code start)
let result = owned.analysis().value_flow_from_seed(0x0004, 0);
// result.reads: all (offset, reg) that read that value (e.g. return v1, invoke v1)
// result.writes: all (offset, reg) that write it (seed + copies, e.g. move v1,v0)

// Def-use: for a def (offset, reg), list uses
let uses = owned.analysis().def_use(0x0004, 0);
// Use-def: for a use (offset, reg), list reaching defs
let defs = owned.analysis().use_def(0x0010, 0);

// Taint returns of Android API methods (e.g. FusedLocationProviderClient.getLastLocation)
let result = owned.value_flow_from_api_sources(&["getLastLocation", "FusedLocationProviderClient.getLastLocation"]);
// result.reads / result.writes: union of all seeds matching the patterns
```

**Complex examples (propagation via params and return):**

1. **One value flows to both a function call (param) and to return**  
   Pseudocode: `v0 = source(); v1 = v0; foo(v1); v2 = v0; return v2;`  
   Seed at the def of `v0`. Then `result.reads` includes the **invoke** (v1 passed as param) and the **return** (v2); `result.writes` includes the seed and both copies (v1, v2).

2. **Value returned from a callee propagates to this method's return**  
   Pseudocode: `bar(); v0 = result; v1 = v0; return v1;`  
   Seed at **move-result** (e.g. from `value_flow_from_api_sources("getLastLocation")`). Then `result.writes` includes move-result v0 and move v1; `result.reads` includes the **return** v1.

The analysis uses **reaching definitions** over the CFG and per-instruction read/write sets (move, const, return, invoke, if-*, binary ops, iget/iput, etc.). Offsets in value-flow results are **relative to the method's code start** (0, 2, 4, …).

**Vulnerabilities this tainting can help find** — Top Android security issues that map to "sensitive source → dangerous sink" data flow. Use `--taint-api` (or a seed) as the source; then inspect `result.reads` for invokes that are sinks (network, log, SQL, WebView, etc.):

| Vulnerability | Sensitive source (seed) | Sensitive sink |
|---------------|------------------------|----------------|
| **Location / PII leakage** | `getLastLocation`, `getCurrentLocation` | Network, Log, file, Intent |
| **Device ID / IMEI leakage** | `getDeviceId`, `getSubscriberId`, `getAndroidId` | Network, log |
| **Clipboard leakage** | `getPrimaryClip`, `getText` (ClipboardManager) | Network, log |
| **Intent / user input injection** | `getIntent().getStringExtra`, `EditText.getText` | `startActivity`, `WebView.loadUrl`, `rawQuery`/`execSQL` |
| **SQL injection** | User input (intent extras, EditText) | `rawQuery`, `execSQL` |
| **Path traversal** | Intent extras, user input | `File(path)`, `openFileOutput` |
| **Insecure WebView** | Intent extras, user input | `WebView.loadUrl`, `loadDataWithBaseURL` |
| **Sensitive data in logs** | Location, IMEI, tokens, clipboard | `Log.d`, `Log.i`, `println` |

Today the tool gives you **all program points (reads/writes)** for a seed; you (or a sink matcher) check whether any read is an invoke to a dangerous sink. Adding **sink patterns** (e.g. method ref contains `Log.`, `OkHttp`, `rawQuery`) would enable automatic vulnerability reports.

### Python bindings

A [PyO3](https://pyo3.rs/) / [maturin](https://www.maturin.rs/) crate in **`dex-decompiler-py/`** exposes the decompiler to Python:

```bash
cd dex-decompiler-py && maturin build --release && pip install target/wheels/dex_decompiler-*.whl
```

```python
import dex_decompiler

dex = dex_decompiler.parse_dex(open("classes.dex", "rb").read())
java_src = dex.decompile()
dex.decompile_to_dir("out/")
method_java = dex.decompile_method("com.example.MainActivity", "onCreate")
bytecode_rows, cfg_nodes, cfg_edges = dex.get_method_bytecode_and_cfg("com.example.MainActivity", "onCreate")

# Decompile with renames (package, class, method, field, variables)
java_renamed = dex.decompile_with_renames(
    package_renames={"com.example": "com.myname"},
    class_renames={"com.example.Main": "com.myname.MainActivity"},
    method_renames={"com.example.Main#onCreate": "myOnCreate"},
    field_renames={"com.example.Main#count": "mCount"},
    variable_renames={"com.example.Main#onCreate": {"p0": "context", "result": "out"}},
)
```

See [dex-decompiler-py/README.md](dex-decompiler-py/README.md) for full API and installation options.

## Tests

Tests mirror [androguard decompiler tests](https://github.com/androguard/androguard/tree/master/tests):

- **Graph / RPO**: `src/decompile/graph.rs` – immediate dominators and reverse post-order (Tarjan, Cross, LinearVit, etc.).
- **Dataflow**: `tests/decompiler/dataflow.rs` – reach-def, def-use, group_variables (GCD, IfBool).
- **Control flow**: `tests/decompiler/control_flow.rs` – return, if/else, while, loop exit.
- **Equivalence**: `tests/decompiler/equivalence.rs` – parse-fail, minimal DEX, try/catch comment, switch packed cases. Optional tests for simplification and arrays run only if androguard test data exists under `tests/data/APK/` (Test.dex, FillArrays.dex).
- **Value flow / tainting**: `src/decompile/value_flow.rs` (unit) and `tests/decompiler/value_flow.rs` (integration) – reaching definitions, def-use/use-def, propagation from a seed (return, pass to function, transitive copies, complex flows: param+return, callee return→return).
- **Privacy + vuln demo**: `testdata/privacy_vuln_demo/` — compile-able APK with intra-procedural PII leaks (default kinds + catalog extras) and one method per `scan_dex_parallel` detector category; `tests/decompiler/privacy_vuln_demo.rs` locks taint, extra-PII merge, VF detectors, and PendingIntent (`cargo test --test decompiler_tests privacy_vuln_demo`). Rebuild with `bash testdata/privacy_vuln_demo/build.sh`.
- **Renames**: `tests/decompiler/renames.rs` – package, class, method, field, and variable renames via `RenameMap` and `decompile_with_renames` (Python).

```bash
cargo test
cargo test -- --ignored   # with fixture DEXs
```

## References

- [Android DEX format](https://source.android.com/docs/core/runtime/dex-format)
- [Dalvik bytecode](https://source.android.com/docs/core/runtime/dalvik-bytecode)
- [androguard decompiler](https://github.com/androguard/androguard/tree/master/androguard/decompiler)
- [jadx](https://github.com/skylot/jadx/tree/master/jadx-core/src/main/java/jadx)
- [jadx gap plan](docs/JADX_GAP_PLAN.md) — living backlog vs jadx (P0–P2)

## License

Same as the repository (see [LICENSE](LICENSE)).
