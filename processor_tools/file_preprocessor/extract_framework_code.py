import os
import enum
import shutil
import sys
import json
import logging
import tempfile
import zipfile
import subprocess
import time
from multiprocessing import Pool
from pathlib import Path
from typing import List, Tuple

from config_data import Config

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(name)s: %(message)s',
    datefmt='%H:%M:%S'
)
logger = logging.getLogger("FrameworkProcessor")

BLOCK_SIZE = 4096

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


class FrameworkProcessor:
    def __init__(self, cfg: Config):
        self.config = cfg
        self._validate_paths()

        self.dex_tmp = self.config.dest_dir / "dexTmp"
        self.jar_tmp = self.config.dest_dir / "jarTmp"
        self.apex_tmp = self.config.dest_dir / "apexTmp"
        self.class_tmp = self.config.dest_dir / "classTmp"
        self._create_directories()

    def _validate_paths(self):
        if not self.config.source_dir.is_dir():
            raise ValueError(f"Invalid source directory: {self.config.source_dir}")

        REQUIRED_TOOLS = [
            ("dex2jar", "dex2jar"),
            ("fsckerofs", "fsck.erofs"),
            ("debugfs", "debugfs")
        ]

        for tool_key, tool_name in REQUIRED_TOOLS:
            tool_path = self.config.tool_paths.get(tool_key)
            if not (tool_path and tool_path.exists()):
                raise FileNotFoundError(
                    f"{tool_name} Tool not found: {tool_path or 'Null Path'}"
                )

        self.config.dest_dir.mkdir(parents=True, exist_ok=True)

    def _create_directories(self):
        self.dex_tmp.mkdir(exist_ok=True)
        self.jar_tmp.mkdir(exist_ok=True)
        self.class_tmp.mkdir(exist_ok=True)
        self.apex_tmp.mkdir(exist_ok=True)

    def process(self):
        try:
            st = time.time()
            self.extract_archives()
            self.convert_dex_to_jar()
            self.convert_apex()
            self.extract_class_from_jar()
            self.zip_class_to_jar()
            end = time.time()
            logger.info(f"Processing complete in {int(end - st)} seconds")
            if self.config.need_clean:
                self.clean()
        except Exception as e:
            logger.error(f"Processing failed: {str(e)}")
            raise

    @staticmethod
    def _find_files(
        source_dir: Path,
        extensions: List[str],
        directories: List[str] = None
    ) -> List[Path]:
        """Recursively find files with specified extensions, optionally limited to specified directories.

        Args:
            source_dir: The source directory to search in
            extensions: File extensions to match (e.g., ['.dex', '.jar'])
            directories: Directory names to limit search to. None means all subdirectories

        Returns:
            List of file paths matching the criteria
        """
        search_roots = [source_dir] if directories is None else [
            source_dir / d for d in directories if (source_dir / d).is_dir()
        ]
        found_files = set()
        for root in search_roots:
            for ext in extensions:
                found_files.update(
                    f for f in root.rglob(f"**/*{ext}")
                    if f.is_file()
                )
        return list(found_files)

    def extract_archives(self):
        logger.info("Extracting DEX files ...")

        for f in self.config.source_dir.rglob("**/*"):
            if f.is_file():
                extension = f.suffix.lower()
                try:
                    match extension:
                        case ".jar":
                            self._extract_single_jar(".dex", f, self.dex_tmp)
                        case ".apex":
                            self._extract_single_apex(".img", f, self.apex_tmp)
                        case _:
                            continue
                except Exception as e:
                    logger.error(f"Error extracting {f}: {str(e)}")
        logger.info("DEX extraction complete")

    def _extract_single_jar(self, file_type: str, jar_file: Path, dst_path: Path):

        with zipfile.ZipFile(jar_file) as myzip:
            default_target = jar_file.parent / jar_file.stem
            
            if not dst_path and self.jar_tmp:
                dst_path = self.jar_tmp

            for file_in_jar in myzip.namelist():
                if file_in_jar.endswith(file_type):
                    target = dst_path / jar_file.stem if dst_path and file_type != ".class" \
                        else dst_path if dst_path \
                        else default_target
                    
                    target.mkdir(parents=True, exist_ok=True)
                    myzip.extract(file_in_jar, str(target))

    def _extract_single_apex(self, file_type: str, apex_file: Path, dst_path: Path):
        if ApexUtil.get_apex_type(apex_file) == ApexType.COMPRESSED:
            decompressed_apex = self.apex_tmp / apex_file.name
            ApexUtil.decompress(apex_file, decompressed_apex)
            self._extract_single_apex(file_type, decompressed_apex, dst_path)
        elif ApexUtil.get_apex_type(apex_file) == ApexType.INVALID:
            logger.error(f"{apex_file} is not a valid APEX file")
            return
        else:
            with zipfile.ZipFile(apex_file) as myzip:
                if not dst_path and self.apex_tmp:
                    dst_path = self.apex_tmp
                for file_in_apex in myzip.namelist():
                    if file_in_apex.endswith(file_type):
                        target = dst_path / apex_file.stem
                        target.mkdir(parents=True, exist_ok=True)
                        myzip.extract(file_in_apex, str(target))

    def _apex_payload_extractor(self, args: Tuple[Path, Path]):
        apex_payload, extract_dir = args
        apex_payload_type = ApexUtil.retrieve_payload_file_system_type(apex_payload)
        try:
            match apex_payload_type:
                case "erofs":
                    subprocess.run([self.config.tool_paths.get("fsckerofs", "fsckerofs"), f"--extract={str(extract_dir)}", "--overwrite", str(apex_payload)], 
                                stdout=subprocess.DEVNULL, check=True)
                case "ext4":
                    subprocess.run([self.config.tool_paths.get("debugfs", "debugfs"), "-R", f"rdump ./ {str(extract_dir)}", str(apex_payload)], 
                                capture_output=True, check=True)
                case "ext2":
                    subprocess.run([self.config.tool_paths.get("debugfs", "debugfs"), "-R", f"rdump ./ {str(extract_dir)}", str(apex_payload)], 
                                capture_output=True, check=True)
                case _:
                    raise ValueError(f"{apex_payload} is not supported for `extract`")
        except Exception as e:
            raise RuntimeError(f"Failed to convert {apex_payload}: {str(e)}")
        
    def convert_apex(self):
        logger.info("Converting APEX ...")
        payload_files: List[Path] = self._find_files(self.apex_tmp, [".img"])
        if not payload_files:
            logger.warning("No APEX payload files found")
            return
        
        tasks = []
        for filepath in payload_files:
            tasks.append((filepath, filepath.parent))

        with Pool(processes=self.config.max_workers) as pool:
            results = pool.map_async(self._apex_payload_extractor, tasks)
            results.wait()

            if not results.successful():
                logger.error("Some conversions failed. Checking individual results...")
                for i, result in enumerate(results.get(timeout=1)):
                    try:
                        result 
                    except Exception as e:
                        failed_task = tasks[i]
                        logger.error(f"Task {i + 1} failed: {str(e)} (File: {failed_task[0]})")

    def convert_dex_to_jar(self):
        logger.info("Converting DEX to JAR ...")
        dex_files: List[Path] = self._find_files(self.dex_tmp, [".dex"])
        if not dex_files:
            logger.warning("No DEX files found")
            return

        tasks = [
            (
                dex_file,
                self.jar_tmp / f"{dex_file.parent.name}_{dex_file.stem}.jar"
            )
            for dex_file in dex_files
        ]

        with Pool(processes=self.config.max_workers) as pool:
            results = pool.map_async(self._dex2jar, tasks)
            results.wait()

            if not results.successful():
                logger.error("Some conversions failed. Checking individual results...")
                for i, result in enumerate(results.get(timeout=1)):
                    try:
                        result 
                    except Exception as e:
                        failed_file = tasks[i][0]
                        logger.error(f"Task {i + 1} failed: {str(e)} (File: {failed_file})")

    def _dex2jar(self, args: Tuple[Path, Path]):
        source, target = args
        try:
            cmd = [self.config.tool_paths.get("dex2jar", "dex2jar"), str(source), "-f", "-o", str(target)]
            subprocess.run(cmd, check=True, capture_output=True, text=True)
        except Exception as e:
            raise RuntimeError(f"Failed to convert {source} to {target}: {str(e)}")

    @staticmethod
    def _process_jar(args: Tuple[Path, Path]) -> Tuple[str, List[str]]:
        jar_file, class_tmp_dir = args
        try:
            with zipfile.ZipFile(jar_file) as zip_file:
                class_list = [n for n in zip_file.namelist() if n.endswith('.class')]
                zip_file.extractall(class_tmp_dir, members=class_list)
                # for n in class_list:
                #     zip_file.extract(n, class_tmp_dir)
            logger.info(f"Processed {jar_file.name} successfully")
            return jar_file.name, class_list

        except zipfile.BadZipFile as e:
            logger.error(f"Bad ZIP file {jar_file}: {str(e)}")
            return jar_file.name, []
        except Exception as e:
            logger.error(f"Error processing {jar_file}: {str(e)}")
            return jar_file.name, []

    def extract_class_from_jar(self):
        jar_files = [
            f
            for directory in (self.jar_tmp, self.apex_tmp)
            for f in directory.iterdir()
            if f.is_file() and f.suffix == '.jar'
        ]

        if not jar_files:
            logger.warning("No JAR files found")
            return

        # Prepare processing tasks with (source_jar, output_dir) tuples
        processing_tasks = [(jar_file, self.class_tmp) for jar_file in jar_files]

        with Pool(processes=self.config.max_workers) as pool:
            class_info = dict(pool.map(self._process_jar, processing_tasks))

        output_path = self.config.dest_dir / "classInfo.json"
        with output_path.open("w", encoding="utf-8") as f:
            json.dump(class_info, f, indent=4, ensure_ascii=False)

    def zip_class_to_jar(self):
        zip_file_path = self.config.dest_dir / f"framework-all.jar"

        # Collect all .class files
        tasks = [
            (file_path, str(file_path.relative_to(self.class_tmp)))
            for file_path in self.class_tmp.rglob("*.class")
            if file_path.is_file()
        ]

        if not tasks:
            logger.warning(f"No valid .class files found in {self.class_tmp}")
            with zipfile.ZipFile(zip_file_path, "w", zipfile.ZIP_DEFLATED):
                pass # Create empty ZIP file
            logger.info(f"Created empty ZIP file: {zip_file_path}")
            return

        chunk_size = max(1, len(tasks) // self.config.max_workers)
        chunks = [tasks[i:i + chunk_size] for i in range(0, len(tasks), chunk_size)]

        temp_files: List[Path] = []
        try:
            with Pool(processes=self.config.max_workers) as pool:
                temp_files = pool.starmap(
                    ZipUtil.write_chunk_to_zip,
                    [(chunk, self.config.dest_dir) for chunk in chunks]
                )

            if any(not f.exists() for f in temp_files):
                logger.error("Some temporary ZIP files were not created successfully")
                raise RuntimeError("Failed to create some temporary ZIP files")

            # Merge temporary ZIP files
            with zipfile.ZipFile(zip_file_path, "w", compression=zipfile.ZIP_DEFLATED) as final_zip:
                for temp_zip in temp_files:
                    try:
                        with zipfile.ZipFile(temp_zip, "r") as temp:
                            for file_info in temp.infolist():
                                final_zip.writestr(file_info, temp.read(file_info.filename))
                                logger.debug(f"Merged {file_info.filename} from {temp_zip}")
                    except (zipfile.BadZipFile, FileNotFoundError, PermissionError) as e:
                        logger.error(f"Error processing {temp_zip}: {str(e)}")
                        raise

            logger.info(f"Created ZIP file: {zip_file_path} with {len(tasks)} files")

        except Exception as e:
            logger.error(f"Failed to create ZIP file {zip_file_path}: {str(e)}")
            raise
        finally:
            # Cleanup temporary files with error handling
            for temp_file in temp_files:
                try:
                    if temp_file.exists():
                        temp_file.unlink(missing_ok=True)
                        logger.debug(f"Deleted temporary file {temp_file}")
                except Exception as e:
                    logger.warning(f"Failed to delete {temp_file}: {str(e)}")

    @classmethod
    def _remove_dirs(cls, directory: Path) -> None:
        """Remove directory tree with enhanced validation and error handling.

        Args:
            directory: Path object of the directory to remove
        """
        if not directory.exists():
            logger.warning(f"Directory {directory} does not exist")
            return
        if not directory.is_dir():
            logger.warning(f"{directory} is not a directory")
            return

        try:
            shutil.rmtree(directory)
            logger.info(f"Deleted directory: {directory}")
        except Exception as e:
            logger.error(f"Deletion failed for {directory}: {e}")
            raise

    def clean(self):
        self._remove_dirs(self.dex_tmp)
        self._remove_dirs(self.jar_tmp)
        self._remove_dirs(self.apex_tmp)
        self._remove_dirs(self.class_tmp)



class ApexUtil:

    @staticmethod
    def retrieve_payload_file_system_type(file: Path) -> str:
        """Returns filesystem type with magic"""
        with open(file, 'rb') as f:
            for fs_type, offset, magic in FS_TYPES:
                buf = bytearray(len(magic))
                f.seek(offset, os.SEEK_SET)
                f.readinto(buf)
                if buf == magic:
                    return fs_type
        raise ValueError('Failed to retrieve filesystem type')
    
    @staticmethod
    def get_apex_type(apex_path: Path) -> ApexType:
        with zipfile.ZipFile(apex_path, 'r') as zip_file:
            names = zip_file.namelist()
            has_payload = 'apex_payload.img' in names
            has_original = 'original_apex' in names
            if has_payload and has_original:
                return ApexType.INVALID
            return ApexType.UNCOMPRESSED if has_payload else \
                ApexType.COMPRESSED if has_original else \
                ApexType.INVALID
        
    @staticmethod
    def decompress(compressed_apex: Path, decompressed_apex: Path):
        if decompressed_apex.exists():
            logger.warning(f"Output path {str(decompressed_apex)} already exists")
            return 
        
        try:
            decompressed_apex.parent.mkdir(parents=True, exist_ok=True)
        except PermissionError as e:
            logger.error(f"Permission denied for creating {decompressed_apex.parent}: {e}")
            return

        with zipfile.ZipFile(compressed_apex, 'r') as zip_obj:
            if 'original_apex' not in zip_obj.namelist():
                logger.warning(f"{str(compressed_apex)} is not a compressed APEX. Missing original_apex' file inside it.")
                return
            original_info = zip_obj.getinfo('original_apex')
            original_info.filename = decompressed_apex.name
            zip_obj.extract(original_info, path=decompressed_apex.parent)

class ZipUtil:

    @staticmethod
    def write_chunk_to_zip(chunk: list[tuple[Path, str]],dest_dir: Path) -> Path:
        """Write file chunk to temporary ZIP file.

        Args:
            args: Tuple containing (chunk, dest_dir) where chunk is list of (file_path, arcname)

        Returns:
            Path: Temporary ZIP file path
        """

        fd, temp_path = tempfile.mkstemp(suffix="-vivotmp.jar", dir=dest_dir)
        os.close(fd)  # Immediately close unused file descriptor
        temp_zip = Path(temp_path)

        try:
            with zipfile.ZipFile(temp_zip, "w", compression=zipfile.ZIP_DEFLATED) as zip_file:
                for file_path, arcname in chunk:
                    try:
                        zip_file.write(file_path, arcname)
                    except PermissionError as e:
                        logger.error(f"Permission denied writing {file_path} to {temp_zip}: {str(e)}")
                    except FileNotFoundError as e:
                        logger.error(f"File {file_path} not found: {str(e)}")
                    except Exception as e:
                        logger.error(f"Unexpected error writing {file_path} to {temp_zip}: {str(e)}")
            return temp_zip
        except Exception as e:  # Broad exception for safety while maintaining behavior
            logger.error(f"Error creating {temp_zip}: {str(e)}")
        return temp_zip

if __name__ == "__main__":
    try:
        config_path = Path(__file__).parent / "config.toml"
        config = Config.from_toml(config_path)

        processor = FrameworkProcessor(config)
        processor.process()
    except Exception as e:
        logger.error(f"Critical failure: {str(e)}")
        sys.exit(1)