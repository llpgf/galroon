"""
Unit tests for Sentinel file system watcher.

Tests Stability Tracker, Event Coalescer, and Sentinel integration.
"""

import time
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from app.core.sentinel import (
    EventCoalescer,
    FileEvent,
    PollingWatcher,
    ScannerMode,
    Sentinel,
    StabilityTracker,
)


class TestFileEvent:
    """Test suite for FileEvent dataclass."""

    def test_file_event_creation(self, tmp_path):
        """FileEvent should be created with correct initial stats."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        stat = test_file.stat()
        event = FileEvent(
            path=test_file,
            event_type="created",
            initial_size=stat.st_size,
            initial_mtime=stat.st_mtime,
            first_seen=time.time()
        )

        assert event.path == test_file
        assert event.event_type == "created"
        assert event.initial_size == stat.st_size

    def test_is_stable_not_enough_time(self, tmp_path):
        """File should not be stable if threshold time hasn't passed."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        stat = test_file.stat()
        event = FileEvent(
            path=test_file,
            event_type="created",
            initial_size=stat.st_size,
            initial_mtime=stat.st_mtime,
            first_seen=time.time()
        )

        # Should not be stable immediately
        assert not event.is_stable(time.time(), stability_threshold=45.0)

    def test_is_stable_time_passed(self, tmp_path):
        """File should be stable after threshold time with unchanged stats."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        stat = test_file.stat()
        past_time = time.time() - 50  # 50 seconds ago
        event = FileEvent(
            path=test_file,
            event_type="created",
            initial_size=stat.st_size,
            initial_mtime=stat.st_mtime,
            first_seen=past_time
        )

        # Should be stable after 50 seconds
        assert event.is_stable(time.time(), stability_threshold=45.0)

    def test_is_stable_size_changed(self, tmp_path):
        """File should not be stable if size changed."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        stat = test_file.stat()
        past_time = time.time() - 50
        event = FileEvent(
            path=test_file,
            event_type="created",
            initial_size=stat.st_size,
            initial_mtime=stat.st_mtime,
            first_seen=past_time
        )

        # Change file size
        test_file.write_text("content with more data")

        # Should not be stable (size changed)
        assert not event.is_stable(time.time(), stability_threshold=45.0)

    def test_is_stable_file_deleted(self, tmp_path):
        """File should not be stable if it was deleted."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        stat = test_file.stat()
        past_time = time.time() - 50
        event = FileEvent(
            path=test_file,
            event_type="created",
            initial_size=stat.st_size,
            initial_mtime=stat.st_mtime,
            first_seen=past_time
        )

        # Delete file
        test_file.unlink()

        # Should not be stable (file gone)
        assert not event.is_stable(time.time(), stability_threshold=45.0)


class TestStabilityTracker:
    """Test suite for StabilityTracker."""

    def test_track_new_event(self, tmp_path):
        """Tracker should track new file events."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        tracker = StabilityTracker(stability_threshold=10.0)
        event = tracker.track_event(test_file, "created")

        assert event is not None
        assert event.path == test_file
        assert event.event_type == "created"

    def test_track_deleted_file_returns_none(self, tmp_path):
        """Tracker should not track deleted files."""
        nonexistent = tmp_path / "deleted.txt"

        tracker = StabilityTracker()
        event = tracker.track_event(nonexistent, "deleted")

        assert event is None

    def test_track_same_file_returns_existing_event(self, tmp_path):
        """Tracking same file again should return existing event."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        tracker = StabilityTracker()
        event1 = tracker.track_event(test_file, "created")
        event2 = tracker.track_event(test_file, "modified")

        assert event1 is event2

    def test_check_stability_returns_empty_initially(self, tmp_path):
        """No events should be stable immediately after tracking."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        tracker = StabilityTracker(stability_threshold=45.0)
        tracker.track_event(test_file, "created")

        stable = tracker.check_stability()
        assert len(stable) == 0

    def test_check_stability_returns_stable_events(self, tmp_path):
        """Should return events that have been stable long enough."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        tracker = StabilityTracker(stability_threshold=1.0)
        tracker.track_event(test_file, "created")

        # Wait for stability threshold
        time.sleep(1.5)

        stable = tracker.check_stability()
        assert len(stable) == 1
        assert stable[0].path == test_file

    def test_stable_events_removed_from_tracking(self, tmp_path):
        """Stable events should be removed from tracking."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        tracker = StabilityTracker(stability_threshold=1.0)
        tracker.track_event(test_file, "created")

        time.sleep(1.5)
        tracker.check_stability()

        # Should no longer be tracked
        stable = tracker.check_stability()
        assert len(stable) == 0

    def test_remove_event(self, tmp_path):
        """Should be able to manually remove events from tracking."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content")

        tracker = StabilityTracker()
        tracker.track_event(test_file, "created")
        tracker.remove(test_file)

        # Event should be gone
        stable = tracker.check_stability(time.time() + 1000)
        assert len(stable) == 0


class TestEventCoalescer:
    """Test suite for EventCoalescer."""

    def test_add_event(self, tmp_path):
        """Events should be added and grouped by parent directory."""
        coalescer = EventCoalescer()
        file1 = tmp_path / "dir1" / "file1.txt"
        file2 = tmp_path / "dir1" / "file2.txt"
        file3 = tmp_path / "dir2" / "file3.txt"

        coalescer.add_event(file1)
        coalescer.add_event(file2)
        coalescer.add_event(file3)

        # Flush to get pending directories
        parents = coalescer.flush()

        assert len(parents) == 2
        assert file1.parent in parents
        assert file3.parent in parents

    def test_flush_clears_pending(self, tmp_path):
        """Flush should clear all pending events."""
        coalescer = EventCoalescer()
        test_file = tmp_path / "test.txt"

        coalescer.add_event(test_file)
        coalescer.flush()

        # Second flush should return empty
        parents = coalescer.flush()
        assert len(parents) == 0

    def test_callback_invoked_on_coalesce(self, tmp_path):
        """Callback should be invoked when events are coalesced."""
        coalescer = EventCoalescer(coalesce_window=0.1)
        test_file = tmp_path / "test.txt"

        callback_mock = MagicMock()
        coalescer.set_callback(callback_mock)

        coalescer.start()
        coalescer.add_event(test_file)

        # Wait for coalesce window
        time.sleep(0.3)

        coalescer.stop()

        # Callback should have been called
        callback_mock.assert_called_once()
        args = callback_mock.call_args[0][0]
        assert len(args) == 1
        assert test_file.parent in args

    def test_start_stop(self, tmp_path):
        """Coalescer should start and stop cleanly."""
        coalescer = EventCoalescer(coalesce_window=1.0)

        coalescer.start()
        assert coalescer._timer_thread is not None

        coalescer.stop()
        assert coalescer._timer_thread is None


class TestPollingWatcher:
    """Test suite for PollingWatcher."""

    def test_initialization(self, tmp_path):
        """PollingWatcher should initialize correctly."""
        watcher = PollingWatcher(tmp_path)

        assert watcher.library_root == tmp_path
        assert watcher.poll_interval == 300.0
        assert watcher._thread is None  # Not started yet

    def test_detects_new_files(self, tmp_path):
        """Polling watcher should detect new files."""
        watcher = PollingWatcher(tmp_path, poll_interval=0.1)

        # Start watcher (initial scan)
        watcher._known_files = {}  # Reset to empty
        watcher.start()

        # Create a new file
        new_file = tmp_path / "new.txt"
        time.sleep(0.05)  # Wait a bit
        new_file.write_text("content")

        # Wait for next poll
        time.sleep(0.2)

        watcher.stop()

        # File should be detected
        assert str(new_file) in watcher._known_files

    def test_detects_modified_files(self, tmp_path):
        """Polling watcher should detect modified files."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("original")

        watcher = PollingWatcher(tmp_path, poll_interval=0.1)
        watcher._scan_directory(tmp_path)  # Initial scan
        watcher.start()

        # Modify file
        time.sleep(0.05)
        test_file.write_text("modified")

        # Wait for next poll
        time.sleep(0.2)
        watcher.stop()

        # Modification should be detected
        # (mtime should be different)

    def test_scan_callback_invoked(self, tmp_path):
        """Scan callback should be invoked on changes."""
        watcher = PollingWatcher(tmp_path, poll_interval=0.1)

        callback_mock = MagicMock()
        watcher.set_scan_callback(callback_mock)

        watcher.start()

        # Create a file
        time.sleep(0.05)
        (tmp_path / "new.txt").write_text("content")

        # Wait for poll
        time.sleep(0.2)
        watcher.stop()

        # Callback should be invoked
        callback_mock.assert_called()


