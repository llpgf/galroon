"""
Journal Manager - Atomic Journaling & Recovery System

This module implements the append-only journal for file system operations.
All writes are atomic (flush + fsync) to prevent corruption on crash.
"""

import logging
import os
from pathlib import Path
from typing import Callable, Optional

from .path_safety import is_safe_config_dir
from ..models.journal import JournalEntry

logger = logging.getLogger(__name__)


class JournalManager:
    """
    Manages the append-only operation journal.

    The journal records all file system operations in journal.jsonl.
    Writes are atomic (flush + fsync) to ensure durability.

    On startup, the journal is scanned for incomplete transactions
    that need rollback or recovery.
    """

    def __init__(self, config_dir: Path, transaction_timeout_seconds: float = 300):
        """
        Initialize the journal manager.

        Args:
            config_dir: Directory where journal.jsonl will be stored
            transaction_timeout_seconds: How long before a transaction is considered stale

        Raises:
            RuntimeError: If config_dir is not safe for journal storage
        """
        self.config_dir = Path(config_dir)
        self.transaction_timeout = transaction_timeout_seconds
        self.journal_path = self.config_dir / "journal.jsonl"

        # Enforce journal sandbox: refuse to start if config dir is unsafe
        if not is_safe_config_dir(self.config_dir):
            raise RuntimeError(
                f"Journal sandbox violation: {self.config_dir} is not safe. "
                "Refusing to start to prevent data hijacking."
            )

        # Ensure journal file exists
        if not self.journal_path.exists():
            self.journal_path.touch()

        logger.info(f"Journal initialized at {self.journal_path}")

    def _atomic_write(self, line: str) -> None:
        """
        Write a single line to the journal atomically.

        Uses flush + fsync to ensure the write is physically on disk
        before returning. This prevents journal corruption on crashes.

        Args:
            line: The line to write (should include newline)

        Raises:
            OSError: If write or fsync fails
        """
        # Open in append mode
        with open(self.journal_path, "a", encoding="utf-8") as f:
            # Write the line
            f.write(line)

            # Flush to OS buffer
            f.flush()

            # Force write to physical disk (critical for durability)
            # Try multiple methods for cross-platform compatibility
            try:
                # Try Python 3.12+ sync() method first
                try:
                    f.sync()
                except AttributeError:
                    # Fall back to os.fsync for all platforms
                    os.fsync(f.fileno())
            except OSError as e:
                # Log error but re-raise - atomicity is critical
                logger.error(f"fsync failed: {e}")
                raise

    def append(self, entry: JournalEntry) -> None:
        """
        Append a new journal entry with atomic write.

        Args:
            entry: The JournalEntry to append

        Raises:
            OSError: If atomic write fails
        """
        line = entry.to_journal_line()
        self._atomic_write(line)
        logger.debug(f"Journal entry appended: {entry.tx_id} ({entry.op})")

    # Phase 19.5: Simple event logging for TimeMachineLog
    def log_event(self, action: str, target: str, status: str = "completed") -> None:
        """
        Log a simple event to the journal.

        Phase 19.5: Convenience method for logging high-level actions
        like "Trash Emptied" or "Metadata Applied".

        Args:
            action: The action performed (e.g., "trash_emptied", "metadata_applied")
            target: The target of the action (e.g., "library", "game/v12345")
            status: The status of the action (default: "completed")

        Example:
            journal.log_event("trash_emptied", "library", "completed")
            journal.log_event("metadata_applied", "game/v12345", "completed")
        """
        import uuid
        from datetime import datetime, timezone

        entry = JournalEntry(
            tx_id=str(uuid.uuid4()),
            timestamp=datetime.now(timezone.utc).isoformat(),
            op="event",  # Special operation type for simple events
            target=target,
            action=action,
            status=status,
            details=f"Action: {action}, Target: {target}, Status: {status}"
        )

        self.append(entry)
        logger.info(f"Event logged: {action} on {target} - {status}")

    def read_all(self) -> list[JournalEntry]:
        """
        Read all entries from the journal.

        Returns:
            List of all JournalEntry objects in the journal
        """
        entries = []
        if not self.journal_path.exists():
            return entries

        with open(self.journal_path, "r", encoding="utf-8") as f:
            for line_num, line in enumerate(f, 1):
                line = line.strip()
                if not line:
                    continue
                try:
                    entry = JournalEntry.model_validate_json(line)
                    entries.append(entry)
                except Exception as e:
                    logger.error(f"Invalid journal entry at line {line_num}: {e}")

        return entries

    def get_incomplete_transactions(self) -> list[JournalEntry]:
        """
        Find all transactions that are in 'prepared' state.

        These represent incomplete operations that may need recovery.

        Returns:
            List of JournalEntry objects with state='prepared'
        """
        all_entries = self.read_all()
        return [e for e in all_entries if e.state == "prepared"]

    def get_stale_transactions(self) -> list[JournalEntry]:
        """
        Find all stale prepared transactions.

        A transaction is stale if it's still in 'prepared' state
        past its timeout.

        Returns:
            List of stale JournalEntry objects
        """
        incomplete = self.get_incomplete_transactions()
        return [e for e in incomplete if e.is_stale()]

    def recover(
        self,
        rollback_handler: Optional[Callable[[JournalEntry], bool]] = None,
        current_time: float | None = None
    ) -> dict[str, list[JournalEntry]]:
        """
        Scan journal and recover incomplete transactions.

        This is called on startup to find any prepared transactions
        that were never committed or rolled back (e.g., due to a crash).

        Args:
            rollback_handler: Optional callback function to execute rollback.
                              If provided, will be called for each stale transaction.
                              Should return True on success, False on failure.
            current_time: Optional current time for staleness testing

        Returns:
            Dictionary with keys:
                - 'stale': List of stale transactions (timeout exceeded)
                - 'active': List of still-valid prepared transactions
        """
        stale = self.get_stale_transactions()
        incomplete = self.get_incomplete_transactions()
        active = [e for e in incomplete if not e.is_stale(current_time)]

        result = {
            'stale': stale,
            'active': active
        }

        if stale:
            logger.warning(f"Found {len(stale)} stale transaction(s) requiring recovery")

            # Attempt rollback if handler provided
            if rollback_handler:
                for entry in stale:
                    try:
                        success = rollback_handler(entry)
                        if success:
                            logger.info(f"Successfully rolled back transaction {entry.tx_id}")
                        else:
                            logger.error(f"Failed to rollback transaction {entry.tx_id}")
                    except Exception as e:
                        logger.error(f"Error rolling back {entry.tx_id}: {e}")
            else:
                logger.warning("No rollback handler provided - transactions not auto-rolled back")

        if active:
            logger.info(f"Found {len(active)} active prepared transaction(s)")

        return result
