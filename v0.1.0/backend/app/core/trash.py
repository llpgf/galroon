"""
Smart Trash Manager - Configurable and Safe Trash System

Provides flexible trash management with:
- Configurable max size (default: 50GB, 0 = unlimited)
- Configurable retention days (default: 30)
- Minimum disk free space guard (default: 5GB)
- Automatic cleanup when limits exceeded
"""

import json
import logging
import shutil
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, Optional

import psutil

logger = logging.getLogger(__name__)


class TrashConfig:
    """
    Configuration for the Smart Trash Manager.

    Attributes:
        max_size_gb: Maximum trash size in GB (0 = unlimited)
        retention_days: Days to keep trash before auto-cleanup
        min_disk_free_gb: Minimum free disk space to maintain (safety guard)
    """

    VERSION = 1  # Config version for migrations

    def __init__(
        self,
        max_size_gb: float = 50.0,
        retention_days: int = 30,
        min_disk_free_gb: float = 5.0
    ):
        """
        Initialize trash configuration.

        Args:
            max_size_gb: Maximum trash size in GB (0 = unlimited, default: 50)
            retention_days: Days to keep trash (default: 30)
            min_disk_free_gb: Min free disk space in GB (default: 5)
        """
        self.max_size_gb = max_size_gb
        self.retention_days = retention_days
        self.min_disk_free_gb = min_disk_free_gb

    def to_dict(self) -> Dict:
        """Convert to dictionary for serialization."""
        return {
            "version": self.VERSION,
            "max_size_gb": self.max_size_gb,
            "retention_days": self.retention_days,
            "min_disk_free_gb": self.min_disk_free_gb
        }

    @classmethod
    def from_dict(cls, data: Dict) -> "TrashConfig":
        """Create from dictionary (with version compatibility)."""
        version = data.get("version", 1)
        return cls(
            max_size_gb=data.get("max_size_gb", 50.0),
            retention_days=data.get("retention_days", 30),
            min_disk_free_gb=data.get("min_disk_free_gb", 5.0)
        )


