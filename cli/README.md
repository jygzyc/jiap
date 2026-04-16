# decx-cli

DECX CLI - Decompiler + X command-line tool.

## Install

```bash
npm install -g @jygzyc/decx-cli
```

## Usage

```bash
decx <command> [options]
```

### Commands

| Command | Description |
|---------|-------------|
| `decx process` | Manage DECX server processes |
| `decx self` | Install and update decx-server.jar |
| `decx ard` | Android specific analysis |
| `decx code` | Common code analysis |

### process

```bash
decx process check               # Check environment status
decx process open <file> [options] # Open and analyze a file (APK, DEX, JAR, etc.)
decx process close [name] [--all]  # Stop session
decx process list                  # List running sessions
decx process status [name]         # Check server status
```

### self

```bash
decx self install [-p]          # Install decx-server.jar (-p for prerelease)
decx self update [-p]           # Update decx-server.jar (-p for prerelease)
```

**open options:**

| Option | Description |
|--------|-------------|
| `-P, --port <port>` | Server port |
| `--force` | Force start even if session exists |
| `-n, --name <name>` | Custom session name |

All standard [jadx-cli options](https://github.com/skylot/jadx) are passed through directly. Common ones: `--deobf`, `--no-res`, `--show-bad-code`, `-j`/`--threads-count`, `--no-imports`, `--no-debug-info`, `--escape-unicode`, `--log-level`.

### ard

```bash
decx ard app-manifest                    # Get AndroidManifest.xml
decx ard main-activity                   # Get main activity name
decx ard app-application                 # Get Application class name
decx ard exported-components             # List exported components
decx ard app-deeplinks                   # List deep link schemes
decx ard app-receivers                   # List dynamic broadcast receivers
decx ard system-service-impl <interface> # Find system service implementations
decx ard all-resources                   # List all resource file names
decx ard resource-file <res>             # Get resource file content
decx ard strings                         # Get strings.xml content
decx ard get-aidl                        # Get all AIDL interfaces
decx ard framework collect               # Collect framework files from the connected device
decx ard framework process <oem>         # Process local framework source files and pack the framework jar
decx ard framework run                   # Collect, process, pack, and optionally open
decx ard framework open [jar]            # Open the generated framework jar or a provided JAR
```

### ard framework

`decx ard framework` integrates the archived preprocessor workflow into the native CLI.
It supports both end-to-end collection from a connected Android device and offline
processing of an existing local framework dump.

Framework artifacts are not treated as a separate runtime session type.
Only `framework open` and `framework run` start a DECX server, and once opened they
are managed exactly like any other DECX session through `decx process list`,
`decx process status`, and `decx process close`.

```bash
decx ard framework run
decx ard framework run --no-open
decx ard framework collect --serial emulator-5554
decx ard framework process xiaomi --out-dir ~/.decx/output/framework/xiaomi
decx ard framework open
decx ard framework open ~/.decx/output/framework/xiaomi/framework_xiaomi_k70_ultra.jar
decx process list
decx process close framework_xiaomi_k70_ultra
```

**Common framework options:**

| Option | Description |
|--------|-------------|
| `--source-dir <dir>` | Framework source directory |
| `--out-dir <dir>` | Framework output directory |
| `--adb-path <path>` | ADB executable path |
| `--serial <serial>` | ADB device serial |
| `--clean-source` | Remove `source/` after the command finishes successfully |

**run/open options:**

| Option | Description |
|--------|-------------|
| `--no-open` | Do not open the generated framework jar after packing |
| `-n, --name <name>` | Custom DECX session name when opening the jar |
| `-P, --port <port>` | Server port when opening the jar |

**Artifact naming**

Packed framework artifacts are named:

```text
framework_<brand>_<vendor>.jar
```

Artifact segments are resolved like this:

1. `brand` is the detected device OEM for `collect/run`, or the explicit `oem` passed to `process`
2. `vendor` comes from previously collected metadata stored in `.meta.json`
3. During `framework collect` / `framework run`, metadata is populated from:
   `adb shell getprop ro.product.model`
4. If metadata is unavailable, `vendor` falls back to `unknown`
5. Framework artifact metadata is stored alongside the output under `.artifact.json`
6. `framework open` and `framework run` create normal DECX process sessions with the default session name `framework_<brand>_<vendor>`
7. `framework collect` and `framework process` only prepare artifacts; they do not create a running session

**Platform notes**

- Windows is not supported for `decx ard framework` yet
- The CLI ships packaged extractor binaries for supported Darwin/Linux targets
- `framework open` and `framework run` reuse the normal `decx process open` flow
- After a framework jar is opened, use the existing `decx process` commands to inspect or close that session
- `framework open` uses an explicit jar path when provided; otherwise it resolves the jar for the currently connected device OEM
- Temporary processing directories are removed only after the command reaches its final step
- `source/` is preserved by default and is removed only when `--clean-source` is set
- If `adb devices` reports exactly one connected device, framework commands use it automatically
- If multiple devices are connected, pass `--serial <serial>` to select the target device
- `collect` and `run` detect the device OEM from adb properties; `process` requires an explicit `oem`

### code

```bash
decx code all-classes                    # Get all classes
decx code class-info <class>             # Get class information
decx code class-source <class>           # Get class source code (--smali for Smali)
decx code method-source <sig>            # Get method source (--smali for Smali)
decx code search-class <keyword>         # Search in class content
decx code search-method <name>           # Search methods by name
decx code xref-method <sig>              # Find method callers
decx code xref-class <class>             # Find class usages
decx code xref-field <field>             # Find field usages
decx code implement <interface>          # Find implementations
decx code subclass <class>               # Find subclasses
```

### Global options

| Option | Description |
|--------|-------------|
| `-s, --session <name>` | Target session name |
| `-P, --port <port>` | Server port (default: 25419) |
| `--page <n>` | Page number (default: 1) |

## Method signature format

For `code method-source` and `code xref-method`:

```
package.Class.methodName(paramType1,paramType2):returnType
```

Example: `com.example.MainActivity.onCreate(android.os.Bundle):void`

## Development

```bash
npm install       # install dependencies
npm run build     # build to dist/
npm test          # run tests
npm run lint      # lint check
npm run dev       # run locally
```

## License

GNU-3.0