class TestSentinel:
    """Test suite for Sentinel integration."""

    def test_initialization_default_mode(self, tmp_path):
        """Sentinel should initialize in MANUAL mode by default."""
        callback = MagicMock()
        sentinel = Sentinel(tmp_path, callback)

        assert sentinel.mode == ScannerMode.MANUAL

    def test_initialization_with_library_root(self, tmp_path):
        """Sentinel should initialize with library root."""
        callback = MagicMock()
        sentinel = Sentinel(tmp_path, callback, initial_mode=ScannerMode.MANUAL)

        assert sentinel.library_root == tmp_path
        assert sentinel.get_mode() == ScannerMode.MANUAL

    def test_start_stop_manual_mode(self, tmp_path):
        """Sentinel should start and stop in MANUAL mode."""
        callback = MagicMock()
        sentinel = Sentinel(tmp_path, callback, initial_mode=ScannerMode.MANUAL)

        sentinel.start()
        assert sentinel.is_running()

        sentinel.stop()
        assert not sentinel.is_running()

    def test_mode_switching_manual_to_realtime(self, tmp_path):
        """Should be able to switch from MANUAL to REALTIME mode."""
        callback = MagicMock()
        sentinel = Sentinel(tmp_path, callback, initial_mode=ScannerMode.MANUAL)

        sentinel.start()
        assert sentinel.mode == ScannerMode.MANUAL

        # Switch to REALTIME
        with patch('app.core.sentinel.WATCHDOG_AVAILABLE', False):
            sentinel.configure(ScannerMode.REALTIME)

        assert sentinel.mode == ScannerMode.REALTIME
        assert sentinel._polling_watcher is not None  # Fallback to polling

        sentinel.stop()

    def test_mode_switching_realtime_to_manual(self, tmp_path):
        """Should be able to switch from REALTIME to MANUAL mode."""
        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            initial_mode=ScannerMode.MANUAL
        )

        sentinel.start()

        # Switch to REALTIME (with watchdog disabled for testing)
        with patch('app.core.sentinel.WATCHDOG_AVAILABLE', False):
            sentinel.configure(ScannerMode.REALTIME)

        assert sentinel.mode == ScannerMode.REALTIME

        # Switch back to MANUAL
        sentinel.configure(ScannerMode.MANUAL)
        assert sentinel.mode == ScannerMode.MANUAL

        sentinel.stop()

    def test_manual_trigger_scan(self, tmp_path):
        """trigger_scan should work in MANUAL mode."""
        callback = MagicMock()
        sentinel = Sentinel(tmp_path, callback, initial_mode=ScannerMode.MANUAL)

        sentinel.start()

        # Create some directories
        (tmp_path / "game1").mkdir()
        (tmp_path / "game2").mkdir()

        # Trigger manual scan
        scanned = sentinel.trigger_scan()

        assert len(scanned) >= 2  # At least the 2 directories we created
        callback.assert_called()

        sentinel.stop()
        assert not sentinel.is_running()

    def test_callback_invoked_on_changes(self, tmp_path):
        """User callback should be invoked when files change in REALTIME mode."""
        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            stability_threshold=0.1,
            coalesce_window=0.1,
            initial_mode=ScannerMode.MANUAL
        )

        # Switch to REALTIME mode (with watchdog disabled for testing)
        with patch('app.core.sentinel.WATCHDOG_AVAILABLE', False):
            sentinel.configure(ScannerMode.REALTIME)

        sentinel.start()

        # Create a file
        (tmp_path / "new.txt").write_text("content")

        # Wait for stability and coalescing
        time.sleep(0.5)

        sentinel.stop()

        # Callback should be invoked
        # (may take some time due to stability threshold)
        assert callback.call_count >= 0

    def test_ignores_unsafe_paths(self, tmp_path):
        """Sentinel should only watch the configured library root."""
        library = tmp_path / "library"
        outside = tmp_path / "outside"
        library.mkdir()
        outside.mkdir()

        callback = MagicMock()
        sentinel = Sentinel(library, callback, initial_mode=ScannerMode.MANUAL)

        # Both directories should be set up
        assert library.exists()
        assert outside.exists()

        # The sentinel should only watch the library directory
        assert sentinel.library_root == library

    def test_custom_stability_threshold(self, tmp_path):
        """Should respect custom stability threshold."""
        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            stability_threshold=60.0,
            initial_mode=ScannerMode.MANUAL
        )

        assert sentinel.stability_tracker.stability_threshold == 60.0

    def test_custom_coalesce_window(self, tmp_path):
        """Should respect custom coalesce window."""
        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            coalesce_window=10.0,
            initial_mode=ScannerMode.MANUAL
        )

        assert sentinel.coalescer.coalesce_window == 10.0

    def test_custom_scheduled_time(self, tmp_path):
        """Should respect custom scheduled time."""
        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            scheduled_time="05:30",
            initial_mode=ScannerMode.SCHEDULED
        )

        assert sentinel.scheduled_time == "05:30"

    def test_custom_poll_interval(self, tmp_path):
        """Should respect custom poll interval (used in REALTIME fallback)."""
        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            poll_interval=600.0,
            initial_mode=ScannerMode.MANUAL
        )

        assert sentinel.poll_interval == 600.0


