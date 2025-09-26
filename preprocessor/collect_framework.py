# -*- coding: utf-8 -*-
# This file is part of jiap
# See the file 'LICENSE' for copying permission.

import functools
import os
import enum
import shutil
import logging
import tempfile
import subprocess
import zipfile
import shlex
import argparse
from multiprocessing import Pool
from typing import Dict, Optional, List, Set
from abc import ABC, abstractmethod
from dataclasses import dataclass

# Setup logging
logger = logging.getLogger(__name__)

# Configure logging to prevent duplicate logs in multiprocessing
import multiprocessing
if multiprocessing.current_process().name == "MainProcess":
    if not logger.handlers:
        handler = logging.StreamHandler()
        formatter = logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s')
        handler.setFormatter(formatter)
        logger.addHandler(handler)
        logger.setLevel(logging.DEBUG)

MAX_PROCESS = os.cpu_count() or 1
OEM_DICT = {
    "vivo": ["/system/framework", "/system/apex", "/vendor/framework", "/system_ext/framework"],
    "oppo": ["/system/framework", "/system/apex", "/system_ext/framework"],
    "xiaomi": ["/system/framework", "/system/apex", "/system_ext/framework", "/vendor/framework"],
    "honor": ["/system/framework", "/system/apex", "/vendor/framework", "/system_ext/framework"],
    "google": ["/system/framework", "/system/apex", "/vendor/framework", "/system_ext/framework"]
}
FILE_TYPES = [".apk", ".jar", ".apex", ".capex", ".dex"]


@dataclass
class TaskResult:
    """Structure for task execution results"""
    success: bool
    task_id: str
    error: Optional[str] = None

    def __str__(self):
        if self.success:
            return f"{self.task_id}"
        else:
            return f"{self.task_id}: {self.error}" if self.error else f"{self.task_id}: Unknown error"


class BaseTaskCollector(ABC):
    """
    Abstract base class for file collection from various sources
    """
    def __init__(self):
        pass
    
    @abstractmethod
    def collect_tasks(self, args) -> List:
        """Collect tasks to be processed"""
        pass

    @abstractmethod
    def single_run(self, arg):
        """Perform a single task and return results as a TaskResult object"""
        pass

    def run(self, args=None) -> List[TaskResult]:
        tasks = self.collect_tasks(args)
        results = []
        try:
            with Pool(processes=MAX_PROCESS) as pool:
                results = pool.map(self.single_run, tasks)
        except Exception:
            raise
        except KeyboardInterrupt:
            raise
        finally:
            pool.close()
            pool.join()
        
        validated_results = []
        for result in results:
            if isinstance(result, TaskResult):
                validated_results.append(result)
            else:
                # Handle any non-TaskResult results
                validated_results.append(TaskResult(success=False, task_id="unknown", error=str(result)))
        
        return validated_results


class FileCollector(BaseTaskCollector):
    """
    Collect files from local or device sources
    """
    def __init__(self, 
                 oem: str,
                 file_types: List[str], 
                 source_dir: str, 
                 adb_path: str = "adb"):
        super().__init__()
        self.oem = oem.lower() if oem.lower() in OEM_DICT.keys() else None
        self.file_types = file_types if file_types is not [] else FILE_TYPES
        self.source_dir = source_dir
        self.adb_path = adb_path

        if not os.path.exists(self.source_dir):
            os.makedirs(self.source_dir, exist_ok=True)

    @property
    def search_paths(self) -> List[str]:
        """Get search paths based on OEM"""
        if self.oem:
            return OEM_DICT[self.oem]
        return ["."]

    @functools.lru_cache
    def _build_find_command(self):
        """Build file search command with path restrictions"""
        escaped_search_paths = [shlex.quote(p) for p in self.search_paths]
        name_conditions = " -o ".join(f"-name '*{ft}'" for ft in self.file_types)
        commands = [f"find {path} -type f {name_conditions};" for path in escaped_search_paths]
        cmd = ''.join(commands)
        return f'"{cmd}"'
    
    def _process_scan_output(self, output: str) -> List[str]:
        """Process scan results"""
        valid_files = []
        for line in output.splitlines():
            line = line.strip()
            if line and not any(msg in line for msg in ["Permission denied", "No such file"]):
                valid_files.append(line)
        return valid_files

    def collect_tasks(self, args) -> List:
        """Scan device files"""
        super().collect_tasks(args)
        find_cmd = self._build_find_command()
        adb_cmd = f"{self.adb_path} shell {find_cmd}"

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
            logger.error(f"Scan failed: {e.output.strip()}")
            return []

    def single_run(self, arg):
        """Pull a single file from device"""
        file_path: str = arg
        dst_file = os.path.join(self.source_dir, file_path.lstrip('/'))
        os.makedirs(os.path.dirname(dst_file), exist_ok=True)
        try:
            result = subprocess.run(
                [self.adb_path, "pull", file_path, dst_file],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                timeout=300
            )
            if result.returncode == 0:
                return TaskResult(success=True, task_id=file_path)
            else:
                error = result.stdout.strip() or "Unknown error"
                return TaskResult(success=False, task_id=file_path, error=error)
        except Exception as e:
            return TaskResult(success=False, task_id=file_path, error=str(e))     
    
    def run(self, args=None) -> List[TaskResult]:
        try:
            results = super().run(args)
            
            # Add logging and printing in the child class
            success_count = sum(1 for result in results if result.success)
            error_count = len(results) - success_count
            
            logger.info(f"FileCollector summary:")
            logger.info(f"  Total tasks: {len(results)}")
            logger.info(f"  Successful: {success_count}")
            logger.info(f"  Failed: {error_count}")
            
            if error_count > 0:
                logger.warning("Failed tasks:")
                for result in results:
                    if not result.success:
                        logger.warning(f"  {result.task_id}: {result.error}")
            
            return results
        except Exception as e:
            logger.error(f"FileCollector run failed: {str(e)}")
            raise


