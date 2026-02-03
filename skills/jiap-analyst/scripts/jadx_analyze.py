#!/usr/bin/env python3
"""
JADX Analyze Script - Simple decompiler wrapper
Executes headless JADX decompilation on input file.

Usage:
    python jadx_analyze.py <path/to/file> [--output DIR] [--threads N]

Examples:
    python jadx_analyze.py /path/to/app.apk
    python jadx_analyze.py /path/to/app.apk --output ./analysis
    python jadx_analyze.py /path/to/dex.jar --threads 8
"""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

DEFAULT_JADX_VERSION = "1.5.3"
JIAP_DIR = Path.home() / ".jiap"
JADX_DIR = Path.home() / ".jiap" / "jadx"


def find_jadx():
    jadx_cmd = shutil.which("jadx")
    if jadx_cmd:
        return Path(jadx_cmd)

    local_jadx = JADX_DIR / "bin" / "jadx"
    if local_jadx.exists():
        return local_jadx

    return None


def decompile(input_file, output_dir, jadx_bin, no_res=False, threads=4):
    cmd = [str(jadx_bin), "-d", str(output_dir), "--threads", str(threads)]

    if no_res:
        cmd.append("--no-res")

    cmd.append("--show-bad-code")
    cmd.append("--no-imports")
    cmd.append("--no-inline-anonymous")
    cmd.append("--use-source-name-as-class-name-alias")
    cmd.append("-Pdex-input.verify-checksum=no")

    cmd.append(str(input_file))

    print(f"Running: {' '.join(cmd)}")

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=600)

        if result.returncode == 0:
            print(f"Decompilation complete: {output_dir}")
            return True
        else:
            print(f"Decompilation failed:")
            print(result.stderr)
            return False
    except subprocess.TimeoutExpired:
        print("Decompilation timed out (10 minutes)")
        return False
    except Exception as e:
        print(f"Decompilation error: {e}")
        return False


def main():
    parser = argparse.ArgumentParser(description="Decompile file using JADX")
    parser.add_argument("input", type=str, help="Input file (APK, DEX, JAR, etc.)")
    parser.add_argument(
        "-o", "--output", type=str, default=None, help="Output directory"
    )
    parser.add_argument("--threads", type=int, default=4, help="Number of threads")
    args = parser.parse_args()

    input_file = Path(args.input).resolve()
    if not input_file.exists():
        print(f"Input file not found: {input_file}")
        return 1

    if args.output:
        output_dir = Path(args.output).resolve()
    else:
        output_dir = JIAP_DIR/ f"{input_file.stem}_jadx"

    jadx_bin = find_jadx()
    if not jadx_bin:
        print("JADX not found in PATH or ~/.jiap/jadx/")
        print("Install JADX first or run: jadx_check.py --install")
        return 1

    print(f"Decompiling {input_file.name}...")
    if decompile(input_file, output_dir, jadx_bin, threads=args.threads):
        print(f"Output: {output_dir}")
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
