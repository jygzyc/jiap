# Installing DECX for Codex

Enable DECX skills in Codex via native skill discovery. Clone the repo, then link `skills/` into `~/.agents/skills/decx`.

## Prerequisites

- Git

## Installation

### macOS / Linux

1. **Clone the DECX repository:**

```bash
git clone https://github.com/jygzyc/decx.git ~/.codex/decx
```

2. **Create the skills symlink:**

```bash
mkdir -p ~/.agents/skills
ln -s ~/.codex/decx/skills ~/.agents/skills/decx
```

3. **Restart Codex** (quit and relaunch the CLI) to discover the skills.

### Windows (PowerShell)

1. **Clone the DECX repository:**

```powershell
git clone https://github.com/jygzyc/decx.git "$HOME\.codex\decx"
```

2. **Create the skills link:**

```powershell
New-Item -ItemType Directory -Force -Path "$HOME\.agents\skills" | Out-Null
New-Item -ItemType SymbolicLink -Path "$HOME\.agents\skills\decx" -Target "$HOME\.codex\decx\skills"
```

If symbolic links are blocked on your machine, use a junction instead:

```powershell
New-Item -ItemType Directory -Force -Path "$HOME\.agents\skills" | Out-Null
New-Item -ItemType Junction -Path "$HOME\.agents\skills\decx" -Target "$HOME\.codex\decx\skills"
```

3. **Restart Codex** to discover the skills.

## Migrating from old bootstrap

If you installed DECX before native skill discovery:

### macOS / Linux

1. **Update the repo:**

```bash
cd ~/.codex/decx && git pull
```

2. **Create the skills symlink** using the installation step above.
3. **Remove the old bootstrap block** from `~/.codex/AGENTS.md` if it references an older DECX bootstrap.
4. **Restart Codex.**

### Windows (PowerShell)

1. **Update the repo:**

```powershell
cd "$HOME\.codex\decx"
git pull
```

2. **Create the skills link** using the installation step above.
3. **Remove the old bootstrap block** from `$HOME\.codex\AGENTS.md` if it references an older DECX bootstrap.
4. **Restart Codex.**

## Verify

### macOS / Linux

```bash
ls -la ~/.agents/skills/decx
```

### Windows (PowerShell)

```powershell
Get-Item "$HOME\.agents\skills\decx" | Format-List FullName,LinkType,Target
```

You should see `~/.agents/skills/decx` pointing to the DECX `skills/` directory.

## Updating

### macOS / Linux

```bash
cd ~/.codex/decx && git pull
```

### Windows (PowerShell)

```powershell
cd "$HOME\.codex\decx"
git pull
```

Skills update instantly through the link.

## Uninstalling

### macOS / Linux

```bash
rm ~/.agents/skills/decx
rm -rf ~/.codex/decx
```

### Windows (PowerShell)

```powershell
Remove-Item "$HOME\.agents\skills\decx"
Remove-Item "$HOME\.codex\decx" -Recurse -Force
```