class BaseProcessor(ABC):
    """
    Abstract base class for framework processing
    """
    def __init__(self, 
                 input_file: str, 
                 output_dir: str,
                 tool_paths: Optional[Dict[str, str]] = None):
        self.input_file: str = input_file
        self.output_dir: str = output_dir
        self.tool_paths: Dict[str, str] = tool_paths or {}

        if not self.input_file:
            raise ValueError(f"Input path cannot be empty: {input_file}")
        if not os.path.exists(self.input_file):
            raise FileNotFoundError(f"Input path does not exist: {self.input_file}")
        if not os.path.exists(self.output_dir):
            os.makedirs(self.output_dir, exist_ok=True)

    def __repr__(self) -> str:
        return f"<BaseProcessor: {self.input_file}, Output: {self.output_dir}>"

    @property
    def fullname(self) -> str:
        """
        Return the filename of input file
        """
        if not self.input_file:
            raise ValueError("input_file is None or empty")
        return os.path.basename(self.input_file)
    
    @property
    def filename(self) -> str:
        """
        Return the name of input file without extension
        """
        if not self.input_file:
            raise ValueError("input_file is None or empty")
        return os.path.splitext(os.path.basename(self.input_file))[0]
    
    @property
    def extension(self) -> str:
        if not self.input_file:
            raise ValueError("input_file is None or empty")
        return os.path.splitext(self.input_file)[-1].lstrip('.').lower()

    @abstractmethod
    def process(self) -> bool:
        """ 
        Process the input file and generate output file path.
        """
        pass

    @abstractmethod
    def clean(self) -> None:
        """Explicitly cleanup resources and release memory"""
        self.input_file = ""
        self.output_dir = ""
        self.tool_paths = {}


class JarProcessor(BaseProcessor):

    def __init__(self, input_file: str, output_dir: str):
        super().__init__(input_file, output_dir)
        if self.extension != 'jar':
            raise ValueError("Input file must be a JAR file")
        
    def __repr__(self) -> str:
        return f"<JarProcessor: {self.input_file}, Output: {self.output_dir}>"

    def process(self) -> bool:
        try:
            with zipfile.ZipFile(self.input_file, 'r') as zf:
                for name in zf.namelist():
                    if name.lower().endswith('.dex'):
                        dex_name = os.path.basename(name)
                        target = os.path.join(self.output_dir, f"{self.filename}_{dex_name}")
                        with zf.open(name) as src, open(target, 'wb') as dst:
                            dst.write(src.read())
            return True
        except Exception as e:
            logger.error(f"Failed to process JAR file: {self.input_file}, Error: {str(e)}")
            return False
    
    def clean(self) -> None:
        super().clean()


