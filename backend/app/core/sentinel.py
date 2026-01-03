"""
Sentinel - Noise-Resilient File System Watcher

This module implements a robust file system watcher with:
- Multiple Scanner Modes: REALTIME, SCHEDULED, MANUAL
- Stability Pact: Waits for size/mtime to be static before triggering
- Event Coalescing: Groups events by parent directory
- Dynamic Mode Switching: Change modes without app restart

Designed for NAS/Docker environments where file operations may be noisy.
"""

import json
import logging
import threading
import time
from collections import defaultdict
from dataclasses import dataclass
from datetime import time as dt_time
from enum import Enum
from pathlib import Path
from typing import Callable, Dict, List, Optional, Set

from .path_safety import is_safe_path

try:
    from watchdog.observers import Observer
    from watchdog.events import (
        DirCreated,
        DirDeleted,
        DirModified,
        FileCreated,
        FileDeleted,
        FileModified,
        FileSystemEventHandler,
        FileSystemEvent
    )
    WATCHDOG_AVAILABLE = True
except ImportError:
    WATCHDOG_AVAILABLE = False
    Observer = None
    FileSystemEventHandler = object
    FileSystemEvent = None

try:
    from apscheduler.schedulers.background import BackgroundScheduler
    APSCHEDULER_AVAILABLE = True
except ImportError:
    APSCHEDULER_AVAILABLE = False
    BackgroundScheduler = None

logger = logging.getLogger(__name__)


class ScannerMode(Enum):
    """Scanner operating modes."""
    REALTIME = "realtime"    # Uses watchdog + Stability Pact + Coalescing
    SCHEDULED = "scheduled"  # Daily scan at 03:00 AM
    MANUAL = "manual"        # Idle, manual trigger only


@dataclass
class FileEvent:
    """
    Represents a file system event with stability tracking.

    Attributes:
        path: Path to the file/directory
        event_type: Type of event (created, modified, deleted)
        initial_size: File size when event was first detected
        initial_mtime: File modification time when event was first detected
        first_seen: Timestamp when event was first detected
    """
    path: Path
    event_type: str
    initial_size: int
    initial_mtime: float
    first_seen: float

    def is_stable(self, current_time: float, stability_threshold: float) -> bool:
        """
        Check if file has been stable for the threshold duration.

        A file is stable if:
        1. At least `stability_threshold` seconds have passed since first_seen
        2. Size has not changed
        3. Modification time has not changed

        Args:
            current_time: Current timestamp
            stability_threshold: Minimum seconds of stability required

        Returns:
            True if file is stable, False otherwise
        """
        # Check time threshold
        if current_time - self.first_seen < stability_threshold:
            return False

        # Check if path still exists
        if not self.path.exists():
            return False

        # Get current stats
        try:
            stat = self.path.stat()
            current_size = stat.st_size
            current_mtime = stat.st_mtime
        except OSError:
            return False

        # Check size and mtime stability
        return (current_size == self.initial_size and
                current_mtime == self.initial_mtime)


