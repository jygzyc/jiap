#!/usr/bin/env python3
"""
JADX Check Script - Cross-platform simplified version
Checks JADX installation, JIAP plugin, and JIAP server status.
"""

import argparse
import json
import platform
import shutil
import subprocess
import sys
import urllib.request
import urllib.error
from pathlib import Path

DEFAULT_JADX_VERSION = "1.5.3"
DEFAULT_PORT = 25419
DEFAULT_URL = f"http://127.0.0.1:{DEFAULT_PORT}/health"
JIAP_DIR = Path.home() / ".jiap"
JADX_DIR = Path.home() / ".jiap" / "jadx"


def find_jadx():
    jadx_cmd = shutil.which("jadx")
    if jadx_cmd:
        return Path(jadx_cmd)

    local_jadx = JADX_DIR / "bin" / "jadx"
    if local_jadx.exists():
        return local_jadx

    if platform.system() == "Windows":
        win_jadx = JADX_DIR / "bin" / "jadx.bat"
        if win_jadx.exists():
            return win_jadx

    return None


def install_jadx():
    try:
        JADX_DIR.mkdir(parents=True, exist_ok=True)
        download_url = f"https://github.com/skylot/jadx/releases/download/v{DEFAULT_JADX_VERSION}/jadx-{DEFAULT_JADX_VERSION}.zip"
        zip_path = JADX_DIR / "jadx.zip"

        print(f"Downloading JADX v{DEFAULT_JADX_VERSION}...")
        urllib.request.urlretrieve(download_url, zip_path)

        import zipfile

        with zipfile.ZipFile(zip_path, "r") as zip_ref:
            zip_ref.extractall(JADX_DIR)

        zip_path.unlink()

        if platform.system() != "Windows":
            jadx_bin = JADX_DIR / "bin" / "jadx"
            if jadx_bin.exists():
                jadx_bin.chmod(0o755)

        print("JADX installed")
        return True
    except Exception as e:
        print(f"Failed to install JADX: {e}")
        return False


def install_jiap_plugin(jadx_path):
    try:
        print("Installing JIAP plugin via jadx plugins...")
        result = subprocess.run(
            [str(jadx_path), "plugins", "--install", "github:jygzyc:jiap"],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            print("JIAP plugin installed")
            return True
        else:
            print(f"Failed to install plugin: {result.stderr}")
            return False
    except Exception as e:
        print(f"Failed to install JIAP plugin: {e}")
        return False


def check_jiap_server(url):
    try:
        req = urllib.request.Request(url, method="GET")
        req.add_header("Accept", "application/json")
        with urllib.request.urlopen(req, timeout=3) as response:
            if response.status == 200:
                return True, json.loads(response.read().decode("utf-8"))
            return False, {"error": f"HTTP {response.status}"}
    except urllib.error.HTTPError as e:
        return False, {"error": f"HTTP {e.code}"}
    except urllib.error.URLError as e:
        return False, {"error": str(e.reason)}
    except Exception as e:
        return False, {"error": str(e)}


def main():
    parser = argparse.ArgumentParser(description="Check JADX and JIAP installation")
    parser.add_argument(
        "--url", type=str, default=DEFAULT_URL, help="JIAP health check URL"
    )
    parser.add_argument(
        "--install", action="store_true", help="Install missing components"
    )
    args = parser.parse_args()

    results = []

    # Check JIAP Server
    server_ok, server_info = check_jiap_server(args.url)
    if server_ok:
        results.append(
            (
                "JIAP Server",
                True,
                f"running on port {server_info.get('port', DEFAULT_PORT)}",
            )
        )
    else:
        results.append(
            ("JIAP Server", False, server_info.get("error", "unknown error"))
        )

    # Check JADX
    jadx_path = find_jadx()
    if jadx_path:
        results.append(("JADX", True, str(jadx_path)))
    else:
        results.append(("JADX", False, "not found"))
        if args.install:
            if install_jadx():
                jadx_path = find_jadx()
                if jadx_path:
                    results.append(("JADX", True, f"installed at {jadx_path}"))

    # Check JIAP Plugin
    if jadx_path:
        try:
            result = subprocess.run(
                [str(jadx_path), "plugins", "--list"],
                capture_output=True,
                text=True,
            )
            if result.returncode == 0 and "jiap" in result.stdout.lower():
                results.append(("JIAP Plugin", True, "installed"))
            else:
                results.append(("JIAP Plugin", False, "not installed"))
                if args.install:
                    if install_jiap_plugin(jadx_path):
                        results.append(("JIAP Plugin", True, "installed"))
        except Exception as e:
            results.append(("JIAP Plugin", False, str(e)))
    else:
        results.append(("JIAP Plugin", False, "JADX not available"))

    # Print results
    max_len = max(len(name) for name, _, _ in results)
    for name, ok, info in results:
        status = "✓" if ok else "✗"
        print(f"{status} {name:<{max_len}} {info}")

    # Exit code
    ok_count = sum(1 for _, ok, _ in results if ok)
    if ok_count == len(results):
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