class SmartTrashManager:
    """
    Manages trash with intelligent size and retention policies.

    Features:
    - Configurable max size and retention
    - Automatic cleanup when limits exceeded
    - Disk space monitoring
    - Transaction isolation
    """

    def __init__(self, config_dir: Path, config: Optional[TrashConfig] = None):
        """
        Initialize the smart trash manager.

        Args:
            config_dir: Configuration directory (contains .trash folder)
            config: Trash configuration (uses defaults if None)
        """
        self.config_dir = Path(config_dir)
        self.trash_dir = self.config_dir / ".trash"
        self.config_file = self.config_dir / "trash_config.json"

        # Load or create config
        if config is None:
            config = self._load_config()
        self.config = config

        # Ensure trash directory exists
        self.trash_dir.mkdir(parents=True, exist_ok=True)

    def _load_config(self) -> TrashConfig:
        """Load configuration from file or create default."""
        if self.config_file.exists():
            try:
                with open(self.config_file, "r") as f:
                    data = json.load(f)
                return TrashConfig.from_dict(data)
            except Exception as e:
                logger.warning(f"Failed to load trash config, using defaults: {e}")
                return TrashConfig()
        else:
            # Save default config
            default_config = TrashConfig()
            self._save_config(default_config)
            return default_config

    def _save_config(self, config: TrashConfig) -> None:
        """Save configuration to file."""
        with open(self.config_file, "w") as f:
            json.dump(config.to_dict(), f, indent=2)

    def update_config(
        self,
        max_size_gb: Optional[float] = None,
        retention_days: Optional[int] = None,
        min_disk_free_gb: Optional[float] = None
    ) -> TrashConfig:
        """
        Update trash configuration.

        Args:
            max_size_gb: New max size (None to keep current)
            retention_days: New retention days (None to keep current)
            min_disk_free_gb: New min free disk (None to keep current)

        Returns:
            Updated configuration
        """
        if max_size_gb is not None:
            self.config.max_size_gb = max_size_gb
        if retention_days is not None:
            self.config.retention_days = retention_days
        if min_disk_free_gb is not None:
            self.config.min_disk_free_gb = min_disk_free_gb

        self._save_config(self.config)

        # Run cleanup after config change
        self.ensure_headroom()

        return self.config

    def get_trash_size_gb(self) -> float:
        """
        Calculate current trash size in GB.

        Returns:
            Trash size in GB
        """
        total_bytes = 0

        if not self.trash_dir.exists():
            return 0.0

        for tx_dir in self.trash_dir.iterdir():
            if tx_dir.is_dir():
                # Calculate size of this transaction's trash
                try:
                    for item in tx_dir.rglob("*"):
                        if item.is_file():
                            total_bytes += item.stat().st_size
                except OSError:
                    pass  # Skip if can't access

        return total_bytes / (1024 ** 3)  # Convert to GB

    def get_disk_free_gb(self) -> float:
        """
        Get free disk space in GB.

        Returns:
            Free disk space in GB
        """
        try:
            stat = psutil.disk_usage(self.config_dir)
            return stat.free / (1024 ** 3)  # Convert to GB
        except Exception as e:
            logger.error(f"Failed to get disk free space: {e}")
            return float('inf')  # Assume infinite if can't check

    def ensure_headroom(self) -> int:
        """
        Ensure trash has enough headroom by deleting oldest trash.

        Cleanup triggers:
        - Trash size > max_size_gb (if max_size_gb > 0)
        - Disk free < min_disk_free_gb

        Deletes oldest transactions until safe.

        Returns:
            Number of trash items deleted
        """
        trash_size_gb = self.get_trash_size_gb()
        disk_free_gb = self.get_disk_free_gb()

        deleted_count = 0

        # Check if we need cleanup
        need_cleanup = False

        if self.config.max_size_gb > 0 and trash_size_gb > self.config.max_size_gb:
            logger.warning(
                f"Trash size ({trash_size_gb:.2f}GB) exceeds max ({self.config.max_size_gb:.2f}GB). "
                "Cleaning up oldest trash..."
            )
            need_cleanup = True

        if disk_free_gb < self.config.min_disk_free_gb:
            logger.critical(
                f"Disk free space ({disk_free_gb:.2f}GB) below minimum ({self.config.min_disk_free_gb:.2f}GB). "
                "EMERGENCY cleanup: deleting oldest trash to prevent disk full..."
            )
            need_cleanup = True

        if not need_cleanup:
            return 0

        # Get all trash transactions sorted by modification time (oldest first)
        tx_dirs = []
        for tx_dir in self.trash_dir.iterdir():
            if tx_dir.is_dir():
                try:
                    mtime = datetime.fromtimestamp(tx_dir.stat().st_mtime)
                    tx_dirs.append((mtime, tx_dir))
                except OSError:
                    pass

        # Sort by modification time (oldest first)
        tx_dirs.sort(key=lambda x: x[0])

        # Delete oldest until we have headroom
        for mtime, tx_dir in tx_dirs:
            if self._is_safe():
                break  # Safe now, stop deleting

            # Delete this transaction's trash
            try:
                shutil.rmtree(tx_dir)
                deleted_count += 1
                logger.info(f"Deleted old trash: {tx_dir.name} (from {mtime})")
            except OSError as e:
                logger.error(f"Failed to delete trash {tx_dir.name}: {e}")

        logger.info(f"Headroom cleanup: deleted {deleted_count} trash items")
        return deleted_count

    def _is_safe(self) -> bool:
        """Check if trash size and disk space are within limits."""
        trash_size_gb = self.get_trash_size_gb()
        disk_free_gb = self.get_disk_free_gb()

        # Check max size (if not unlimited)
        if self.config.max_size_gb > 0 and trash_size_gb > self.config.max_size_gb:
            return False

        # Check min disk free
        if disk_free_gb < self.config.min_disk_free_gb:
            return False

        return True

    def cleanup_by_retention(self) -> int:
        """
        Delete trash older than retention_days.

        Returns:
            Number of trash items deleted
        """
        cutoff_time = datetime.now() - timedelta(days=self.config.retention_days)
        deleted_count = 0

        for tx_dir in self.trash_dir.iterdir():
            if not tx_dir.is_dir():
                continue

            try:
                mtime = datetime.fromtimestamp(tx_dir.stat().st_mtime)

                if mtime < cutoff_time:
                    shutil.rmtree(tx_dir)
                    deleted_count += 1
                    logger.info(f"Deleted expired trash: {tx_dir.name} (from {mtime})")

            except OSError as e:
                logger.error(f"Failed to delete old trash {tx_dir.name}: {e}")

        return deleted_count

    def empty_trash(self) -> int:
        """
        Empty all trash immediately.

        Returns:
            Number of trash items deleted
        """
        deleted_count = 0

        for tx_dir in self.trash_dir.iterdir():
            if tx_dir.is_dir():
                try:
                    shutil.rmtree(tx_dir)
                    deleted_count += 1
                except OSError as e:
                    logger.error(f"Failed to delete {tx_dir.name}: {e}")

        logger.warning(f"Emptied all trash: {deleted_count} items deleted")
        return deleted_count

    def get_status(self) -> Dict:
        """
        Get current trash status.

        Returns:
            Dictionary with trash statistics
        """
        trash_items = 0
        total_size_gb = self.get_trash_size_gb()
        disk_free_gb = self.get_disk_free_gb()

        for tx_dir in self.trash_dir.iterdir():
            if tx_dir.is_dir():
                trash_items += 1

        # Calculate oldest trash item
        oldest = None
        tx_dirs = []
        for tx_dir in self.trash_dir.iterdir():
            if tx_dir.is_dir():
                try:
                    mtime = datetime.fromtimestamp(tx_dir.stat().st_mtime)
                    tx_dirs.append((mtime, tx_dir))
                except OSError:
                    pass

        if tx_dirs:
            tx_dirs.sort()
            oldest = tx_dirs[0][0]

        return {
            "trash_items": trash_items,
            "trash_size_gb": round(total_size_gb, 2),
            "max_size_gb": self.config.max_size_gb,
            "disk_free_gb": round(disk_free_gb, 2),
            "min_disk_free_gb": self.config.min_disk_free_gb,
            "retention_days": self.config.retention_days,
            "oldest_item": oldest.isoformat() if oldest else None
        }
