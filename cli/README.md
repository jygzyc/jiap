# jiap-cli

JIAP CLI - Java Intelligence Analysis Platform command-line tool.

## Install

```bash
npm install -g jiap-cli
```

## Usage

```bash
jiap <command> [options]
```

### Commands

| Command | Description |
|---------|-------------|
| `jiap process` | Manage JIAP server processes and installation |
| `jiap ard` | Android specific analysis |
| `jiap code` | Common code analysis |

### process

```bash
jiap process check [--install]     # Check environment status
jiap process open <file> [options] # Open and analyze a file (APK, DEX, JAR, etc.)
jiap process close [name] [--all]  # Stop session
jiap process list                  # List running sessions
jiap process status [name]         # Check server status
jiap process install [-p]          # Install jiap-server.jar (-p for prerelease)
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
jiap ard app-manifest                    # Get AndroidManifest.xml
jiap ard main-activity                   # Get main activity name
jiap ard app-application                 # Get Application class name
jiap ard exported-components             # List exported components
jiap ard app-deeplinks                   # List deep link schemes
jiap ard app-receivers                   # List dynamic broadcast receivers
jiap ard system-service-impl <interface> # Find system service implementations
jiap ard all-resources                   # List all resource file names
jiap ard resource-file <res>             # Get resource file content
jiap ard strings                         # Get strings.xml content
```

### code

```bash
jiap code all-classes                    # Get all classes
jiap code class-info <class>             # Get class information
jiap code class-source <class>           # Get class source code
jiap code method-source <sig>            # Get method source
jiap code search-class <keyword>         # Search in class content
jiap code search-method <name>           # Search methods by name
jiap code xref-method <sig>              # Find method callers
jiap code xref-class <class>             # Find class usages
jiap code xref-field <field>             # Find field usages
jiap code implement <interface>          # Find implementations
jiap code subclass <class>               # Find subclasses
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
