"""
Backup Manager Service for Galgame Library Manager.

Phase 24.5: System Governance - The Time Machine

Features:
- Create backup: Zip library.db + settings.json
- Restore backup: Unzip and overwrite
- Auto-prune: Keep last N backups (configurable)
- Backup metadata: Date, size, version
"""

import logging
import json
import zipfile
from pathlib import Path
from datetime import datetime
from typing import List, Dict, Optional, Any
from dataclasses import dataclass, asdict

from ..config import get_config

logger = logging.getLogger(__name__)


@dataclass
class BackupMetadata:
    """Metadata for a backup."""
    filename: str
    created_at: str  # ISO format timestamp
    size_bytes: int
    size_mb: float
    version: str = "1.0"

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        return asdict(self)


class BackupManager:
    """
    Backup manager with zip-based compression.

    Phase 24.5: Enterprise-grade backup/restore system.
    """

    DEFAULT_MAX_BACKUPS = 10
    BACKUP_DIR_NAME = "backups"
    METADATA_FILE = "metadata.json"

    def __init__(self):
        """Initialize backup manager."""
        self.config = get_config()
        self.backup_dir = self.config.config_dir / self.BACKUP_DIR_NAME
        self.backup_dir.mkdir(parents=True, exist_ok=True)

        # Load max_backups from config
        self.max_backups = self._load_max_backups()

    def _load_max_backups(self) -> int:
        """Load max_backups setting from config file."""
        config_file = self.config.config_dir / "settings.json"

        if config_file.exists():
            try:
                with open(config_file, 'r', encoding='utf-8') as f:
                    config = json.load(f)
                    return config.get('backup', {}).get('max_backups', self.DEFAULT_MAX_BACKUPS)
            except Exception as e:
                logger.warning(f"Failed to load max_backups config: {e}")

        return self.DEFAULT_MAX_BACKUPS

    def _save_max_backups(self, max_backups: int):
        """Save max_backups setting to config file."""
        config_file = self.config.config_dir / "settings.json"

        try:
            # Load existing config
            if config_file.exists():
                with open(config_file, 'r', encoding='utf-8') as f:
                    config = json.load(f)
            else:
                config = {}

            # Update backup settings
            if 'backup' not in config:
                config['backup'] = {}
            config['backup']['max_backups'] = max_backups

            # Save config
            with open(config_file, 'w', encoding='utf-8') as f:
                json.dump(config, f, indent=2, ensure_ascii=False)

            self.max_backups = max_backups
            logger.info(f"Max backups set to {max_backups}")
        except Exception as e:
            logger.error(f"Failed to save max_backups config: {e}")

    def create_backup(self) -> BackupMetadata:
        """
        Create a backup of library.db and settings.json.

        Returns:
            BackupMetadata with backup details

        Raises:
            Exception: If backup creation fails
        """
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"backup_{timestamp}.zip"
        backup_path = self.backup_dir / filename

        try:
            with zipfile.ZipFile(backup_path, 'w', zipfile.ZIP_DEFLATED) as zipf:
                # Backup library.db
                db_path = self.config.config_dir / "library.db"

                if db_path.exists():
                    zipf.write(db_path, "library.db")
                    logger.info(f"Added library.db to backup")
                else:
                    logger.warning(f"library.db not found at {db_path}")

                # Backup settings.json
                settings_path = self.config.config_dir / "settings.json"

                if settings_path.exists():
                    zipf.write(settings_path, "settings.json")
                    logger.info(f"Added settings.json to backup")
                else:
                    logger.warning(f"settings.json not found at {settings_path}")

                # Add metadata to zip
                metadata = {
                    "created_at": datetime.now().isoformat(),
                    "version": "1.0",
                    "files": ["library.db", "settings.json"]
                }

                zipf.writestr("metadata.json", json.dumps(metadata, indent=2))

            # Get file size
            size_bytes = backup_path.stat().st_size
            size_mb = round(size_bytes / (1024 * 1024), 2)

            backup_meta = BackupMetadata(
                filename=filename,
                created_at=datetime.now().isoformat(),
                size_bytes=size_bytes,
                size_mb=size_mb,
                version="1.0"
            )

            logger.info(f"Backup created: {filename} ({size_mb} MB)")

            # Auto-prune old backups
            self._auto_prune()

            return backup_meta

        except Exception as e:
            logger.error(f"Failed to create backup: {e}")
            # Clean up failed backup
            if backup_path.exists():
                backup_path.unlink()
            raise

    def restore_backup(self, filename: str) -> bool:
        """
        Restore a backup from zip file.

        Args:
            filename: Name of backup file to restore

        Returns:
            True if restore successful

        Raises:
            Exception: If restore fails
        """
        backup_path = self.backup_dir / filename

        if not backup_path.exists():
            raise FileNotFoundError(f"Backup not found: {filename}")

        try:
            with zipfile.ZipFile(backup_path, 'r') as zipf:
                # List contents
                logger.info(f"Restoring from backup: {filename}")

                # Extract library.db
                if "library.db" in zipf.namelist():
                    zipf.extract("library.db", self.config.config_dir)
                    logger.info("Restored library.db")

                # Extract settings.json
                if "settings.json" in zipf.namelist():
                    zipf.extract("settings.json", self.config.config_dir)
                    logger.info("Restored settings.json")

            logger.info(f"Backup restored successfully: {filename}")
            return True

        except Exception as e:
            logger.error(f"Failed to restore backup: {e}")
            raise

    def delete_backup(self, filename: str) -> bool:
        """
        Delete a backup file.

        Args:
            filename: Name of backup file to delete

        Returns:
            True if deletion successful
        """
        backup_path = self.backup_dir / filename

        if not backup_path.exists():
            logger.warning(f"Backup not found: {filename}")
            return False

        try:
            backup_path.unlink()
            logger.info(f"Backup deleted: {filename}")
            return True
        except Exception as e:
            logger.error(f"Failed to delete backup: {e}")
            raise

    def list_backups(self) -> List[BackupMetadata]:
        """
        List all backups.

        Returns:
            List of BackupMetadata, sorted by creation time (newest first)
        """
        backups = []

        for file_path in self.backup_dir.glob("backup_*.zip"):
            try:
                # Read metadata from zip
                with zipfile.ZipFile(file_path, 'r') as zipf:
                    if "metadata.json" in zipf.namelist():
                        metadata_json = zipf.read("metadata.json").decode('utf-8')
                        metadata = json.loads(metadata_json)

                        backup_meta = BackupMetadata(
                            filename=file_path.name,
                            created_at=metadata.get("created_at", file_path.stat().st_ctime),
                            size_bytes=file_path.stat().st_size,
                            size_mb=round(file_path.stat().st_size / (1024 * 1024), 2),
                            version=metadata.get("version", "1.0")
                        )
                    else:
                        # Fallback: create metadata from file
                        backup_meta = BackupMetadata(
                            filename=file_path.name,
                            created_at=datetime.fromtimestamp(file_path.stat().st_ctime).isoformat(),
                            size_bytes=file_path.stat().st_size,
                            size_mb=round(file_path.stat().st_size / (1024 * 1024), 2),
                            version="1.0"
                        )

                backups.append(backup_meta)
            except Exception as e:
                logger.warning(f"Failed to read backup metadata for {file_path.name}: {e}")

        # Sort by creation time (newest first)
        backups.sort(key=lambda b: b.created_at, reverse=True)

        return backups

    def _auto_prune(self):
        """
        Auto-prune old backups, keeping only the most recent N backups.

        Called automatically after creating a new backup.
        """
        backups = self.list_backups()

        if len(backups) > self.max_backups:
            # Delete oldest backups
            to_delete = backups[self.max_backups:]

            for backup in to_delete:
                try:
                    self.delete_backup(backup.filename)
                    logger.info(f"Auto-pruned old backup: {backup.filename}")
                except Exception as e:
                    logger.error(f"Failed to prune backup {backup.filename}: {e}")

    def set_max_backups(self, max_backups: int):
        """
        Set maximum number of backups to keep.

        Args:
            max_backups: Maximum number of backups (1-100)
        """
        if not (1 <= max_backups <= 100):
            raise ValueError("max_backups must be between 1 and 100")

        self._save_max_backups(max_backups)

        # Prune if needed
        self._auto_prune()

    def get_backup_stats(self) -> Dict[str, Any]:
        """
        Get backup statistics.

        Returns:
            Dict with backup stats
        """
        backups = self.list_backups()

        total_size = sum(b.size_bytes for b in backups)

        return {
            "total_backups": len(backups),
            "total_size_bytes": total_size,
            "total_size_mb": round(total_size / (1024 * 1024), 2),
            "max_backups": self.max_backups,
            "oldest_backup": backups[-1].created_at if backups else None,
            "newest_backup": backups[0].created_at if backups else None
        }


# Global backup manager instance
_backup_manager: BackupManager = None


def get_backup_manager() -> BackupManager:
    """
    Get or create global backup manager instance.

    Returns:
        BackupManager singleton
    """
    global _backup_manager
    if _backup_manager is None:
        _backup_manager = BackupManager()
    return _backup_manager
