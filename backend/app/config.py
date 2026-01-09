"""
Configuration management for Galgame Library Manager.

Supports SANDBOX mode for safe testing with isolated data paths.

Phase 19.5: Scanner configuration with JSON persistence
"""

import json
import logging
from pathlib import Path
from typing import Dict, Any, Optional

logger = logging.getLogger(__name__)
from .core.config import settings


class Config:
    """
    Application configuration with sandbox mode support.

    Sandbox Mode:
    - Activated via GALGAME_ENV=sandbox environment variable
    - ALL paths are redirected to ./sandbox_data/...
    - Prevents accidental data corruption during testing

    Phase 19.5: Scanner configuration
    - scan_on_startup: bool - Scan library when server starts
    - scan_interval_min: int - Auto-scan interval (0 = manual mode)

    Phase 26.0: Portable Mode
    - Activated via VNITE_DATA_PATH environment variable (set by Electron)
    - All data stored in app directory instead of user home
    - Required for portable/green software distribution
    """

    # Default paths
    DEFAULT_LIBRARY_ROOT = Path.home() / "Galgames"
    DEFAULT_CONFIG_DIR = Path.home() / ".galgame-manager" / "config"

    # Sandbox paths
    SANDBOX_BASE = Path.cwd() / "sandbox_data"
    SANDBOX_LIBRARY = SANDBOX_BASE / "library"
    SANDBOX_CONFIG = SANDBOX_BASE / "config"
    SANDBOX_TRASH = SANDBOX_BASE / "trash"
    SANDBOX_JOURNAL = SANDBOX_BASE / "journal"

    # Phase 19.5: Scanner defaults
    DEFAULT_SCAN_ON_STARTUP = False
    DEFAULT_SCAN_INTERVAL_MIN = 0  # 0 = manual mode

    # Phase 24.5: Update defaults
    DEFAULT_AUTO_CHECK_ENABLED = True
    DEFAULT_CHECK_INTERVAL_HOURS = 24

    # Phase 24.5: Backup defaults
    DEFAULT_BACKUP_HOUR = 3  # 03:00 AM
    DEFAULT_BACKUP_MINUTE = 0

    def __init__(self):
        """Initialize configuration based on environment."""
        self.sandbox_mode = (settings.GALGAME_ENV or "").lower() == "sandbox"

        # Phase 26.0: Check for portable mode
        self.portable_mode = settings.VNITE_DATA_PATH is not None
        self.portable_data_path = Path(settings.VNITE_DATA_PATH or ".")

        # Phase 19.5: Initialize scanner settings
        self.scan_on_startup = self.DEFAULT_SCAN_ON_STARTUP
        self.scan_interval_min = self.DEFAULT_SCAN_INTERVAL_MIN

        # Phase 24.5: Initialize update settings
        self.auto_check_enabled = self.DEFAULT_AUTO_CHECK_ENABLED
        self.check_interval_hours = self.DEFAULT_CHECK_INTERVAL_HOURS

        # Phase 24.5: Initialize backup settings
        self.backup_hour = self.DEFAULT_BACKUP_HOUR
        self.backup_minute = self.DEFAULT_BACKUP_MINUTE

        if self.sandbox_mode:
            self._setup_sandbox()
        elif self.portable_mode:
            self._setup_portable()
        else:
            self._setup_production()

        # Phase 19.5: Load settings from JSON
        self._load_settings()

    def _setup_sandbox(self):
        """Setup sandbox mode with isolated paths."""
        # PHASE 9: Support multiple library roots
        self.library_roots = [self.SANDBOX_LIBRARY]
        self.config_dir = self.SANDBOX_CONFIG
        self.trash_dir = self.SANDBOX_TRASH
        self.journal_dir = self.SANDBOX_JOURNAL

        # Create all directories
        for path in self.library_roots + [self.config_dir, self.trash_dir, self.journal_dir]:
            path.mkdir(parents=True, exist_ok=True)

        logger.info("=" * 70)
        logger.info("SANDBOX MODE ACTIVATED")
        logger.info("=" * 70)
        logger.info("ALL operations are isolated to: ./sandbox_data/")
        logger.info("Library Roots: %s", self.library_roots)
        logger.info("Config:   %s", self.config_dir)
        logger.info("Trash:    %s", self.trash_dir)
        logger.info("Journal:  %s", self.journal_dir)
        logger.info("=" * 70)
        logger.info("THIS IS A SAFE TESTING ENVIRONMENT")
        logger.info("=" * 70)

        # Print to console as well (ASCII only to avoid CP950 errors)
        print("\n" + "=" * 70)
        print("         RUNNING IN SANDBOX MODE")
        print("=" * 70)
        print("ALL DATA IS ISOLATED TO: ./sandbox_data/")
        print("Library Roots:", self.library_roots)
        print("Config:  ", self.config_dir)
        print("Trash:   ", self.trash_dir)
        print("Journal: ", self.journal_dir)
        print("=" * 70)
        print("SAFE TESTING ENVIRONMENT - NO PRODUCTION DATA")
        print("=" * 70 + "\n")

    def _setup_portable(self):
        """Setup portable mode with app-local paths (Phase 26.0)."""
        # In portable mode, store all data in app directory
        portable_data = self.portable_data_path / "data"

        self.library_roots = [portable_data / "library"]
        self.config_dir = portable_data / "config"
        self.trash_dir = portable_data / "trash"
        self.journal_dir = portable_data / "journal"

        # Create all directories
        for path in self.library_roots + [self.config_dir, self.trash_dir, self.journal_dir]:
            path.mkdir(parents=True, exist_ok=True)

        # Legacy: Set library_root to first root for backward compatibility
        self.library_root = self.library_roots[0]

        logger.info("=" * 70)
        logger.info("PORTABLE MODE ACTIVATED")
        logger.info("=" * 70)
        logger.info(f"App Root: {self.portable_data_path.parent}")
        logger.info(f"Library Roots: {self.library_roots}")
        logger.info(f"Config:   {self.config_dir}")
        logger.info(f"Trash:    {self.trash_dir}")
        logger.info(f"Journal:  {self.journal_dir}")
        logger.info("=" * 70)
        logger.info("ALL DATA STORED IN APP DIRECTORY")
        logger.info("=" * 70)

        # Print to console as well (ASCII only to avoid CP950 errors)
        print("\n" + "=" * 70)
        print("         RUNNING IN PORTABLE MODE")
        print("=" * 70)
        print(f"App Root: {self.portable_data_path.parent}")
        print("Library Roots:", self.library_roots)
        print("Config:  ", self.config_dir)
        print("Trash:   ", self.trash_dir)
        print("Journal: ", self.journal_dir)
        print("=" * 70)
        print("ALL DATA STORED IN APP DIRECTORY")
        print("=" * 70 + "\n")

    def _setup_production(self):
        """Setup production mode with default paths."""
        # PHASE 9: Support multiple library roots via GALGAME_LIBRARY_ROOTS (JSON list) or GALGAME_LIBRARY_ROOT (single)
        library_roots_env = settings.GALGAME_LIBRARY_ROOTS
        if library_roots_env:
            # Parse JSON list of paths
            try:
                paths = json.loads(library_roots_env)
                self.library_roots = [Path(p) for p in paths]
            except (json.JSONDecodeError, TypeError):
                logger.warning(f"Invalid GALGAME_LIBRARY_ROOTS JSON, using default")
                self.library_roots = [self.DEFAULT_LIBRARY_ROOT]
        else:
            # Fallback to single path (backward compatibility)
            single_root = settings.GALGAME_LIBRARY_ROOT or str(self.DEFAULT_LIBRARY_ROOT)
            self.library_roots = [Path(single_root)]

        self.config_dir = Path(settings.GALGAME_CONFIG_DIR or self.DEFAULT_CONFIG_DIR)
        self.trash_dir = self.config_dir / "trash"
        self.journal_dir = self.config_dir / "journal"

        # Ensure directories exist
        self.config_dir.mkdir(parents=True, exist_ok=True)
        for library_root in self.library_roots:
            library_root.mkdir(parents=True, exist_ok=True)

        # Legacy: Set library_root to first root for backward compatibility
        self.library_root = self.library_roots[0]

    def get_paths(self) -> Dict[str, Any]:
        """Get all configured paths."""
        # FIX: Always use first root from library_roots list
        # This works for all modes (portable, sandbox, production)
        library_root = self.library_roots[0] if self.library_roots else None

        return {
            "library_roots": self.library_roots,
            "library_root": library_root,  # Legacy: first root
            "config_dir": self.config_dir,
            "trash_dir": self.trash_dir,
            "journal_dir": self.journal_dir,
        }

    def get_info(self) -> Dict[str, Any]:
        """Get configuration info for API responses."""
        # Determine mode string
        if self.sandbox_mode:
            mode = "sandbox"
        elif self.portable_mode:
            mode = "portable"
        else:
            mode = "production"

        return {
            "mode": mode,
            "library_roots": [str(p) for p in self.library_roots],
            "library_root": str(self.library_root),  # Legacy: first root
            "config_dir": str(self.config_dir),
            "trash_dir": str(self.trash_dir),
            "journal_dir": str(self.journal_dir),
        }

    def add_library_root(self, new_root: Path) -> None:
        """
        Add a new library root.

        Args:
            new_root: Path to new library root
        """
        new_root = Path(new_root)
        if new_root not in self.library_roots:
            self.library_roots.append(new_root)
            new_root.mkdir(parents=True, exist_ok=True)
            logger.info(f"Added library root: {new_root}")

    # Phase 19.5: Scanner settings persistence
    # Phase 24.5: Update and Backup settings persistence
    def _load_settings(self):
        """Load settings from settings.json file."""
        settings_file = self.config_dir / "settings.json"
        if settings_file.exists():
            try:
                with open(settings_file, 'r', encoding='utf-8') as f:
                    settings = json.load(f)

                # Load scanner settings
                scanner_config = settings.get('scanner', {})
                self.scan_on_startup = scanner_config.get('scan_on_startup', self.DEFAULT_SCAN_ON_STARTUP)
                self.scan_interval_min = scanner_config.get('scan_interval_min', self.DEFAULT_SCAN_INTERVAL_MIN)

                # Phase 24.5: Load update settings
                update_config = settings.get('update', {})
                self.auto_check_enabled = update_config.get('auto_check_enabled', self.DEFAULT_AUTO_CHECK_ENABLED)
                self.check_interval_hours = update_config.get('check_interval_hours', self.DEFAULT_CHECK_INTERVAL_HOURS)

                # Phase 24.5: Load backup settings
                backup_config = settings.get('backup', {})
                self.backup_hour = backup_config.get('hour', self.DEFAULT_BACKUP_HOUR)
                self.backup_minute = backup_config.get('minute', self.DEFAULT_BACKUP_MINUTE)

                logger.info(f"Loaded scanner settings: scan_on_startup={self.scan_on_startup}, scan_interval_min={self.scan_interval_min}")
                logger.info(f"Loaded update settings: auto_check_enabled={self.auto_check_enabled}, check_interval_hours={self.check_interval_hours}")
                logger.info(f"Loaded backup settings: hour={self.backup_hour}, minute={self.backup_minute}")
            except Exception as e:
                logger.warning(f"Failed to load settings from {settings_file}: {e}")
                # Use defaults
                self.scan_on_startup = self.DEFAULT_SCAN_ON_STARTUP
                self.scan_interval_min = self.DEFAULT_SCAN_INTERVAL_MIN
                self.auto_check_enabled = self.DEFAULT_AUTO_CHECK_ENABLED
                self.check_interval_hours = self.DEFAULT_CHECK_INTERVAL_HOURS
                self.backup_hour = self.DEFAULT_BACKUP_HOUR
                self.backup_minute = self.DEFAULT_BACKUP_MINUTE

    def _save_settings(self):
        """Save settings to settings.json file."""
        settings_file = self.config_dir / "settings.json"

        # Load existing settings or create new
        try:
            if settings_file.exists():
                with open(settings_file, 'r', encoding='utf-8') as f:
                    settings = json.load(f)
            else:
                settings = {}
        except Exception as e:
            logger.warning(f"Failed to load existing settings: {e}")
            settings = {}

        # Update scanner settings
        if 'scanner' not in settings:
            settings['scanner'] = {}
        settings['scanner']['scan_on_startup'] = self.scan_on_startup
        settings['scanner']['scan_interval_min'] = self.scan_interval_min

        # Phase 24.5: Update update settings
        if 'update' not in settings:
            settings['update'] = {}
        settings['update']['auto_check_enabled'] = self.auto_check_enabled
        settings['update']['check_interval_hours'] = self.check_interval_hours

        # Phase 24.5: Update backup settings
        if 'backup' not in settings:
            settings['backup'] = {}
        settings['backup']['hour'] = self.backup_hour
        settings['backup']['minute'] = self.backup_minute

        # Save to file
        try:
            with open(settings_file, 'w', encoding='utf-8') as f:
                json.dump(settings, f, indent=2, ensure_ascii=False)
            logger.info(f"Saved settings to {settings_file}")
        except Exception as e:
            logger.error(f"Failed to save settings to {settings_file}: {e}")

    def update_scanner_settings(self, scan_on_startup: Optional[bool] = None, scan_interval_min: Optional[int] = None):
        """
        Update scanner configuration.

        Phase 19.5: Persists to settings.json

        Args:
            scan_on_startup: Whether to scan on server startup
            scan_interval_min: Auto-scan interval in minutes (0 = manual)
        """
        if scan_on_startup is not None:
            self.scan_on_startup = scan_on_startup

        if scan_interval_min is not None:
            self.scan_interval_min = scan_interval_min

        # Persist to JSON
        self._save_settings()

        logger.info(f"Updated scanner settings: scan_on_startup={self.scan_on_startup}, scan_interval_min={self.scan_interval_min}")

    # Phase 24.5: Update settings methods
    def update_update_settings(self, auto_check_enabled: Optional[bool] = None, check_interval_hours: Optional[int] = None):
        """
        Update update configuration.

        Phase 24.5: Persists to settings.json

        Args:
            auto_check_enabled: Whether to automatically check for updates
            check_interval_hours: Check interval in hours
        """
        if auto_check_enabled is not None:
            self.auto_check_enabled = auto_check_enabled

        if check_interval_hours is not None:
            self.check_interval_hours = check_interval_hours

        # Persist to JSON
        self._save_settings()

        logger.info(f"Updated update settings: auto_check_enabled={self.auto_check_enabled}, check_interval_hours={self.check_interval_hours}")

    # Phase 24.5: Backup settings methods
    def update_backup_settings(self, hour: Optional[int] = None, minute: Optional[int] = None):
        """
        Update backup schedule configuration.

        Phase 24.5: Persists to settings.json

        Args:
            hour: Backup hour (0-23)
            minute: Backup minute (0-59)
        """
        if hour is not None:
            if 0 <= hour <= 23:
                self.backup_hour = hour
            else:
                logger.warning(f"Invalid backup hour: {hour}, must be 0-23")

        if minute is not None:
            if 0 <= minute <= 59:
                self.backup_minute = minute
            else:
                logger.warning(f"Invalid backup minute: {minute}, must be 0-59")

        # Persist to JSON
        self._save_settings()

        logger.info(f"Updated backup settings: hour={self.backup_hour}, minute={self.backup_minute}")

    def remove_library_root(self, root_to_remove: Path) -> bool:
        """
        Remove a library root.

        Args:
            root_to_remove: Path to remove

        Returns:
            True if removed, False if not found or would remove last root
        """
        root_to_remove = Path(root_to_remove)
        if root_to_remove in self.library_roots:
            if len(self.library_roots) <= 1:
                logger.warning("Cannot remove the last library root")
                return False
            self.library_roots.remove(root_to_remove)
            # Update legacy library_root if needed
            if self.library_root == root_to_remove:
                self.library_root = self.library_roots[0]
            logger.info(f"Removed library root: {root_to_remove}")
            return True
        return False


# Global configuration instance
_config: Config = None


def get_config() -> Config:
    """
    Get or create global configuration instance.

    Returns:
        Config singleton
    """
    global _config
    if _config is None:
        _config = Config()
    return _config


def reset_config():
    """Reset configuration (mainly for testing)."""
    global _config
    _config = None
 