class ApkProcessor(BaseProcessor):
    """
    Processor for APK files
    """

    def __init__(self, input_file: str, output_dir: str):
        super().__init__(input_file, output_dir)
        if self.extension != 'apk':
            raise ValueError("Input file must be an APK file")
    
    def __repr__(self) -> str:
        return f"<ApkProcessor: {self.input_file}, Output: {self.output_dir}>" 

    def process(self) -> bool:
        try:
            with zipfile.ZipFile(self.input_file, 'r') as zf:
                for name in zf.namelist():
                    if name.lower().endswith('.dex'):
                        dex_name = os.path.basename(name)
                        target = os.path.join(self.output_dir, f"{self.filename}_{dex_name}.dex")
                        with zf.open(name) as src, open(target, 'wb') as dst:
                            dst.write(src.read())
            return True
        except Exception as e:
            logger.error(f"Failed to process APK file: {self.input_file}, Error: {str(e)}")
            return False
    
    def clean(self) -> None:
        super().clean()


class ApexProcessor(BaseProcessor):

    FS_TYPES = [
        ('f2fs', 1024, b'\x10\x20\xf5\xf2'),
        ('ext4', 1024 + 0x38, b'\123\357'),
        ('erofs', 1024, b'\xe2\xe1\xf5\xe0'),
        ('ext2', 1024 + 0x38, b'\x53\xEF')
    ]

    class ApexType(enum.Enum):
        INVALID = 0
        UNCOMPRESSED = 1
        COMPRESSED = 2
    
    def __init__(self, 
                 input_file: str, 
                 output_dir: str,
                 tool_paths: Dict[str, str],
                 apex_out: Optional[str] = None):
        super().__init__(input_file, output_dir, tool_paths)
        self.apex_out_dir = apex_out or tempfile.mkdtemp(prefix="apex_tmp")
        if not os.path.exists(self.apex_out_dir):
            os.makedirs(self.apex_out_dir, exist_ok=True)
        if self.extension != 'apex' and self.extension != 'capex':
            raise ValueError("Input file must be an APEX file")
        
    def __repr__(self) -> str:
        return f"<ApexProcessor: {self.input_file}, Output: {self.output_dir}>"

    def _get_apex_type(self, apex_file: str) -> ApexType:
        with zipfile.ZipFile(apex_file, 'r') as zip_file:
            names = zip_file.namelist()
            has_payload = 'apex_payload.img' in names
            has_original = 'original_apex' in names
            if has_payload and has_original:
                return self.ApexType.INVALID
            if has_payload:
                return self.ApexType.UNCOMPRESSED
            if has_original:
                return self.ApexType.COMPRESSED
            return self.ApexType.INVALID
    
    @functools.cached_property
    def source_apex_type(self) -> ApexType:
        """Determine the type of input APEX file."""
        return self._get_apex_type(self.input_file)
    
    @functools.cached_property
    def apex_tmp_dir(self) -> str:
        temp = os.path.join(self.apex_out_dir, self.filename)
        if not os.path.exists(temp):
            os.makedirs(temp, exist_ok=True)
        return temp
    
    @functools.cached_property
    def payload_dir(self) -> str:
        """Temporary directory for APEX payload extraction."""
        return os.path.join(self.apex_tmp_dir, "payload")
    
    @functools.cached_property
    def tool_paths(self) -> Dict[str, str]:
        """Return paths to required tools."""
        return {
            "fsckerofs": self.tool_paths.get("fsckerofs", "fsck.erofs"),
            "debugfs": self.tool_paths.get("debugfs", "debugfs")
        }

    def _extract_single_apex(self, apex_file: str, dst_dir: str):
        if not os.path.exists(self.payload_dir):
            os.makedirs(self.payload_dir, mode=0o755)
        with zipfile.ZipFile(apex_file) as zip_obj:
            for file_in_apex in zip_obj.namelist():
                if file_in_apex == 'apex_payload.img':
                    zip_obj.extract(file_in_apex, dst_dir)
        self._extract(os.path.join(dst_dir, 'apex_payload.img'), self.payload_dir)

    def _decompress(self):
        with zipfile.ZipFile(self.input_file, 'r') as zip_obj:
            original_apex_info = zip_obj.getinfo('original_apex')
            original_apex_info.filename = 'temp.apex'
            zip_obj.extract(original_apex_info, self.apex_tmp_dir)

    @functools.lru_cache
    def _retrieve_payload_file_system_type(self, file: str) -> str:
        """Returns filesystem type with magic"""
        with open(file, 'rb') as f:
            for fs_type, offset, magic in self.FS_TYPES:
                buf = bytearray(len(magic))
                f.seek(offset, os.SEEK_SET)
                f.readinto(buf)
                if buf == magic:
                    return fs_type
        raise ValueError('Failed to retrieve filesystem type')

    def _extract(self, apex_payload: str, extract_dir: str) -> None:
        """Extract APEX payload to specified directory."""
        payload_type = self._retrieve_payload_file_system_type(apex_payload)
        try:
            match payload_type:
                case "erofs":
                    fsckerofs_path = self.tool_paths.get("fsckerofs", "fsck.erofs")
                    subprocess.run([fsckerofs_path, f"--extract={extract_dir}", "--overwrite", apex_payload], 
                                stdout=subprocess.DEVNULL, check=True)
                case "ext4":
                    debugfs_path = self.tool_paths.get("debugfs", "debugfs")
                    subprocess.run([debugfs_path, "-R", f"rdump ./ {extract_dir}", apex_payload], 
                                capture_output=True, check=True)
                case "ext2":
                    debugfs_path = self.tool_paths.get("debugfs", "debugfs")
                    subprocess.run([debugfs_path, "-R", f"rdump ./ {extract_dir}", apex_payload], 
                                capture_output=True, check=True)
                case _:
                    raise ValueError(f"{apex_payload} is not supported for `extract`")
            self._extract_dex_files(extract_dir, self.output_dir)
        except Exception as e:
            raise RuntimeError(f"Failed to convert {apex_payload}: {str(e)}")

    def _extract_dex_files(self, source_dir: str, target_dir: str) -> None:
        """
        Extract .dex files from .jar and .apk files within the source directory.
        
        Args:
            source_dir: Directory to search for .jar and .apk files
            target_dir: Directory to extract .dex files to
        """
        for root, _, files in os.walk(source_dir):
            for file in files:
                file_path = os.path.join(root, file)
                extension = os.path.splitext(file)[-1].lstrip('.').lower()
                if extension in ['jar', 'apk']:
                    try:
                        with zipfile.ZipFile(file_path, 'r') as zf:
                            for name in zf.namelist():
                                if name.lower().endswith('.dex'):
                                    dex_name = os.path.basename(name)
                                    base_name = os.path.splitext(file)[0]
                                    target = os.path.join(target_dir, f"{self.filename}_{base_name}_{dex_name}")
                                    with zf.open(name) as src, open(target, 'wb') as dst:
                                        dst.write(src.read())
                    except zipfile.BadZipFile:
                        logger.warning(f"Skipping invalid zip file: {file_path}")
                    except Exception as e:
                        raise e

    def process(self) -> bool:
        try:
            match self.source_apex_type:
                case self.ApexType.UNCOMPRESSED:
                    self._extract_single_apex(self.input_file, self.apex_tmp_dir)
                case self.ApexType.COMPRESSED:
                    self._decompress()
                    self._extract_single_apex(os.path.join(self.apex_tmp_dir, 'temp.apex'), self.apex_tmp_dir)
                case self.ApexType.INVALID:
                    logger.error(f"{self.input_file} is not a valid APEX file")
                    return False
            self._extract_dex_files(self.payload_dir, self.output_dir)
            return True
        except Exception as e:
            logger.error(f"Failed to process APEX file: {self.input_file}, Error: {str(e)}")
            return False

    def clean(self) -> None:
        super().clean()
        if os.path.exists(self.apex_tmp_dir):
            shutil.rmtree(self.apex_tmp_dir)
        if os.path.exists(self.payload_dir):
            shutil.rmtree(self.payload_dir)


