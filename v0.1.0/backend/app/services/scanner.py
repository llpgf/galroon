"""
Background Scanner Service for Galgame Library Manager.

Phase 20.0: The "Silent" Scanner - Fast diff with minimal I/O.

Algorithm:
1. Scan filesystem: Get all folder paths + mtime (OS-level, very fast)
2. Query database: Get all folder paths + json_mtime
3. Set operations: Calculate Added, Removed, Modified sets in memory
4. Batch update: Only read JSONs for the Modified set

Performance:
- Initial scan: O(N) where N = number of games
- Subsequent scans: O(M) where M = modified games only
- Zero-latency for user: Runs in background thread
"""

import logging
import threading
import time
from pathlib import Path
from typing import Set, Dict, List, Optional, Tuple
from datetime import datetime

from ..core.database import get_database
from ..metadata import get_resource_manager
from ..config import get_config

logger = logging.getLogger(__name__)


class LibraryScanner:
    """
    Background scanner with fast diff algorithm.

    Thread-safe: Can be run in background while UI reads from DB.

    Phase 24.5: Enhanced with visual progress tracking and controls.
    """

    def __init__(self):
        """Initialize scanner."""
        self.db = get_database()
        self.config = get_config()
        self.resource_manager = get_resource_manager(self.config.library_root)
        self._is_scanning = False
        self._scan_lock = threading.Lock()
        self._stop_flag = threading.Event()
        self._pause_flag = threading.Event()

        # Phase 24.5: Progress state
        self._progress = {
            "stage": "idle",  # idle, diffing, processing
            "current_file": "",
            "processed_count": 0,
            "total_changes": 0,
            "is_paused": False,
            "added_count": 0,
            "modified_count": 0,
            "removed_count": 0
        }

    def scan_library(self, background: bool = True) -> Dict[str, int]:
        """
        Scan library and sync with database.

        Args:
            background: If True, run in background thread. If False, block until complete.

        Returns:
            Dict with scan statistics: {added, modified, removed, total_time_ms}
        """
        if self._is_scanning:
            logger.warning("Scan already in progress, skipping")
            return {"status": "already_scanning"}

        if background:
            # Run in background thread
            thread = threading.Thread(target=self._scan_thread, daemon=True)
            thread.start()
            return {"status": "scan_started"}
        else:
            # Block until complete
            return self._scan_internal()

    def _scan_thread(self):
        """Background thread for scanning."""
        try:
            logger.info("Background scan started")
            result = self._scan_internal()
            logger.info(f"Background scan complete: {result}")
        except Exception as e:
            logger.error(f"Background scan failed: {e}", exc_info=True)

    def _scan_internal(self) -> Dict[str, int]:
        """
        Internal scan implementation.

        Returns:
            Dict with scan statistics
        """
        start_time = time.time()

        with self._scan_lock:
            self._is_scanning = True
            self._stop_flag.clear()
            self._pause_flag.clear()

            # Reset progress state
            self._progress = {
                "stage": "idle",
                "current_file": "",
                "processed_count": 0,
                "total_changes": 0,
                "is_paused": False,
                "added_count": 0,
                "modified_count": 0,
                "removed_count": 0
            }

        try:
            # Step 1: Scan filesystem (OS-level directory scan)
            self._progress["stage"] = "scanning"
            filesystem_state = self._scan_filesystem()

            # Step 2: Get database state
            db_folder_mtimes = self.db.get_all_folder_mtimes()
            db_json_mtimes = self.db.get_all_json_mtimes()

            # Step 3: Calculate diffs using set operations
            self._progress["stage"] = "diffing"
            added, removed, modified = self._calculate_diffs(
                filesystem_state, db_folder_mtimes, db_json_mtimes
            )

            # Set total changes for progress tracking
            total_changes = len(added) + len(modified) + len(removed)
            self._progress["total_changes"] = total_changes

            # Step 4: Apply changes to database
            self._progress["stage"] = "processing"

            added_count = self._process_added(added)
            modified_count = self._process_modified(modified)
            removed_count = self._process_removed(removed)

            # Update progress counters
            self._progress["added_count"] = added_count
            self._progress["modified_count"] = modified_count
            self._progress["removed_count"] = removed_count

            total_time_ms = int((time.time() - start_time) * 1000)

            # Mark scan as complete
            self._progress["stage"] = "idle"

            stats = {
                "added": added_count,
                "modified": modified_count,
                "removed": removed_count,
                "total_time_ms": total_time_ms,
                "status": "complete"
            }

            logger.info(
                f"Scan complete: {added_count} added, {modified_count} modified, "
                f"{removed_count} removed, {total_time_ms}ms"
            )

            return stats

        finally:
            with self._scan_lock:
                self._is_scanning = False
                # Keep stage as idle when done
                self._progress["stage"] = "idle"

    def _scan_filesystem(self) -> Dict[str, float]:
        """
        Scan filesystem and get folder paths + mtime.

        Returns:
            Dict mapping folder_path -> folder_mtime
        """
        filesystem_state = {}

        for root in self.config.library_roots:
            if not root.exists():
                continue

            # Scan directory structure only (very fast, no JSON reads)
            for game_folder in root.iterdir():
                if game_folder.is_dir() and not game_folder.name.startswith('.'):
                    try:
                        # Get folder modification time (OS-level metadata)
                        folder_mtime = game_folder.stat().st_mtime
                        filesystem_state[str(game_folder)] = folder_mtime
                    except Exception as e:
                        logger.warning(f"Failed to stat {game_folder}: {e}")
                        continue

        return filesystem_state

    def _calculate_diffs(
        self,
        filesystem_state: Dict[str, float],
        db_folder_mtimes: Dict[str, float],
        db_json_mtimes: Dict[str, float]
    ) -> Tuple[Set[str], Set[str], Set[str]]:
        """
        Calculate added, removed, and modified sets using set operations.

        Args:
            filesystem_state: Current filesystem state (folder_path -> mtime)
            db_folder_mtimes: Database folder mtimes (folder_path -> mtime)
            db_json_mtimes: Database JSON mtimes (folder_path -> mtime)

        Returns:
            Tuple of (added_set, removed_set, modified_set)
        """
        # Convert to sets for fast operations
        filesystem_paths = set(filesystem_state.keys())
        db_paths = set(db_folder_mtimes.keys())

        # Added: In filesystem but not in database
        added = filesystem_paths - db_paths

        # Removed: In database but not in filesystem (auto-prune)
        removed = db_paths - filesystem_paths

        # Modified: Check both folder mtime AND JSON mtime
        modified = set()
        for path in filesystem_paths & db_paths:
            # Check if folder was modified
            folder_changed = filesystem_state[path] != db_folder_mtimes[path]

            # Check if metadata.json was modified
            json_path = Path(path) / 'metadata.json'
            if json_path.exists():
                json_mtime = json_path.stat().st_mtime
                db_json_mtime = db_json_mtimes.get(path, 0)
                json_changed = json_mtime != db_json_mtime
            else:
                # JSON doesn't exist, need to create
                json_changed = True

            if folder_changed or json_changed:
                modified.add(path)

        logger.info(
            f"Diff calculation: {len(added)} added, {len(removed)} removed, "
            f"{len(modified)} modified"
        )

        return added, removed, modified

    def _process_added(self, added: Set[str]) -> int:
        """
        Process added games (insert into database).

        Args:
            added: Set of folder paths that are new

        Returns:
            Number of games added
        """
        count = 0

        for folder_path in added:
            # Check for stop flag
            if self._stop_flag.is_set():
                logger.info("Scan stopped by user")
                break

            # Check for pause flag - wait if paused
            while self._pause_flag.is_set():
                time.sleep(0.1)

            path = Path(folder_path)

            # Update progress
            self._progress["current_file"] = path.name
            self._progress["processed_count"] = count

            # Check if metadata.json exists
            json_path = path / 'metadata.json'
            if not json_path.exists():
                # Create minimal metadata
                metadata = self._create_minimal_metadata(path)
            else:
                # Load existing metadata
                metadata = self.resource_manager.load_metadata(path)
                if not metadata:
                    metadata = self._create_minimal_metadata(path)

            # Get mtimes
            folder_mtime = path.stat().st_mtime
            json_mtime = json_path.stat().st_mtime if json_path.exists() else folder_mtime

            # Insert into database
            try:
                self.db.upsert_game(metadata, path, folder_mtime, json_mtime)
                count += 1
                logger.debug(f"Added: {path.name}")
            except Exception as e:
                logger.error(f"Failed to add {path}: {e}")

        return count

    def _process_modified(self, modified: Set[str]) -> int:
        """
        Process modified games (update in database).

        Args:
            modified: Set of folder paths that were modified

        Returns:
            Number of games updated
        """
        count = 0

        for folder_path in modified:
            # Check for stop flag
            if self._stop_flag.is_set():
                logger.info("Scan stopped by user")
                break

            # Check for pause flag - wait if paused
            while self._pause_flag.is_set():
                time.sleep(0.1)

            path = Path(folder_path)

            # Update progress
            self._progress["current_file"] = path.name
            self._progress["processed_count"] = count

            # Check if metadata.json exists
            json_path = path / 'metadata.json'
            if not json_path.exists():
                # Create minimal metadata
                metadata = self._create_minimal_metadata(path)
            else:
                # Load existing metadata
                metadata = self.resource_manager.load_metadata(path)
                if not metadata:
                    # Fallback to minimal metadata if load fails
                    logger.warning(f"Failed to load metadata for {path}, creating minimal metadata")
                    metadata = self._create_minimal_metadata(path)

            # Get mtimes
            folder_mtime = path.stat().st_mtime
            json_mtime = json_path.stat().st_mtime if json_path.exists() else folder_mtime

            # Update database
            try:
                self.db.upsert_game(metadata, path, folder_mtime, json_mtime)
                count += 1
                logger.debug(f"Updated: {path.name}")
            except Exception as e:
                logger.error(f"Failed to update {path}: {e}")

        return count

    def _process_removed(self, removed: Set[str]) -> int:
        """
        Process removed games (delete from database).

        Auto-prune: Remove games that no longer exist in filesystem.

        Args:
            removed: Set of folder paths that were deleted

        Returns:
            Number of games removed
        """
        count = 0

        for folder_path in removed:
            # Check for stop flag
            if self._stop_flag.is_set():
                logger.info("Scan stopped by user")
                break

            # Check for pause flag - wait if paused
            while self._pause_flag.is_set():
                time.sleep(0.1)

            # Update progress
            self._progress["current_file"] = folder_path
            self._progress["processed_count"] = count

            try:
                self.db.delete_game(folder_path)
                count += 1
                logger.debug(f"Removed: {folder_path}")
            except Exception as e:
                logger.error(f"Failed to remove {folder_path}: {e}")

        return count

    def _create_minimal_metadata(self, folder_path: Path) -> Dict:
        """
        Create minimal metadata for games without metadata.json.

        Args:
            folder_path: Path to game folder

        Returns:
            Minimal metadata dict
        """
        return {
            "title": {
                "value": folder_path.name,
                "source": "filesystem",
                "locked": False
            },
            "developer": {
                "value": "Unknown Developer",
                "source": "filesystem",
                "locked": False
            },
            "library_status": {
                "value": "unstarted",
                "locked": False
            },
            "metadata_version": "2.1",
            "last_sync": datetime.now().isoformat()
        }

    def get_progress(self) -> Dict:
        """
        Get current scan progress.

        Returns:
            Dict with progress state
        """
        return self._progress.copy()

    def pause_scan(self):
        """Pause the current scan (if running)."""
        self._pause_flag.set()
        self._progress["is_paused"] = True
        logger.info("Scan paused")

    def resume_scan(self):
        """Resume the paused scan."""
        self._pause_flag.clear()
        self._progress["is_paused"] = False
        logger.info("Scan resumed")

    def stop_scan(self):
        """Stop the current scan (if running)."""
        self._stop_flag.set()
        self._pause_flag.clear()  # Clear pause flag too
        logger.info("Stop signal sent to scanner")

    def is_scanning(self) -> bool:
        """Check if scan is currently running."""
        return self._is_scanning


# Global scanner instance
_scanner: LibraryScanner = None


def get_scanner() -> LibraryScanner:
    """
    Get or create global scanner instance.

    Returns:
        LibraryScanner singleton
    """
    global _scanner
    if _scanner is None:
        _scanner = LibraryScanner()
    return _scanner