class StabilityTracker:
    """
    Tracks file events and determines when they are stable.

    Implements the Stability Pact: files must have static size/mtime
    for a configured duration before being considered stable.
    """

    def __init__(self, stability_threshold: float = 45.0):
        """
        Initialize the stability tracker.

        Args:
            stability_threshold: Seconds of stability required (default: 45)
        """
        self.stability_threshold = stability_threshold
        self.tracked_events: Dict[str, FileEvent] = {}
        self._lock = threading.Lock()

    def track_event(self, path: Path, event_type: str) -> Optional[FileEvent]:
        """
        Start tracking a new file event or update existing.

        Args:
            path: Path to the file
            event_type: Type of event (created, modified, deleted)

        Returns:
            FileEvent object (new or existing), None if path doesn't exist
        """
        path_str = str(path)

        with self._lock:
            # If already tracking, return existing event
            if path_str in self.tracked_events:
                return self.tracked_events[path_str]

            # Don't track deleted files (they don't exist)
            if event_type == "deleted":
                return None

            # Get initial stats
            try:
                stat = path.stat()
            except OSError:
                return None

            # Create new tracking event
            event = FileEvent(
                path=path,
                event_type=event_type,
                initial_size=stat.st_size,
                initial_mtime=stat.st_mtime,
                first_seen=time.time()
            )

            self.tracked_events[path_str] = event
            logger.debug(f"Started tracking {event_type} event for {path}")
            return event

    def check_stability(self, current_time: Optional[float] = None) -> List[FileEvent]:
        """
        Check all tracked events and return those that are now stable.

        Args:
            current_time: Current timestamp (defaults to now)

        Returns:
            List of stable FileEvents
        """
        if current_time is None:
            current_time = time.time()

        stable_events = []

        with self._lock:
            for path_str, event in list(self.tracked_events.items()):
                if event.is_stable(current_time, self.stability_threshold):
                    stable_events.append(event)
                    # Remove from tracking
                    del self.tracked_events[path_str]
                    logger.debug(f"Event stabilized for {event.path}")

        return stable_events

    def remove(self, path: Path) -> None:
        """Remove a path from tracking."""
        path_str = str(path)
        with self._lock:
            if path_str in self.tracked_events:
                del self.tracked_events[path_str]


class EventCoalescer:
    """
    Coalesces multiple file events into directory-level actions.

    Instead of triggering 50 separate scans for 50 files in one directory,
    coalesces them into a single scan for the parent directory.
    """

    def __init__(self, coalesce_window: float = 5.0):
        """
        Initialize the event coalescer.

        Args:
            coalesce_window: Seconds to wait before coalescing events
        """
        self.coalesce_window = coalesce_window
        self.pending_events: Dict[Path, Set[str]] = defaultdict(set)
        self._lock = threading.Lock()
        self._timer_thread: Optional[threading.Thread] = None
        self._stop_event = threading.Event()
        self._callback: Optional[Callable[[List[Path]], None]] = None

    def set_callback(self, callback: Callable[[List[Path]], None]) -> None:
        """
        Set the callback to be called when events are coalesced.

        Args:
            callback: Function that receives list of parent directories to scan
        """
        self._callback = callback

    def add_event(self, path: Path) -> None:
        """
        Add a file event for coalescing.

        Events are grouped by parent directory.

        Args:
            path: Path that changed
        """
        parent = path.parent

        with self._lock:
            self.pending_events[parent].add(str(path))

        logger.debug(f"Added to coalescer: {path} (parent: {parent})")

    def flush(self) -> List[Path]:
        """
        Immediately flush all pending events.

        Returns:
            List of parent directories with pending events
        """
        with self._lock:
            parents = list(self.pending_events.keys())
            self.pending_events.clear()
            return parents

    def start(self) -> None:
        """Start the coalescer background thread."""
        if self._timer_thread is not None:
            return

        self._stop_event.clear()
        self._timer_thread = threading.Thread(
            target=self._coalesce_loop,
            daemon=True,
            name="EventCoalescer"
        )
        self._timer_thread.start()
        logger.info("Event coalescer started")

    def stop(self) -> None:
        """Stop the coalescer background thread."""
        if self._timer_thread is None:
            return

        self._stop_event.set()
        self._timer_thread.join(timeout=5.0)
        self._timer_thread = None
        logger.info("Event coalescer stopped")

    def _coalesce_loop(self) -> None:
        """Background thread that periodically coalesces events."""
        while not self._stop_event.is_set():
            try:
                # Wait for coalesce window
                self._stop_event.wait(self.coalesce_window)

                # Check if we should stop
                if self._stop_event.is_set():
                    break

                # Get pending directories
                with self._lock:
                    if not self.pending_events:
                        continue

                    parents = list(self.pending_events.keys())
                    self.pending_events.clear()

                if parents and self._callback:
                    logger.info(f"Coalescing {len(parents)} directory events")
                    try:
                        self._callback(parents)
                    except Exception as e:
                        logger.error(f"Error in coalescer callback: {e}")

            except Exception as e:
                logger.error(f"Error in coalescer loop: {e}")