class DexProcessor(BaseProcessor):
    def __init__(self, input_file: str, output_dir: str):
        super().__init__(input_file, output_dir)
        if self.extension != 'dex':
            raise ValueError("Input file must be a DEX file")
    
    def __repr__(self) -> str:
        return f"<DexProcessor: {self.input_file}, Output: {self.output_dir}>"

    def process(self) -> bool:
        try:
            target = os.path.join(self.output_dir, self.fullname)
            shutil.copy2(self.input_file, target)
            return True
        except Exception as e:
            logger.error(f"Failed to process DEX file: {self.input_file}, Error: {str(e)}")
            return False
    
    def clean(self) -> None:
        super().clean()


class FrameworkProcessor(BaseTaskCollector):
    def __init__(self,
                 oem: str,
                 source_dir: str, 
                 out_dir: str, 
                 tool_paths: Optional[dict] = None, 
                 need_clean: bool = False):
        super().__init__()
        self.oem = oem.lower() if oem.lower() in OEM_DICT.keys() else None
        self.source_dir = source_dir
        self.work_dir = out_dir
        self.tool_paths = tool_paths or {}
        self.max_workers = MAX_PROCESS
        self.need_clean = need_clean
        
        self._validate_tool_paths()
        self._create_directories()

    @property
    def oem_dirs(self) -> List[str]:
        """Get search paths based on OEM"""
        if self.oem:
            return OEM_DICT[self.oem]
        return ["."]

    @property
    def out_tmp(self) -> str:
        return os.path.join(self.work_dir, "out_tmp")

    @property
    def jar_tmp(self) -> str:
        return os.path.join(self.work_dir, "jar_tmp")

    @property
    def apex_tmp(self) -> str:
        return os.path.join(self.work_dir, "apex_tmp")

    def _validate_tool_paths(self):
        if not os.path.isdir(self.source_dir):
            raise ValueError(f"Invalid source directory: {self.source_dir}")

        REQUIRED_TOOLS = [
            ("fsckerofs", "fsck.erofs"),
            ("debugfs", "debugfs")
        ]

        for tool_key, tool_name in REQUIRED_TOOLS:
            tool_path = self.tool_paths.get(tool_key, tool_name)
            if not os.path.exists(tool_path):
                # Try to find in PATH
                import shutil
                if not shutil.which(tool_name):
                    raise FileNotFoundError(
                        f"{tool_name} Tool not found: {tool_path} and not in PATH"
                    )

    def _create_directories(self):
        os.makedirs(self.work_dir, exist_ok=True)
        os.makedirs(self.out_tmp, exist_ok=True)
        os.makedirs(self.jar_tmp, exist_ok=True)
        os.makedirs(self.apex_tmp, exist_ok=True)

    @staticmethod
    def _find_files(
        source_dir: str,
        extensions: Set[str],
        directories: Optional[List[str]] = None
    ) -> List[str]:
        """Recursively find files with specified extensions, optionally limited to specified directories.

        Args:
            source_dir: The source directory to search in
            extensions: File extensions to match (e.g., ['.dex', '.jar'])
            directories: Directory names to limit search to. None means all subdirectories

        Returns:
            List of file paths matching the criteria
        """
        found_files = []
        ext_set = {ext.lower() for ext in extensions}
        # Build search directories
        if directories is None:
            search_dirs = [source_dir]
        else:
            search_dirs = [os.path.join(source_dir, d.lstrip("/")) for d in directories if os.path.isdir(os.path.join(source_dir, d.lstrip("/")))]
        for search_dir in search_dirs:
            if not os.path.exists(search_dir):
                continue
                
            for root, _, files in os.walk(search_dir):
                for filename in files:
                    if filename.lower().endswith(tuple(ext_set)):
                        found_files.append(os.path.join(root, filename))
        return found_files

    def collect_tasks(self, args=None) -> List[str]:
        super().collect_tasks(args)
        files = self._find_files(
            self.source_dir,
            set(FILE_TYPES),
            directories=self.oem_dirs
        )
        return files

    def single_run(self, arg):
        """
        Process a single file and return success status.
        """
        file_path: str = arg
        if not os.path.isfile(file_path):
            return False

        ext = os.path.splitext(file_path)[-1].lower()
        try:
            match ext:
                case ".jar":
                    processor = JarProcessor(
                        file_path, 
                        self.out_tmp)
                    result = TaskResult(success=True, task_id=file_path) if processor.process() else TaskResult(success=False, task_id=file_path, error="Failed to process JAR")
                    if self.need_clean:
                        processor.clean()
                    return result
                case ".apex" | ".capex":
                    processor = ApexProcessor(
                        file_path, 
                        self.out_tmp, 
                        self.tool_paths,
                        self.apex_tmp)
                    result = TaskResult(success=True, task_id=file_path) if processor.process() else TaskResult(success=False, task_id=file_path, error="Failed to process APEX")
                    if self.need_clean:
                        processor.clean()
                    return result
                case ".apk":
                    processor = ApkProcessor(
                        file_path, 
                        self.out_tmp)
                    result = TaskResult(success=True, task_id=file_path) if processor.process() else TaskResult(success=False, task_id=file_path, error="Failed to process APK")
                    if self.need_clean:
                        processor.clean()
                    return result
                case ".dex":
                    processor = DexProcessor(
                        file_path, 
                        self.out_tmp)
                    result = TaskResult(success=True, task_id=file_path) if processor.process() else TaskResult(success=False, task_id=file_path, error="Failed to process DEX")
                    if self.need_clean:
                        processor.clean()
                    return result
                case _:
                    return TaskResult(success=False, task_id=file_path, error="Unsupported file type")
        except Exception as e:
            return TaskResult(success=False, task_id=file_path, error=str(e))

    def run(self, args=None):
        results: List[TaskResult] = []
        try:
            results = super().run(args)
            
            # Add logging and printing in the child class
            success_count = sum(1 for result in results if result.success)
            error_count = len(results) - success_count
            
            logger.info(f"FrameworkProcessor summary:")
            logger.info(f"  Total tasks: {len(results)}")
            logger.info(f"  Successful: {success_count}")
            logger.info(f"  Failed: {error_count}")
            
            if error_count > 0:
                logger.warning("Failed tasks:")
                for result in results:
                    if not result.success:
                        logger.warning(f"  {result.task_id}: {result.error}")
            
            return results
        except Exception as e:
            logger.error(f"FrameworkProcessor run failed: {str(e)}")
            raise

    @classmethod
    def _remove_dirs(cls, directory: str) -> None:
        """Remove directory tree with enhanced validation and error handling.
    
        Args:
            directory: String path of the directory to remove
        """
        if not directory or not directory.strip():
            logger.warning("Empty or invalid directory path provided")
            return
        directory = os.path.abspath(directory.strip())
        if not os.path.exists(directory):
            logger.warning(f"Directory {directory} does not exist")
            return
        
        if not os.path.isdir(directory):
            logger.warning(f"{directory} is not a directory")
            return
    
        try:
            shutil.rmtree(directory)
            logger.info(f"Deleted directory: {directory}")
        except Exception as e:
            logger.error(f"Deletion failed for {directory}: {e}")
            raise

    def clean(self):
        self._remove_dirs(self.jar_tmp)
        self._remove_dirs(self.apex_tmp)


