"""
Batch Metadata Manager for async library scanning.

Provides visibility, control, and predictability for large-scale metadata operations.
Integrates with VNDB provider, resource manager, and merger for full metadata pipeline.
"""

import threading
import time
import logging
from pathlib import Path
from typing import Optional, List, Dict, Any
from enum import Enum
from datetime import datetime
import json

from .models import UnifiedMetadata, create_empty_metadata
from .providers import get_vndb_provider, get_bangumi_provider, get_erogamescape_provider, get_steam_provider
from .manager import get_resource_manager
from .merger import MetadataMerger, merge_metadata
from .inventory import AssetDetector, AssetDetectionResult

logger = logging.getLogger(__name__)


class BatchStatus(str, Enum):
    """Status of batch job."""
    IDLE = "idle"
    RUNNING = "running"
    PAUSED = "paused"
    STOPPING = "stopping"
    COMPLETED = "completed"
    ERROR = "error"


class BatchManager:
    """
    Singleton batch manager for metadata operations.

    Provides:
    - Async processing in background thread
    - Pause/stop control
    - Dry run simulation
    - Rate limiting for API requests (via VNDB provider)
    - Real-time progress tracking
    - User-friendly logging
    - Integration with VNDB, ResourceManager, and MetadataMerger
    """

    _instance: Optional['BatchManager'] = None
    _lock = threading.Lock()

    def __new__(cls):
        if cls._instance is None:
            with cls._lock:
                if cls._instance is None:
                    cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(self):
        # Only initialize once
        if hasattr(self, '_initialized'):
            return

        self._initialized = True

        # State management
        self._status = BatchStatus.IDLE
        self._pause_event = threading.Event()
        self._pause_event.set()  # Start unpaused
        self._stop_event = threading.Event()
        self._worker_thread: Optional[threading.Thread] = None

        # Progress tracking
        self._total_items: int = 0
        self._processed_items: int = 0
        self._current_file: str = ""
        self._start_time: Optional[float] = None
        self._logs: List[Dict[str, str]] = []
        self._max_logs = 100  # Keep last 100 logs

        # Configuration
        self._library_root: Optional[Path] = None
        self._chunk_size = 5  # Process 5 items at a time
        self._rate_limit_delay = 1.0  # 1 second between API requests
        self._dry_run = True
        self._prefer_traditional = True
        self._download_screenshots = True

        # Results
        self._results: Dict[str, Any] = {
            "matched": 0,
            "skipped": 0,
            "downloaded": 0,
            "failed": 0,
            "total_downloaded_bytes": 0,
        }

        # Providers and managers
        self._provider = "vndb"  # Default provider
        self._vndb_provider = None
        self._bangumi_provider = None
        self._erogamescape_provider = None
        self._steam_provider = None
        self._resource_manager = None

        # PHASE 9: Asset detection
        self._asset_detector = AssetDetector()

        # PHASE 12: Auto-enrichment after scan
        self._auto_enrich = False  # Disabled by default

    def configure(
        self,
        library_root,  # Accept both Path and List[Path] for PHASE 9
        rate_limit_delay: float = 1.0,
        quota_gb: float = 2.0,
        provider: str = "vndb"
    ):
        """
        Configure the batch manager.

        **PHASE 9 UPDATE:**
        - Accepts multiple library roots

        Args:
            library_root: Root directory(ies) of library (Path or List[Path])
            rate_limit_delay: Seconds to wait between API requests (default 1.0)
            quota_gb: Maximum metadata storage in GB
            provider: Metadata provider ("vndb" or "bangumi", default "vndb")
        """
        # PHASE 9: Support multiple roots
        if isinstance(library_root, list):
            self._library_roots = library_root
            self._library_root = library_root[0]  # Legacy: first root
        else:
            self._library_root = library_root
            self._library_roots = [library_root]

        self._rate_limit_delay = rate_limit_delay
        self._provider = provider

        # Initialize providers
        if provider == "bangumi":
            self._bangumi_provider = get_bangumi_provider()
        elif provider == "erogamescape":
            self._erogamescape_provider = get_erogamescape_provider()
        elif provider == "steam":
            self._steam_provider = get_steam_provider()
        else:
            self._vndb_provider = get_vndb_provider(rate_limit=rate_limit_delay)

        self._resource_manager = get_resource_manager(library_root, quota_gb)

        # Calculate initial usage
        self._resource_manager.calculate_usage()

        logger.info(f"BatchManager configured: library_root={library_root}, rate_limit={rate_limit_delay}s, quota={quota_gb}GB, provider={provider}")

    def _add_log(self, level: str, message: str, item: str = ""):
        """
        Add a log entry.

        Args:
            level: Log level (info, warning, error, success)
            message: Log message
            item: Related item (optional)
        """
        log_entry = {
            "timestamp": datetime.now().isoformat(),
            "level": level,
            "message": message,
            "item": item,
        }

        self._logs.append(log_entry)

        # Keep only last N logs
        if len(self._logs) > self._max_logs:
            self._logs.pop(0)

        # Also log to Python logger
        if level == "error":
            logger.error(f"{message} [{item}]")
        elif level == "warning":
            logger.warning(f"{message} [{item}]")
        else:
            logger.info(f"{message} [{item}]")

    def _reset_state(self):
        """Reset state for new job."""
        self._processed_items = 0
        self._current_file = ""
        self._start_time = None
        self._logs.clear()
        self._results = {
            "matched": 0,
            "skipped": 0,
            "downloaded": 0,
            "failed": 0,
            "total_downloaded_bytes": 0,
        }

    def _discover_targets(self) -> List[Path]:
        """
        Discover game directories for metadata scanning.

        **PHASE 9 UPDATE:**
        - Scans all configured library roots
        - Supports multi-library setups (SSD + NAS)

        Returns:
            List of game directory paths
        """
        if not self._library_root:
            self._add_log("error", "Library root not configured")
            return []

        targets = []

        try:
            # PHASE 9: Get all library roots from config if available
            # For backward compatibility, use _library_root if library_roots is not set
            library_roots = getattr(self, '_library_roots', [self._library_root])

            for library_root in library_roots:
                self._add_log("info", f"Scanning library root: {library_root}")

                # Find all directories containing game files
                for item in library_root.rglob("*"):
                    if item.is_dir():
                        # Check if this looks like a game directory
                        # (has .exe files or metadata.json already)
                        exe_files = list(item.glob("*.exe"))
                        has_metadata = (item / "metadata.json").exists()

                        if exe_files or has_metadata:
                            targets.append(item)

            self._add_log("info", f"Discovered {len(targets)} game directories across {len(library_roots)} roots")
            return targets

        except Exception as e:
            self._add_log("error", f"Failed to discover targets: {str(e)}")
            return []

    def _process_item(self, game_dir: Path) -> bool:
        """
        Process a single game directory.

        Args:
            game_dir: Game directory path

        Returns:
            True if successful, False otherwise
        """
        game_name = game_dir.name
        self._current_file = str(game_dir.relative_to(self._library_root))

        # Check for pause
        while self._status == BatchStatus.PAUSED:
            time.sleep(0.1)

        # Check for stop
        if self._status == BatchStatus.STOPPING:
            return False

        try:
            # Load existing metadata if available
            existing_metadata_dict = self._resource_manager.load_metadata(game_dir)
            existing_metadata = None

            if existing_metadata_dict:
                try:
                    existing_metadata = UnifiedMetadata(**existing_metadata_dict)
                    self._add_log("info", "Loaded existing metadata", game_name)
                except Exception as e:
                    self._add_log("warning", f"Invalid existing metadata: {e}", game_name)
                    existing_metadata = create_empty_metadata()

            if not existing_metadata:
                existing_metadata = create_empty_metadata()

            # ========== PHASE 9: Asset Inventory Detection ==========
            # Detect assets in this game directory
            self._add_log("info", "Detecting assets...", game_name)
            detection_result = self._asset_detector.detect_directory(game_dir)

            # Update assets_detected field
            existing_metadata.assets_detected = detection_result.assets

            # Add this as a version (PHASE 9: Work-Centric model)
            game_path_str = str(game_dir)
            existing_metadata.add_version(
                path=game_path_str,
                label=detection_result.version_label,
                is_primary=(not existing_metadata.versions),  # First version is primary
                assets=detection_result.assets
            )

            # Log detection results
            if detection_result.assets:
                self._add_log("info", f"Assets: {detection_result.version_label}", game_name)
            else:
                self._add_log("info", "No specific assets detected", game_name)

            # ========== END PHASE 9 ==========

            # PHASE 10: Skip if critical fields are locked
            # Check if key fields are in locked_fields list
            critical_fields = ['title', 'description', 'developer']
            locked_critical = [f for f in critical_fields if existing_metadata.is_field_locked(f)]

            if locked_critical:
                self._add_log("info", f"Skipped (Critical fields locked: {', '.join(locked_critical)})", game_name)
                self._results["skipped"] += 1
                return True

            # Legacy: Also check MetadataField.locked (for backward compatibility)
            if existing_metadata.title.locked and existing_metadata.description.locked:
                self._add_log("info", "Skipped (Metadata locked)", game_name)
                self._results["skipped"] += 1
                return True

            # Fetch from VNDB
            self._add_log("info", f"Fetching from VNDB: {game_name}", game_name)

            try:
                new_metadata = self._vndb_provider.fetch_and_parse(
                    game_name,
                    prefer_traditional=self._prefer_traditional
                )

                if not new_metadata:
                    self._add_log("warning", "No match found on VNDB", game_name)
                    self._results["failed"] += 1
                    return False

                # Get fuzzy match score if available
                fuzzy_score = getattr(new_metadata, '_fuzzy_score', None)
                if fuzzy_score:
                    self._add_log("info", f"Matched: {new_metadata.get_preferred_title()} ({fuzzy_score}%)", game_name)
                else:
                    self._add_log("info", f"Matched: {new_metadata.get_preferred_title()}", game_name)

                self._results["matched"] += 1

            except Exception as e:
                self._add_log("error", f"Failed to fetch from VNDB: {str(e)}", game_name)
                self._results["failed"] += 1
                return False

            # Merge metadata with field-level locking
            try:
                merged_metadata, changes = merge_metadata(
                    existing_metadata,
                    new_metadata.model_dump(),
                    "vndb",
                    self._prefer_traditional
                )

                if changes:
                    self._add_log("success", f"Updated metadata ({len(changes)} fields)", game_name)
                else:
                    self._add_log("info", "No changes needed", game_name)

            except Exception as e:
                self._add_log("error", f"Failed to merge metadata: {str(e)}", game_name)
                self._results["failed"] += 1
                return False

            # Dry run - don't actually save or download
            if self._dry_run:
                # Estimate download size
                estimated_size = 2.5 * 1024 * 1024  # 2.5 MB
                self._results["total_downloaded_bytes"] += estimated_size
                self._add_log("info", f"Dry run - Would save metadata ({estimated_size / (1024*1024):.1f}MB)", game_name)
                return True

            # Real execution - save metadata
            try:
                # Save metadata.json
                metadata_dict = merged_metadata.model_dump()
                success = self._resource_manager.save_metadata(metadata_dict, game_dir)

                if not success:
                    self._add_log("error", "Failed to save metadata", game_name)
                    self._results["failed"] += 1
                    return False

                # Download cover image
                if merged_metadata.cover_url.value:
                    cover_path = self._resource_manager.download_metadata_image(
                        metadata_dict,
                        game_dir,
                        "cover"
                    )
                    if cover_path:
                        self._add_log("success", "Downloaded cover image", game_name)

                # Download screenshots if requested
                if self._download_screenshots and merged_metadata.screenshot_urls.value:
                    screenshot_path = self._resource_manager.download_metadata_image(
                        metadata_dict,
                        game_dir,
                        "screenshots"
                    )
                    if screenshot_path:
                        self._add_log("success", "Downloaded screenshot", game_name)

                self._add_log("success", "Metadata saved successfully", game_name)
                self._results["downloaded"] += 1

            except Exception as e:
                self._add_log("error", f"Failed to save metadata: {str(e)}", game_name)
                self._results["failed"] += 1
                return False

            return True

        except Exception as e:
            self._add_log("error", f"Processing failed: {str(e)}", game_name)
            self._results["failed"] += 1
            return False

    def _worker_loop(self, targets: List[Path]):
        """
        Main worker loop for batch processing.

        Args:
            targets: List of game directories to process
        """
        self._total_items = len(targets)
        self._start_time = time.time()

        mode_str = "DRY RUN" if self._dry_run else "REAL"
        self._add_log("info", f"Starting {mode_str} batch scan of {len(targets)} games")

        # Process in chunks
        for i in range(0, len(targets), self._chunk_size):
            # Check for stop
            if self._status == BatchStatus.STOPPING:
                self._add_log("info", "Scan stopped by user")
                break

            chunk = targets[i:i + self._chunk_size]

            for game_dir in chunk:
                if self._status == BatchStatus.STOPPING:
                    break

                self._process_item(game_dir)
                self._processed_items += 1

            # Small delay between chunks
            if i + self._chunk_size < len(targets):
                time.sleep(0.1)

        # Finalize
        if self._status != BatchStatus.STOPPING:
            self._status = BatchStatus.COMPLETED
            self._add_log("success", f"Batch scan completed: {self._results['matched']} matched, {self._results['skipped']} skipped, {self._results['downloaded']} downloaded")

            # PHASE 12: Trigger enrichment after successful scan (optional)
            if not self._dry_run and self._auto_enrich:
                self._add_log("info", "Starting enrichment with external connectors...")
                self._run_enrichment()
        else:
            self._status = BatchStatus.IDLE

    def start_scan(
        self,
        dry_run: bool = True,
        download_screenshots: bool = True,
        prefer_traditional: bool = True,
        targets: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """
        Start a batch metadata scan.

        Args:
            dry_run: If True, simulate without actual downloads
            download_screenshots: If True, download screenshots (only when dry_run=False)
            prefer_traditional: If True, prefer Traditional Chinese
            targets: Optional list of specific game paths (default: auto-discover all)

        Returns:
            Status dictionary
        """
        if self._status == BatchStatus.RUNNING:
            return {
                "success": False,
                "message": "Scan already running"
            }

        if not self._library_root:
            return {
                "success": False,
                "message": "Library root not configured"
            }

        # Reset state
        self._reset_state()
        self._dry_run = dry_run
        self._download_screenshots = download_screenshots
        self._prefer_traditional = prefer_traditional
        self._status = BatchStatus.RUNNING
        self._pause_event.set()

        # Discover targets
        if targets:
            # Use provided targets
            target_paths = [self._library_root / t for t in targets]
        else:
            # Auto-discover all games
            target_paths = self._discover_targets()

        if not target_paths:
            self._status = BatchStatus.IDLE
            return {
                "success": False,
                "message": "No games found to scan"
            }

        # Start worker thread
        self._worker_thread = threading.Thread(
            target=self._worker_loop,
            args=(target_paths,),
            daemon=True
        )
        self._worker_thread.start()

        return {
            "success": True,
            "message": f"Started {'dry run' if dry_run else 'real'} scan of {len(target_paths)} games",
            "total_items": len(target_paths)
        }

    def pause_scan(self) -> Dict[str, Any]:
        """
        Pause the current scan after current item finishes.

        Returns:
            Status dictionary
        """
        if self._status != BatchStatus.RUNNING:
            return {
                "success": False,
                "message": "No scan running"
            }

        self._status = BatchStatus.PAUSED
        self._pause_event.clear()
        self._add_log("info", "Scan paused")

        return {
            "success": True,
            "message": "Scan paused"
        }

    def resume_scan(self) -> Dict[str, Any]:
        """
        Resume a paused scan.

        Returns:
            Status dictionary
        """
        if self._status != BatchStatus.PAUSED:
            return {
                "success": False,
                "message": "No scan paused"
            }

        self._status = BatchStatus.RUNNING
        self._pause_event.set()
        self._add_log("info", "Scan resumed")

        return {
            "success": True,
            "message": "Scan resumed"
        }

    def stop_scan(self) -> Dict[str, Any]:
        """
        Stop the current scan completely.

        Returns:
            Status dictionary
        """
        if self._status not in [BatchStatus.RUNNING, BatchStatus.PAUSED]:
            return {
                "success": False,
                "message": "No scan running"
            }

        self._status = BatchStatus.STOPPING
        self._pause_event.set()  # Unpause if paused
        self._add_log("info", "Stopping scan...")

        # Wait for worker to finish (max 5 seconds)
        if self._worker_thread and self._worker_thread.is_alive():
            self._worker_thread.join(timeout=5.0)

        return {
            "success": True,
            "message": "Scan stopped"
        }

    def get_status(self) -> Dict[str, Any]:
        """
        Get current batch status and progress.

        Returns:
            Status dictionary with progress, logs, and results
        """
        progress_percent = 0.0
        if self._total_items > 0:
            progress_percent = (self._processed_items / self._total_items) * 100

        eta_seconds = None
        if self._start_time and self._processed_items > 0 and self._status == BatchStatus.RUNNING:
            elapsed = time.time() - self._start_time
            rate = self._processed_items / elapsed
            if rate > 0:
                remaining = self._total_items - self._processed_items
                eta_seconds = remaining / rate

        # Add quota status
        quota_status = {}
        if self._resource_manager:
            quota_status = self._resource_manager.get_quota_status()

        return {
            "status": self._status.value,
            "progress_percent": round(progress_percent, 1),
            "processed_count": self._processed_items,
            "total_count": self._total_items,
            "current_item": self._current_file,
            "eta_seconds": round(eta_seconds) if eta_seconds else None,
            "logs": self._logs[-20:],  # Return last 20 logs
            "results": self._results,
            "dry_run": self._dry_run,
            "quota": quota_status,
        }

    def set_auto_enrich(self, enabled: bool):
        """
        Enable or disable auto-enrichment after batch scan.

        PHASE 12: When enabled, triggers enrichment after successful scan.

        Args:
            enabled: Whether to enable auto-enrichment
        """
        self._auto_enrich = enabled
        logger.info(f"Auto-enrichment {'enabled' if enabled else 'disabled'}")

    def _run_enrichment(self):
        """
        Run enrichment on all successfully processed games.

        PHASE 12: Called after batch scan completes (if auto_enrich is enabled).
        Enriches with Steam and Bangumi connectors.
        """
        try:
            from .enricher import get_enricher

            # Get enricher
            enricher = get_enricher(
                library_root=self._library_root,
                rate_limit_delay=1.0,
                download_assets=True
            )

            # Collect successfully processed games
            game_folders = []
            for metadata_file in self._library_root.rglob("metadata.json"):
                if metadata_file.parent != self._library_root:
                    game_folders.append(metadata_file.parent)

            if not game_folders:
                self._add_log("warning", "No games found for enrichment")
                return

            self._add_log("info", f"Enriching {len(game_folders)} games with Steam/Bangumi...")

            # Run enrichment
            results = enricher.enrich_library(
                game_folders=game_folders,
                force_steam=False,
                force_bangumi=False
            )

            # Log results
            self._add_log("success", f"Enrichment complete: {results['success']}/{results['total']} succeeded")
            self._add_log("info", f"  Steam IDs added: {results['steam_added']}")
            self._add_log("info", f"  Bangumi IDs added: {results['bangumi_added']}")
            self._add_log("info", f"  Assets downloaded: {results['assets_downloaded']}")

            if results['errors']:
                self._add_log("warning", f"  Errors: {len(results['errors'])} games failed")

        except Exception as e:
            logger.error(f"Error during enrichment: {e}")
            self._add_log("error", f"Enrichment failed: {str(e)}")


# Global singleton instance
_batch_manager: Optional[BatchManager] = None


def get_batch_manager() -> BatchManager:
    """Get the global BatchManager singleton."""
    global _batch_manager
    if _batch_manager is None:
        _batch_manager = BatchManager()
    return _batch_manager