class SentinelEventHandler(FileSystemEventHandler if WATCHDOG_AVAILABLE else object):
    """
    Watchdog event handler that respects is_safe_path.

    Filters out events outside the library root and forwards
    safe events to the stability tracker.
    """

    def __init__(
        self,
        library_root: Path,
        stability_tracker: StabilityTracker,
        coalescer: EventCoalescer
    ):
        """
        Initialize the event handler.

        Args:
            library_root: Root directory for path validation
            stability_tracker: StabilityTracker instance
            coalescer: EventCoalescer instance
        """
        self.library_root = Path(library_root)
        self.stability_tracker = stability_tracker
        self.coalescer = coalescer

    def _process_event(self, event: FileSystemEvent) -> None:
        """
        Process a file system event if it's safe.

        Args:
            event: Watchdog FileSystemEvent
        """
        path = Path(event.src_path)

        # Validate path is safe
        if not is_safe_path(path, self.library_root):
            logger.debug(f"Ignoring unsafe path: {path}")
            return

        # Determine event type
        if isinstance(event, (FileCreated, DirCreated)):
            event_type = "created"
        elif isinstance(event, (FileModified, DirModified)):
            event_type = "modified"
        elif isinstance(event, (FileDeleted, DirDeleted)):
            event_type = "deleted"
        else:
            event_type = "modified"  # Fallback

        # Track for stability
        tracked = self.stability_tracker.track_event(path, event_type)

        # For immediate processing of deleted files
        if event_type == "deleted":
            self.coalescer.add_event(path)

        logger.debug(f"Processed {event_type} event: {path}")

    def on_created(self, event: FileSystemEvent) -> None:
        """Handle file/directory creation."""
        self._process_event(event)

    def on_modified(self, event: FileSystemEvent) -> None:
        """Handle file/directory modification."""
        self._process_event(event)

    def on_deleted(self, event: FileSystemEvent) -> None:
        """Handle file/directory deletion."""
        self._process_event(event)

    def on_moved(self, event: FileSystemEvent) -> None:
        """Handle file/directory move/rename."""
        # Process as deletion of old path and creation of new
        if hasattr(event, 'dest_path'):
            old_path = Path(event.src_path)
            new_path = Path(event.dest_path)

            if is_safe_path(old_path, self.library_root):
                self.coalescer.add_event(old_path)
            if is_safe_path(new_path, self.library_root):
                self.coalescer.add_event(new_path)