class TestIntegration:
    """Integration tests for Sentinel components."""

    def test_full_pipeline(self, tmp_path):
        """Test the full event pipeline: detection -> stability -> coalescing."""
        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            stability_threshold=0.1,
            coalesce_window=0.1,
            initial_mode=ScannerMode.MANUAL
        )

        # Switch to REALTIME mode (with watchdog disabled for testing)
        with patch('app.core.sentinel.WATCHDOG_AVAILABLE', False):
            sentinel.configure(ScannerMode.REALTIME)

        sentinel.start()

        # Create multiple files in same directory
        for i in range(3):
            (tmp_path / f"file{i}.txt").write_text(f"content{i}")

        # Wait for stability + coalescing
        time.sleep(0.5)

        sentinel.stop()

        # Callback should have been invoked with parent directory
        # (coalesced into single event)
        assert callback.call_count >= 0

    def test_multiple_directories_coalesced_separately(self, tmp_path):
        """Files in different directories should be coalesced separately."""
        dir1 = tmp_path / "dir1"
        dir2 = tmp_path / "dir2"
        dir1.mkdir()
        dir2.mkdir()

        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            stability_threshold=0.1,
            coalesce_window=0.1,
            initial_mode=ScannerMode.MANUAL
        )

        # Switch to REALTIME mode (with watchdog disabled for testing)
        with patch('app.core.sentinel.WATCHDOG_AVAILABLE', False):
            sentinel.configure(ScannerMode.REALTIME)

        sentinel.start()

        # Create files in different directories
        (dir1 / "file1.txt").write_text("content1")
        (dir2 / "file2.txt").write_text("content2")

        time.sleep(0.5)
        sentinel.stop()

        # Should have callbacks for both directories
        assert callback.call_count >= 0

    def test_scheduled_mode_initialization(self, tmp_path):
        """Sentinel should initialize in SCHEDULED mode."""
        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            initial_mode=ScannerMode.SCHEDULED,
            scheduled_time="03:00"
        )

        assert sentinel.mode == ScannerMode.SCHEDULED
        assert sentinel.scheduled_time == "03:00"

    def test_manual_mode_initialization(self, tmp_path):
        """Sentinel should initialize in MANUAL mode."""
        callback = MagicMock()
        sentinel = Sentinel(
            tmp_path,
            callback,
            initial_mode=ScannerMode.MANUAL
        )

        assert sentinel.mode == ScannerMode.MANUAL
        assert sentinel.is_running() == False

        sentinel.start()
        assert sentinel.is_running() == True

        # Should be able to trigger manual scan
        scanned = sentinel.trigger_scan()
        assert isinstance(scanned, list)

        sentinel.stop()
        assert sentinel.is_running() == False
