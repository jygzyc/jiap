import os
import subprocess
from concurrent.futures import ThreadPoolExecutor
import concurrent.futures
import shlex
from pathlib import Path

try:
    import tomllib as toml
except ImportError:
    toml = None

from config_data import Config # Removed AdbConfig import as it's not directly used here

class FileSynchronizer:
    """Core module for ADB file synchronization using a Config object."""

    def __init__(self, config_path=None, **kwargs):
        self.config = self._load_config(config_path)
        self._init_parameters(kwargs)
        self.dir_cache = set()

    def _load_config(self, config_path):
        """Load configuration file"""
        if not config_path and toml:
            default_path = os.path.join(os.path.dirname(__file__), "config.toml")
            config_path = default_path if os.path.exists(default_path) else None

        if config_path and toml:
            try:
                with open(config_path, "rb") as f:
                    return toml.load(f)
            except Exception as e:
                print(f"Config load warning: {str(e)}")
        return {}

    def _init_parameters(self, kwargs):
        """Initialize runtime parameters"""
        # Merge configuration layers
        general_config = self.config.get("general", {})
        adb_config = self.config.get("adb", {})

        self.source_dir = kwargs.get(
            'source_dir',
            general_config.get("source_dir", "./Source")
        )
        self.file_types = kwargs.get(
            'file_types',
            general_config.get("file_types", ["vdex", "odex", "oat", "jar", "apk"])
        )
        self.search_paths = kwargs.get(
            'search_paths',
            general_config.get("search_paths", ["."])  # Default to entire device
        )
        self.max_workers = kwargs.get(
            'max_workers',
            general_config.get("max_workers", os.cpu_count() * 2)
        )
        self.adb_path = kwargs.get(
            'adb_path',
            adb_config.get("path", "adb")
        )

    def scan_device_files(self):
        """Scan device files"""
        find_cmd = self._build_find_command()
        adb_cmd = f"{self.adb_path} shell {find_cmd}"
        print(f"[D] cmd: {adb_cmd}")

        try:
            output = subprocess.check_output(
                adb_cmd,
                shell=True,
                stderr=subprocess.STDOUT,
                encoding='utf-8',
                text=True
            )
            return self._process_scan_output(output)
        except subprocess.CalledProcessError as e:
            print(f"Scan failed: {e.output.strip()}")
            return []

    def _build_find_command(self):
        """Build file search command with path restrictions"""
        if not self.file_types:
            raise ValueError("File type list cannot be empty")

        # Escape paths containing spaces or special characters
        escaped_search_paths = [shlex.quote(p) for p in self.search_paths]

        # Build name conditions
        # Use self.file_types which now refers to config.adb_file_types
        name_conditions = " -o ".join(f"-name '*.{ft}'" for ft in self.file_types)

        commands = [f"find {path} -type f {name_conditions};" for path in escaped_search_paths]
        cmd = ''.join(commands)

        return f'"{cmd}"'

    def sync_files(self, file_list, progress_callback=None):
        """Optimized synchronization entry point"""

        dirs_to_create = set()
        for fpath in file_list:
            rel_path = fpath.lstrip('/')
            dst_dir = os.path.dirname(os.path.join(self.source_dir, rel_path))
            dirs_to_create.add(dst_dir)

        self._batch_create_dirs(dirs_to_create)

        with ThreadPoolExecutor(max_workers=self.max_workers) as executor:
            futures = {
                executor.submit(
                    self._optimized_pull,
                    fpath,
                    progress_callback
                ): fpath for fpath in file_list
            }

            for future in concurrent.futures.as_completed(futures):
                try:
                    future.result()
                except Exception as e:
                    file_path = futures[future]
                    self._notify_progress(progress_callback, file_path, "error", str(e))

    def _batch_create_dirs(self, dir_list):
        """Batch create directories with caching"""
        new_dirs = [d for d in dir_list if d not in self.dir_cache]
        for d in new_dirs:
            os.makedirs(d, exist_ok=True)
        self.dir_cache.update(new_dirs)

    def _process_scan_output(self, output):
        """Process scan results"""
        valid_files = []
        for line in output.splitlines():
            line = line.strip()
            if line and not any(msg in line for msg in ["Permission denied", "No such file"]):
                valid_files.append(line)
        return valid_files

    def _optimized_pull(self, file_path, callback):
        """Optimized single file transfer"""
        dst_file = os.path.join(self.source_dir, file_path.lstrip('/'))
        try:
            result = subprocess.run(
                [self.adb_path, "pull", file_path, os.path.dirname(dst_file)],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                timeout=300
            )

            if result.returncode == 0:
                self._notify_progress(callback, file_path, "success", None)
            else:
                error = result.stdout.strip() or "Unknown error"
                self._notify_progress(callback, file_path, "failed", error)
        except Exception as e:
            self._notify_progress(callback, file_path, "error", str(e))

    def _notify_progress(self, callback, file_path, status, message):
        """Handle progress notifications"""
        if callback:
            callback({
                "file": file_path,
                "status": status,
                "message": message,
                "local_path": os.path.join(self.source_dir, file_path)
            })
        else:
            status_map = {
                "exists": f"[SKIPPED] File exists: {file_path}",
                "success": f"[SUCCESS] Synced: {file_path}",
                "failed": f"[FAILED] {file_path} - {message}",
                "error": f"[ERROR] {file_path} - {message}"
            }
            print(status_map.get(status, f"[UNKNOWN] {file_path}"))

def main():
    """Command line entry point"""
    sync = FileSynchronizer()
    files = sync.scan_device_files()
    print(f"Found {len(files)} files to sync")
    sync.sync_files(files)

if __name__ == "__main__":
    main()