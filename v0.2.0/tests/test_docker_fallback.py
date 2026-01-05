"""
Test Docker/Container Fallback for PollingWatcher

This test proves that when watchdog is unavailable (e.g., in Docker containers),
the Sentinel automatically falls back to PollingWatcher.
"""

import sys
import tempfile
from pathlib import Path
from unittest.mock import patch, Mock
import pytest
import time

# Add backend to path
sys.path.insert(0, str(Path(__file__).parent.parent / "backend"))

from backend.app.core import Sentinel, ScannerMode


class TestDockerFallback:
    """Test PollingWatcher fallback when watchdog is unavailable."""

    def test_polling_watcher_used_when_watchdog_unavailable(self):
        """
        Test that PollingWatcher is used when watchdog import fails.

        This simulates a Docker environment where watchdog is not installed.
        """
        with patch('backend.app.core.sentinel.WATCHDOG_AVAILABLE', False):
            from backend.app.core import Sentinel as SentinelReloaded

            scan_results = []

            def callback(directories):
                scan_results.extend(directories)

            # Create a temporary library
            import tempfile
            with tempfile.TemporaryDirectory() as tmpdir:
                library_root = Path(tmpdir)

                # Create a test file
                test_file = library_root / "test_game.exe"
                test_file.write_text("test")

                # Initialize Sentinel in REALTIME mode
                sentinel = SentinelReloaded(
                    library_root=library_root,
                    callback=callback,
                    initial_mode=ScannerMode.REALTIME
                )

                # Start the sentinel
                sentinel.start()

                # Verify polling watcher was initialized
                # (observer should be None, polling_watcher should be set)
                assert sentinel._observer is None, "Observer should be None when watchdog unavailable"
                assert sentinel._polling_watcher is not None, "PollingWatcher should be initialized"

                # Verify the polling watcher thread is running
                assert sentinel._polling_watcher._thread is not None, "Polling thread should be running"
                assert sentinel._polling_watcher._thread.is_alive(), "Polling thread should be alive"

                # Stop the sentinel
                sentinel.stop()

    def test_polling_watcher_fallback_on_observer_failure(self):
        """
        Test that PollingWatcher is used when watchdog Observer.start() fails.

        This simulates an environment where watchdog is installed but fails to start
        (e.g., Docker container with limited inotify capabilities).
        """
        with patch('backend.app.core.sentinel.WATCHDOG_AVAILABLE', True):
            from backend.app.core import Sentinel as SentinelReloaded

            scan_results = []

            def callback(directories):
                scan_results.extend(directories)

            with tempfile.TemporaryDirectory() as tmpdir:
                library_root = Path(tmpdir)
                test_file = library_root / "test.exe"
                test_file.write_text("test")

                # Create a mock Observer that fails on start
                mock_observer = Mock()
                mock_observer.start.side_effect = Exception("inotify limit reached")

                # Need to patch Observer class AND also ensure the module imports work
                with patch('backend.app.core.sentinel.Observer') as MockObserver:
                    MockObserver.return_value = mock_observer

                    sentinel = SentinelReloaded(
                        library_root=library_root,
                        callback=callback,
                        initial_mode=ScannerMode.REALTIME
                    )

                    # Start should fall back to PollingWatcher
                    sentinel.start()

                    # Verify polling watcher was used
                    assert sentinel._polling_watcher is not None, "Should fall back to PollingWatcher"

                    # Verify observer start was attempted but failed
                    assert mock_observer.start.called, "Observer start should have been attempted"

                    sentinel.stop()

    def test_polling_watcher_detects_file_changes(self):
        """
        Test that PollingWatcher actually detects file changes.

        This proves PollingWatcher is a functional fallback, not a stub.
        """
        with patch('backend.app.core.sentinel.WATCHDOG_AVAILABLE', False):
            from backend.app.core import Sentinel as SentinelReloaded

            scan_results = []

            def callback(directories):
                scan_results.extend(directories)

            with tempfile.TemporaryDirectory() as tmpdir:
                library_root = Path(tmpdir)

                # Use a short poll interval for testing
                sentinel = SentinelReloaded(
                    library_root=library_root,
                    callback=callback,
                    poll_interval=1.0,  # 1 second for fast testing
                    initial_mode=ScannerMode.REALTIME
                )

                sentinel.start()

                # Wait a bit for initial scan
                time.sleep(1.5)

                # Create a new file
                new_file = library_root / "new_game.exe"
                new_file.write_text("test content")

                # Wait for polling to detect the change
                time.sleep(2)

                # Verify the polling watcher detected the file
                # The file should be in the known files
                assert str(new_file) in sentinel._polling_watcher._known_files

                sentinel.stop()

    def test_sentinel_logs_fallback_to_polling(self):
        """
        Test that Sentinel logs a warning when falling back to polling.

        This ensures administrators are aware of the fallback.
        """
        with patch('backend.app.core.sentinel.WATCHDOG_AVAILABLE', True):
            from backend.app.core import Sentinel as SentinelReloaded
            import logging

            # Capture log messages
            with patch('backend.app.core.sentinel.logger') as mock_logger:
                scan_results = []

                def callback(directories):
                    scan_results.extend(directories)

                with tempfile.TemporaryDirectory() as tmpdir:
                    library_root = Path(tmpdir)

                    # Mock Observer to fail
                    mock_observer = Mock()
                    mock_observer.start.side_effect = Exception("Failed to start observer")

                    with patch('backend.app.core.sentinel.Observer', return_value=mock_observer):
                        sentinel = SentinelReloaded(
                            library_root=library_root,
                            callback=callback,
                            initial_mode=ScannerMode.REALTIME
                        )

                        sentinel.start()

                        # Verify warning was logged
                        mock_logger.warning.assert_called()
                        warning_calls = [str(call) for call in mock_logger.warning.call_args_list]
                        assert any("Failed to start watchdog" in str(call) for call in warning_calls)
                        assert any("Falling back to polling" in str(call) for call in warning_calls)

                        sentinel.stop()

    def test_watchdog_available_constant(self):
        """
        Test that WATCHDOG_AVAILABLE is set correctly based on import success.

        This verifies the module-level check works.
        """
        from backend.app.core import sentinel

        # WATCHDOG_AVAILABLE should be a boolean
        assert isinstance(sentinel.WATCHDOG_AVAILABLE, bool)

        # Verify it matches whether Observer is importable
        if sentinel.WATCHDOG_AVAILABLE:
            # If available, Observer should be the watchdog class
            assert sentinel.Observer is not None
        else:
            # If not available, Observer should be None
            assert sentinel.Observer is None


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