def pack_jar(out_dir: str):
    out_tmp = os.path.join(out_dir, "out_tmp")
    if not os.path.exists(out_dir):
        logger.error(f"Directory does not exist: {out_dir}")
        return False
    if not os.path.isdir(out_dir):
        logger.error(f"Path is not a directory: {out_dir}")
        return False
    
    jar_path = os.path.join(out_dir, "out.jar")
    
    try:
        with zipfile.ZipFile(jar_path, 'w', zipfile.ZIP_DEFLATED) as jar:
            # Add MANIFEST.MF file first
            manifest_content = """Manifest-Version: 1.0
Created-By: jiap
"""
            jar.writestr("META-INF/MANIFEST.MF", manifest_content)
            
            # Add all other files
            for root, _, files in os.walk(out_tmp):
                for file in files:
                    file_path = os.path.join(root, file)
                    jar.write(file_path, arcname=os.path.basename(file_path))
        logger.info(f"Successfully packed directory to JAR: {jar_path}")
        return True
    except Exception as e:
        logger.error(f"Failed to pack directory to JAR: {str(e)}")
        return False


def parse_arguments():
    parser = argparse.ArgumentParser(description="Collect Android framework files from OEM devices")
    parser.add_argument("oem",
                       help="OEM manufacturer (vivo, oppo, xiaomi, honor, google)")
    parser.add_argument("--source-dir",
                       default="./source",
                       help="Source directory for collected files")
    parser.add_argument("--out-dir",
                       default="./out",
                       help="Output directory for processed files")
    parser.add_argument("--adb-path",
                       default="adb",
                       help="Path to ADB executable")
    return parser.parse_args()


if __name__ == "__main__":
    args = parse_arguments()

    device_file = FileCollector(oem=args.oem,
                                file_types=FILE_TYPES,
                                source_dir=args.source_dir,
                                adb_path=args.adb_path)
    device_file.run()

    tool_paths = {
        "fsckerofs": "fsck.erofs",
        "debugfs": "debugfs"
    }
    frameworkProcessor = FrameworkProcessor(oem=args.oem,
                                           source_dir=args.source_dir,
                                           out_dir=args.out_dir,
                                           tool_paths=tool_paths,
                                           need_clean=False)
    frameworkProcessor.run()
    pack_jar(args.out_dir)