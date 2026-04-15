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
| `decx process` | Manage DECX server processes and installation |
| `decx ard` | Android specific analysis |
| `decx code` | Common code analysis |

### process

```bash
decx process check [--install]     # Check environment status
decx process open <file> [options] # Open and analyze a file (APK, DEX, JAR, etc.)
decx process close [name] [--all]  # Stop session
decx process list                  # List running sessions
decx process status [name]         # Check server status
decx process install [-p]          # Install decx-server.jar (-p for prerelease)
```

**open options:**

| Option | Description |
|--------|-------------|
| `-P, --port <port>` | Server port |
| `--force` | Force start even if session exists |
| `-n, --name <name>` | Custom session name |
| `--json` | JSON output |

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
```

### code

```bash
decx code all-classes                    # Get all classes
decx code class-info <class>             # Get class information
decx code class-source <class>           # Get class source code
decx code method-source <sig>            # Get method source
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
| `--json` | JSON output |
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
