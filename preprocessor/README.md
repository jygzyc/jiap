# Preprocessor

A tool for collecting and processing Android framework files from different OEM devices for security research and analysis.

## Environment

### Darwin

```bash
# debugfs installation
brew install e2fsprogs
echo 'export PATH="/opt/homebrew/opt/e2fsprogs/bin:$PATH"' >> ~/.zshrc

# fsck.erofs installation
brew install erofs-utils
```

### Linux

`debugfs` and `fsck.erofs` in `bin` directory

### Windows

Windows support is not currently available. This is a temporary limitation that may be addressed in future updates.

## Usage

### Command Line Usage

```bash
usage: collect_framework.py [-h] [--source-dir SOURCE_DIR] [--out-dir OUT_DIR] [--adb-path ADB_PATH] oem

Collect Android framework files from OEM devices

positional arguments:
  oem                   OEM manufacturer (vivo, oppo, xiaomi, honor, google)

options:
  -h, --help            show this help message and exit
  --source-dir SOURCE_DIR
                        Source directory for collected files
  --out-dir OUT_DIR     Output directory for processed files
  --adb-path ADB_PATH   Path to ADB executable
```

The script will:
1. Connect to Android device via ADB
2. Collect framework files from system directories
3. Process filesystem images (EROFS, EXT4)
4. Extract and organize framework files
5. Generate JAR files from collected frameworks

### Supported File Types
- `.apk` - Android application packages
- `.jar` - Java archive files
- `.apex` - Android Pony EXpress packages
- `.capex` - Compressed APEX packages
- `.dex` - Dalvik executable files

### Supported OEMs
- vivo, oppo, xiaomi, honor, google

## Requirements
- Python 3.8+
- ADB (Android Debug Bridge)
- fsck.erofs, debugfs utilities