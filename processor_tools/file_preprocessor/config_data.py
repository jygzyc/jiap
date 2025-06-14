from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List

try:
    import tomllib as toml
except ImportError:
    toml = None

@dataclass
class Config:
    source_dir: Path
    dest_dir: Path
    tool_paths: Dict[str, Path]
    file_types: List[str] = None
    max_workers: int = 4
    need_clean: bool = False

    @classmethod
    def from_toml(cls, config_path: Path):
        """Load configuration from TOML file"""
        if not config_path.exists():
            raise FileNotFoundError(f"Config file not found: {config_path}")

        with open(config_path, "rb") as f:
            config_data = toml.load(f)

        general = config_data.get("general", {})
        tools = config_data.get("tools", {})

        return cls(
            source_dir=Path(general.get("source_dir", "Source")).resolve(),
            dest_dir=Path(general.get("dest_dir", "Output")).resolve(),
            file_types=general.get("file_types", [".jar", ".apex", ".apk"]),
            max_workers=general.get("max_workers", 4),
            need_clean=general.get("need_clean", False),
            tool_paths={
                "dex2jar": Path(tools.get("dex2jar", "d2j-dex2jar")).resolve(),
                "fsckerofs": Path(tools.get("fsckerofs", "fsck.erofs")).resolve(),
                "debugfs": Path(tools.get("debugfs", "debugfs")).resolve()
            }
        )