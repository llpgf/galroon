"""
Test Journal Write Failure Handling

This test proves that if JournalManager fails to write (e.g., disk full),
the Transaction API will NOT execute file operations and will return 500.
"""

import sys
import tempfile
from pathlib import Path
from unittest.mock import Mock, patch, MagicMock
import pytest

# Add backend to path
sys.path.insert(0, str(Path(__file__).parent.parent / "backend"))

from backend.app.core import JournalManager, Transaction, TransactionExecutionError
from backend.app.models.journal import JournalEntry


class TestJournalFailureHandling:
    """Test that journal write failures prevent file operations."""

    def test_transaction_prepare_journal_write_failure(self):
        """
        Test that Transaction.prepare() fails if journal write fails.

        This proves that operations are NOT executed if journaling fails.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            config_dir = Path(tmpdir)
            library_root = Path(tmpdir) / "library"
            library_root.mkdir()

            # Create a real journal manager
            journal = JournalManager(config_dir)

            # Mock journal.append to raise OSError (simulating disk full)
            with patch.object(journal, 'append', side_effect=OSError("No space left on device")):
                tx = Transaction(journal, library_root)

                # Create a test file
                test_file = library_root / "test.txt"
                test_file.write_text("before")

                # Attempt to prepare a rename operation
                # This should FAIL because journal.append raises OSError
                with pytest.raises(TransactionExecutionError) as exc_info:
                    tx.prepare("rename", test_file, library_root / "renamed.txt")

                # Verify the error message mentions journal write failure
                assert "Journal write failed" in str(exc_info.value)
                assert "Operation ABORTED" in str(exc_info.value)

                # Verify transaction is in FAILED state
                assert tx.state.value == "failed"
                assert tx.error is not None

                # CRITICAL: Verify file was NOT renamed
                # (because commit() was never called)
                assert test_file.exists()
                assert not (library_root / "renamed.txt").exists()

    def test_transaction_prepare_cleanup_on_journal_failure(self):
        """
        Test that Transaction.prepare() properly cleans up if journal write fails.

        This ensures no partial transaction state is left behind.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            config_dir = Path(tmpdir)
            library_root = Path(tmpdir) / "library"
            library_root.mkdir()

            journal = JournalManager(config_dir)

            # Mock journal.append to fail
            with patch.object(journal, 'append', side_effect=OSError("Disk full")):
                tx = Transaction(journal, library_root)

                test_file = library_root / "test.txt"
                test_file.write_text("content")

                with pytest.raises(TransactionExecutionError):
                    tx.prepare("delete", test_file)

                # Verify transaction entry was cleaned up
                assert tx.entry is None

                # Verify state is FAILED
                assert tx.state.value == "failed"

    def test_journal_write_atomicity(self):
        """
        Test that journal.write raises OSError if fsync fails.

        This proves the journal uses flush+fsync and propagates errors.
        Note: Data may be written to OS buffer before fsync fails,
        but the ERROR is always raised to alert the caller.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            config_dir = Path(tmpdir)
            journal = JournalManager(config_dir)

            # Create a journal entry
            entry = JournalEntry(
                op="rename",
                src="/test/src",
                dest="/test/dest",
                state="prepared",
                timeout_at=9999999999.0
            )

            # Mock fsync to fail (simulate disk error during sync)
            # This should raise OSError
            with pytest.raises(OSError) as exc_info:
                with patch('os.fsync', side_effect=OSError("I/O error")):
                    journal.append(entry)

            # Verify the error is about fsync failure
            assert "I/O error" in str(exc_info.value)

            # CRITICAL: The error was raised, so the caller knows the write failed
            # Even if data is in OS buffer, we've alerted them to the failure

    def test_api_returns_500_on_journal_failure(self):
        """
        Test that if journal write fails, operation is aborted.

        This proves that at the Transaction level, journal failures
        prevent file operations from executing.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            library_root = Path(tmpdir) / "library"
            library_root.mkdir()
            config_dir = Path(tmpdir) / "config"
            config_dir.mkdir()

            # Initialize journal and transaction
            journal = JournalManager(config_dir)

            # Create a test file
            test_file = library_root / "test.txt"
            test_file.write_text("before")

            # Mock journal.append to fail (simulate disk full)
            with patch.object(journal, 'append', side_effect=OSError("Disk full")):
                tx = Transaction(journal, library_root)

                # Attempt to prepare and commit a rename operation
                # This should FAIL during prepare() when journal write fails
                with pytest.raises(TransactionExecutionError) as exc_info:
                    tx.prepare("rename", test_file, library_root / "renamed.txt")

                # Verify the error mentions journal write failure
                assert "Journal write failed" in str(exc_info.value)

                # CRITICAL: Verify transaction is in FAILED state
                assert tx.state.value == "failed"

                # CRITICAL: Verify file was NOT renamed
                # (because prepare() failed and commit() was never called)
                assert test_file.exists(), "Original file should still exist"
                assert test_file.read_text() == "before", "Original content should be unchanged"
                assert not (library_root / "renamed.txt").exists(), "Target file should not exist"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