class PollingWatcher:
    """
    Fallback polling-based watcher for environments where watchdog fails.

    Uses INCREMENTAL scanning with a snapshot (path -> mtime) to minimize I/O.
    Only processes files that have actually changed since last poll.
    """

    def __init__(
        self,
        library_root: Path,
        poll_interval: float = 600.0,  # 10 minutes (optimized for reduced I/O)
        stability_tracker: Optional[StabilityTracker] = None,
        coalescer: Optional[EventCoalescer] = None
    ):
        """
        Initialize the polling watcher.

        Args:
            library_root: Root directory to watch
            poll_interval: Seconds between polls (default: 600 = 10 min, optimized for I/O)
            stability_tracker: Optional stability tracker
            coalescer: Optional event coalescer
        """
        self.library_root = Path(library_root)
        self.poll_interval = poll_interval
        self.stability_tracker = stability_tracker
        self.coalescer = coalescer

        # Snapshot for incremental scanning: path -> mtime
        self._snapshot: Dict[str, float] = {}

        self._thread: Optional[threading.Thread] = None
        self._stop_event = threading.Event()
        self._scan_callback: Optional[Callable[[List[Path]], None]] = None

    def set_scan_callback(self, callback: Callable[[List[Path]], None]) -> None:
        """
        Set callback for when changes are detected.

        Args:
            callback: Function receiving list of changed directories
        """
        self._scan_callback = callback

    def _scan_directory(self, directory: Path) -> Set[Path]:
        """
        Incrementally scan a directory and return changed paths.

        Uses snapshot comparison to minimize I/O - only processes files
        that have actually changed since the last poll.

        Args:
            directory: Directory to scan

        Returns:
            Set of paths that changed
        """
        changed = set()

        if not directory.exists() or not is_safe_path(directory, self.library_root):
            return changed

        try:
            # Get current state of all files
            current_files: Dict[str, float] = {}

            for item in directory.rglob("*"):
                if not is_safe_path(item, self.library_root):
                    continue

                try:
                    stat = item.stat()
                    mtime = stat.st_mtime
                    path_str = str(item)

                    # Store in current state
                    current_files[path_str] = mtime

                    # Check if file is new or modified
                    if path_str not in self._snapshot:
                        # New file
                        changed.add(item)

                        # Track for stability
                        if self.stability_tracker:
                            self.stability_tracker.track_event(item, "created")

                    elif abs(self._snapshot[path_str] - mtime) > 0.001:
                        # Modified file
                        changed.add(item)

                        # Track for stability
                        if self.stability_tracker:
                            self.stability_tracker.track_event(item, "modified")

                except OSError:
                    # File might have been deleted during scan, ignore
                    pass

            # Check for deleted files (in snapshot but not in current)
            deleted_files = set(self._snapshot.keys()) - set(current_files.keys())
            for deleted_path_str in deleted_files:
                deleted_path = Path(deleted_path_str)
                if deleted_path.exists():
                    # File still exists but might have been moved
                    # Add as modified to trigger rescan
                    changed.add(deleted_path)
                else:
                    # File was actually deleted
                    if self.stability_tracker:
                        self.stability_tracker.track_event(deleted_path, "deleted")
                    # Add parent directory to trigger rescan
                    if deleted_path.parent.exists():
                        changed.add(deleted_path.parent)

            # Update snapshot for next poll
            self._snapshot = current_files

        except OSError as e:
            logger.error(f"Error scanning directory {directory}: {e}")

        return changed

    def _poll_loop(self) -> None:
        """Background polling loop."""
        logger.info(f"Polling watcher started for {self.library_root}")

        # Initial scan
        self._scan_directory(self.library_root)

        while not self._stop_event.is_set():
            try:
                # Wait for poll interval
                self._stop_event.wait(self.poll_interval)

                if self._stop_event.is_set():
                    break

                # Scan for changes
                changed = self._scan_directory(self.library_root)

                if changed:
                    # Group by parent directory
                    parents = {p.parent for p in changed}

                    if self.coalescer:
                        for path in changed:
                            self.coalescer.add_event(path)
                    elif self._scan_callback:
                        try:
                            self._scan_callback(list(parents))
                        except Exception as e:
                            logger.error(f"Error in scan callback: {e}")

                # Save snapshot after successful poll (for persistence)
                self.save_snapshot()

            except Exception as e:
                logger.error(f"Error in polling loop: {e}")

        logger.info("Polling watcher stopped")

    def start(self) -> None:
        """Start the polling watcher."""
        if self._thread is not None:
            return

        # Try to load previous snapshot for instant boot
        self.load_snapshot()

        self._stop_event.clear()
        self._thread = threading.Thread(
            target=self._poll_loop,
            daemon=True,
            name="PollingWatcher"
        )
        self._thread.start()

    def stop(self) -> None:
        """Stop the polling watcher."""
        if self._thread is None:
            return

        self._stop_event.set()
        self._thread.join(timeout=10.0)
        self._thread = None

    def _get_snapshot_path(self) -> Path:
        """Get the path to the snapshot file."""
        return self.library_root / ".polling_snapshot.json"

    def save_snapshot(self) -> None:
        """
        Save current snapshot to disk for persistence across restarts.

        This enables instant boot on next startup by loading the previous
        snapshot instead of doing a full initial scan.
        """
        snapshot_path = self._get_snapshot_path()

        try:
            # Convert snapshot to JSON-serializable format
            snapshot_data = {
                "version": 1,
                "timestamp": time.time(),
                "snapshot": self._snapshot
            }

            with open(snapshot_path, "w") as f:
                json.dump(snapshot_data, f)

            logger.debug(f"Saved polling snapshot with {len(self._snapshot)} entries")

        except Exception as e:
            logger.error(f"Failed to save polling snapshot: {e}")

    def load_snapshot(self) -> bool:
        """
        Load snapshot from disk.

        If successful, skips initial full scan and uses loaded snapshot.

        Returns:
            True if snapshot loaded successfully, False otherwise
        """
        snapshot_path = self._get_snapshot_path()

        if not snapshot_path.exists():
            logger.info("No polling snapshot found, will perform initial scan")
            return False

        try:
            with open(snapshot_path, "r") as f:
                snapshot_data = json.load(f)

            version = snapshot_data.get("version", 0)
            if version != 1:
                logger.warning(f"Incompatible snapshot version {version}, ignoring")
                return False

            # Load snapshot
            self._snapshot = snapshot_data.get("snapshot", {})

            snapshot_time = snapshot_data.get("timestamp", 0)
            snapshot_age = time.time() - snapshot_time

            logger.info(f"Loaded polling snapshot with {len(self._snapshot)} entries "
                       f"(age: {snapshot_age:.0f}s)")

            return True

        except Exception as e:
            logger.error(f"Failed to load polling snapshot: {e}")
            return False


class Sentinel:
    """
    Noise-resilient file system watcher with multiple scanner modes.

    **PHASE 9 UPDATE - Multi-Root Support:**
    - Monitors multiple library roots simultaneously
    - Tracks which root an event originated from

    Features:
    - Multiple Scanner Modes: REALTIME, SCHEDULED, MANUAL
    - Dynamic Mode Switching: Change modes without app restart
    - Stability Pact: Waits for files to be stable before triggering (REALTIME only)
    - Event Coalescing: Groups events by parent directory (REALTIME only)
    - Path Safety: All paths validated against library roots

    Usage:
        def on_directories_changed(dirs: List[Path]):
            print(f"Changed: {dirs}")

        sentinel = Sentinel(
            library_roots=[Path("/games"), Path("/nas/archive")],
            callback=on_directories_changed
        )
        sentinel.configure(ScannerMode.REALTIME)
        sentinel.start()

        # Later, switch mode without restart
        sentinel.configure(ScannerMode.SCHEDULED)
    """

    def __init__(
        self,
        library_roots,  # Accept both List[Path] and single Path for backward compatibility
        callback: Callable[[List[Path]], None],
        stability_threshold: float = 45.0,
        coalesce_window: float = 5.0,
        poll_interval: float = 600.0,  # 10 minutes (optimized for reduced I/O)
        scheduled_time: str = "03:00",
        initial_mode: ScannerMode = ScannerMode.MANUAL
    ):
        """
        Initialize the Sentinel.

        Args:
            library_roots: Root directory(ies) to watch (List[Path] or single Path)
            callback: Function to call when directories change (receives List[Path])
            stability_threshold: Seconds of stability required (default: 45, REALTIME only)
            coalesce_window: Seconds to wait before coalescing events (default: 5, REALTIME only)
            poll_interval: Polling interval in seconds (default: 600 = 10 min, REALTIME fallback, optimized)
            scheduled_time: Time for daily scan in "HH:MM" format (default: "03:00", SCHEDULED only)
            initial_mode: Starting scanner mode (default: MANUAL)
        """
        # PHASE 9: Support multiple roots
        if isinstance(library_roots, Path):
            self.library_roots = [library_roots]
        elif isinstance(library_roots, str):
            self.library_roots = [Path(library_roots)]
        else:
            self.library_roots = [Path(p) if isinstance(p, str) else p for p in library_roots]

        # Legacy: Keep library_root for backward compatibility
        self.library_root = self.library_roots[0]

        self._user_callback = callback
        self.stability_threshold = stability_threshold
        self.coalesce_window = coalesce_window
        self.poll_interval = poll_interval
        self.scheduled_time = scheduled_time

        # Current mode and running state
        self.mode = initial_mode
        self._is_running = False

        # Initialize components
        self.stability_tracker = StabilityTracker(stability_threshold)
        self.coalescer = EventCoalescer(coalesce_window)
        self.coalescer.set_callback(self._on_coalesced_events)

        # REALTIME mode components
        self._observer = None  # Will be a list of observers for multi-root
        self._polling_watcher = None  # Will be a list of polling watchers for multi-root
        self._stability_thread = None
        self._stability_stop_event = None

        # SCHEDULED mode components
        self._scheduler = None
        self._scheduler_job = None

        # Lock for mode switching
        self._mode_lock = threading.Lock()

        logger.info(f"Sentinel initialized with {initial_mode.value} mode for {len(self.library_roots)} roots")

    def _on_coalesced_events(self, directories: List[Path]) -> None:
        """
        Internal callback when events are coalesced.

        Args:
            directories: List of parent directories with changes
        """
        try:
            self._user_callback(directories)
        except Exception as e:
            logger.error(f"Error in user callback: {e}")

    def _scan_library(self) -> List[Path]:
        """
        Perform a full scan of all library directories.

        **PHASE 9 UPDATE:**
        - Scans all library roots
        - Validates paths against all roots

        Returns:
            List of directories that should be scanned/processed

        Note:
            This is a simple implementation. In production, you would
            compare against a database to find actual changes.
        """
        changed_dirs = set()
        items_seen = 0
        items_skipped_unsafe = 0
        items_skipped_file = 0

        try:
            # PHASE 9: Walk all library roots
            for library_root in self.library_roots:
                # Walk the library and find all subdirectories
                for item in library_root.rglob("*"):
                    items_seen += 1

                    # Check path safety first (against ALL roots)
                    if not any(is_safe_path(item, root) for root in self.library_roots):
                        items_skipped_unsafe += 1
                        logger.debug(f"Scan: Skipped unsafe path: {item}")
                        continue

                    if item.is_dir():
                        # Add subdirectories
                        changed_dirs.add(item)
                        logger.debug(f"Scan: Found directory: {item}")
                    else:
                        # For files, add their parent directory
                        parent = item.parent
                        changed_dirs.add(parent)
                        items_skipped_file += 1
                        logger.debug(f"Scan: Found file {item}, adding parent {parent}")

            # Edge case: If library root itself has files but no subdirs,
            # include the library root in results
            if not changed_dirs and items_seen > 0:
                for library_root in self.library_roots:
                    changed_dirs.add(library_root)
                logger.debug(f"Scan: No subdirs found, including library roots")

            logger.info(
                f"Scan completed: {items_seen} items seen, "
                f"{len(changed_dirs)} directories to process, "
                f"{items_skipped_unsafe} unsafe, {items_skipped_file} files"
            )

        except Exception as e:
            logger.error(f"Error scanning library: {e}")

        return list(changed_dirs)

    def _trigger_scheduled_scan(self) -> None:
        """Internal method called by scheduler for SCHEDULED mode."""
        logger.info("Triggering scheduled scan")
        try:
            changed_dirs = self._scan_library()
            if changed_dirs:
                self._user_callback(changed_dirs)
        except Exception as e:
            logger.error(f"Error in scheduled scan: {e}")

    def _start_realtime_mode(self) -> None:
        """Start REALTIME mode: watchdog + stability pact + coalescing."""
        logger.info("Starting REALTIME mode")

        # Start coalescer
        self.coalescer.start()

        # PHASE 9: Try to initialize watchdog for ALL roots
        if WATCHDOG_AVAILABLE:
            try:
                self._observer = []  # List of observers
                for library_root in self.library_roots:
                    observer = Observer()
                    handler = SentinelEventHandler(
                        library_root,
                        self.stability_tracker,
                        self.coalescer
                    )
                    observer.schedule(handler, str(library_root), recursive=True)
                    observer.start()
                    self._observer.append(observer)
                logger.info(f"REALTIME mode: watchdog observers started for {len(self.library_roots)} roots")
                return
            except Exception as e:
                logger.warning(f"Failed to start watchdog: {e}. Falling back to polling.")
                self._observer = None

        # Fallback to polling for ALL roots
        self._polling_watcher = []
        for library_root in self.library_roots:
            polling_watcher = PollingWatcher(
                library_root,
                self.poll_interval,
                self.stability_tracker,
                self.coalescer
            )
            polling_watcher.set_scan_callback(self._on_coalesced_events)
            polling_watcher.start()
            self._polling_watcher.append(polling_watcher)
        logger.info(f"REALTIME mode: polling watchers started for {len(self.library_roots)} roots")

        # Start stability checker (for both watchdog and polling)
        self._start_stability_checker()

    def _stop_realtime_mode(self) -> None:
        """Stop REALTIME mode components."""
        logger.info("Stopping REALTIME mode")

        # Stop stability checker
        self._stop_stability_checker()

        # Stop coalescer
        self.coalescer.stop()

        # PHASE 9: Stop all observers
        if self._observer:
            if isinstance(self._observer, list):
                for observer in self._observer:
                    observer.stop()
                    observer.join(timeout=5.0)
                self._observer = None
                logger.info(f"REALTIME mode: watchdog observers stopped")
            else:
                # Legacy: single observer
                self._observer.stop()
                self._observer.join(timeout=5.0)
                self._observer = None
                logger.info("REALTIME mode: watchdog observer stopped")

        # PHASE 9: Stop all polling watchers
        if self._polling_watcher:
            if isinstance(self._polling_watcher, list):
                for watcher in self._polling_watcher:
                    watcher.stop()
                self._polling_watcher = None
                logger.info(f"REALTIME mode: polling watchers stopped")
            else:
                # Legacy: single watcher
                self._polling_watcher.stop()
                self._polling_watcher = None
                logger.info("REALTIME mode: polling watcher stopped")

    def _start_scheduled_mode(self) -> None:
        """Start SCHEDULED mode: daily scan at specified time."""
        if not APSCHEDULER_AVAILABLE:
            logger.error("APScheduler not available. Cannot use SCHEDULED mode.")
            return

        logger.info(f"Starting SCHEDULED mode at {self.scheduled_time}")

        self._scheduler = BackgroundScheduler()

        # Parse scheduled time
        hour, minute = map(int, self.scheduled_time.split(":"))

        # Add daily job
        self._scheduler_job = self._scheduler.add_job(
            self._trigger_scheduled_scan,
            'cron',
            hour=hour,
            minute=minute,
            id='daily_scan'
        )

        self._scheduler.start()
        logger.info(f"SCHEDULED mode: scheduler started for {self.scheduled_time} daily")

    def _stop_scheduled_mode(self) -> None:
        """Stop SCHEDULED mode components."""
        logger.info("Stopping SCHEDULED mode")

        if self._scheduler:
            self._scheduler.shutdown(wait=True)
            self._scheduler = None
            self._scheduler_job = None
            logger.info("SCHEDULED mode: scheduler stopped")

    def _start_manual_mode(self) -> None:
        """Start MANUAL mode: idle, waiting for manual triggers."""
        logger.info("Starting MANUAL mode (idle)")
        # MANUAL mode has no background components

    def _stop_manual_mode(self) -> None:
        """Stop MANUAL mode components."""
        logger.info("Stopping MANUAL mode")
        # Nothing to stop in MANUAL mode

    def configure(self, new_mode: ScannerMode) -> None:
        """
        Dynamically switch scanner modes without restarting the app.

        Args:
            new_mode: The new ScannerMode to switch to

        Example:
            sentinel.configure(ScannerMode.REALTIME)
            # Later...
            sentinel.configure(ScannerMode.SCHEDULED)
        """
        with self._mode_lock:
            if self.mode == new_mode:
                logger.info(f"Already in {new_mode.value} mode")
                return

            logger.info(f"Switching from {self.mode.value} to {new_mode.value}")

            # Stop current mode
            if self._is_running:
                if self.mode == ScannerMode.REALTIME:
                    self._stop_realtime_mode()
                elif self.mode == ScannerMode.SCHEDULED:
                    self._stop_scheduled_mode()
                elif self.mode == ScannerMode.MANUAL:
                    self._stop_manual_mode()

            # Switch to new mode
            self.mode = new_mode

            # Start new mode if already running
            if self._is_running:
                if new_mode == ScannerMode.REALTIME:
                    self._start_realtime_mode()
                elif new_mode == ScannerMode.SCHEDULED:
                    self._start_scheduled_mode()
                elif new_mode == ScannerMode.MANUAL:
                    self._start_manual_mode()

            logger.info(f"Switched to {new_mode.value} mode")

    def start(self) -> None:
        """Start the sentinel in the current mode."""
        with self._mode_lock:
            if self._is_running:
                logger.warning("Sentinel is already running")
                return

            self._is_running = True

            if self.mode == ScannerMode.REALTIME:
                self._start_realtime_mode()
            elif self.mode == ScannerMode.SCHEDULED:
                self._start_scheduled_mode()
            elif self.mode == ScannerMode.MANUAL:
                self._start_manual_mode()

            logger.info(f"Sentinel started in {self.mode.value} mode")

    def stop(self) -> None:
        """Stop the sentinel."""
        with self._mode_lock:
            if not self._is_running:
                logger.warning("Sentinel is not running")
                return

            if self.mode == ScannerMode.REALTIME:
                self._stop_realtime_mode()
            elif self.mode == ScannerMode.SCHEDULED:
                self._stop_scheduled_mode()
            elif self.mode == ScannerMode.MANUAL:
                self._stop_manual_mode()

            self._is_running = False
            logger.info("Sentinel stopped")

    def trigger_scan(self) -> List[Path]:
        """
        Manually trigger a library scan (MANUAL mode).

        Can be called in any mode, but primarily useful in MANUAL mode.

        Returns:
            List of directories that were scanned

        Example:
            sentinel.configure(ScannerMode.MANUAL)
            sentinel.start()
            # Later...
            changed = sentinel.trigger_scan()
        """
        logger.info("Manual scan triggered")

        changed_dirs = self._scan_library()

        if changed_dirs:
            try:
                self._user_callback(changed_dirs)
                logger.info(f"Manual scan completed: {len(changed_dirs)} directories")
            except Exception as e:
                logger.error(f"Error in manual scan callback: {e}")

        return changed_dirs

    def _start_stability_checker(self) -> None:
        """Start the background stability checker thread (REALTIME mode)."""
        self._stability_stop_event = threading.Event()
        self._stability_thread = threading.Thread(
            target=self._stability_check_loop,
            daemon=True,
            name="StabilityChecker"
        )
        self._stability_thread.start()
        logger.debug("Stability checker thread started")

    def _stop_stability_checker(self) -> None:
        """Stop the stability checker thread (REALTIME mode)."""
        if self._stability_thread:
            self._stability_stop_event.set()
            self._stability_thread.join(timeout=5.0)
            self._stability_thread = None
            logger.debug("Stability checker thread stopped")

    def _stability_check_loop(self) -> None:
        """Background thread that checks for stable events (REALTIME mode)."""
        while not self._stability_stop_event.is_set():
            try:
                # Check every 5 seconds
                self._stability_stop_event.wait(5.0)

                if self._stability_stop_event.is_set():
                    break

                # Get stable events
                stable_events = self.stability_tracker.check_stability()

                if stable_events:
                    # Add to coalescer
                    for event in stable_events:
                        self.coalescer.add_event(event.path)

                    logger.debug(f"Found {len(stable_events)} stable events")

            except Exception as e:
                logger.error(f"Error in stability check loop: {e}")

    def get_mode(self) -> ScannerMode:
        """
        Get the current scanner mode.

        Returns:
            Current ScannerMode
        """
        return self.mode

    def is_running(self) -> bool:
        """
        Check if the sentinel is currently running.

        Returns:
            True if running, False otherwise
        """
        return self._is_running
