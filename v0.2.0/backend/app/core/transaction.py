"""
Transaction Engine - FSM-Based File Operations

This module implements transaction-based file operations with strict
Finite State Machine (FSM) control: Prepared -> Committed | Failed -> Rolled Back

Every operation is journaled before execution, enabling crash recovery
and atomic rollback capabilities.
"""

import logging
import os
import shutil
import time
from datetime import datetime, timedelta
from enum import Enum
from pathlib import Path
from typing import Literal, Optional

from .journal import JournalManager
from .path_safety import is_safe_path, validate_path_or_raise
from .trash import SmartTrashManager
from ..models.journal import JournalEntry

logger = logging.getLogger(__name__)


class TransactionState(Enum):
    """
    Transaction states following strict FSM.
    Valid transitions: PREPARED -> COMMITTED | FAILED -> ROLLED_BACK
    """
    PREPARED = "prepared"
    COMMITTED = "committed"
    FAILED = "failed"
    ROLLED_BACK = "rolled_back"


class TransactionError(Exception):
    """Base exception for transaction errors."""
    pass


class TransactionValidationError(TransactionError):
    """Raised when path validation fails."""
    pass


class TransactionExecutionError(TransactionError):
    """Raised when file operation execution fails."""
    pass


class Transaction:
    """
    A transaction-based file operation with FSM control and rollback capability.

    Each transaction goes through:
    1. Prepare: Validate paths, create journal entry
    2. Commit: Execute the operation
    3. Rollback (if needed): Reverse the operation

    All paths are validated against the library root before execution.
    """

    def __init__(self, journal: JournalManager, library_root: Path):
        """
        Initialize a new transaction.

        Args:
            journal: JournalManager instance for persistence
            library_root: Root directory for path validation
        """
        self.journal = journal
        self.library_root = Path(library_root)
        self.entry: Optional[JournalEntry] = None
        self.state = TransactionState.PREPARED
        self.error: Optional[Exception] = None

        # Smart trash manager for configurable and safe trash handling
        self.trash_manager = SmartTrashManager(journal.config_dir)
        self.trash_dir = self.trash_manager.trash_dir

    def _validate_path(self, path: Path) -> Path:
        """
        Validate a path is safe within the library root.

        Args:
            path: Path to validate

        Returns:
            Resolved safe path

        Raises:
            TransactionValidationError: If path is not safe
        """
        try:
            return validate_path_or_raise(path, self.library_root)
        except ValueError as e:
            raise TransactionValidationError(f"Path validation failed: {e}")

    def _validate_paths(self, src: Path, dest: Optional[Path] = None) -> tuple[Path, Optional[Path]]:
        """
        Validate source and destination paths.

        Args:
            src: Source path
            dest: Optional destination path

        Returns:
            Tuple of validated paths

        Raises:
            TransactionValidationError: If any path is not safe
        """
        validated_src = self._validate_path(src)
        validated_dest = self._validate_path(dest) if dest else None
        return validated_src, validated_dest

    def _create_journal_entry(
        self,
        op: Literal["rename", "mkdir", "copy", "delete"],
        src: Path,
        dest: Optional[Path] = None
    ) -> JournalEntry:
        """
        Create a journal entry for this transaction.

        Args:
            op: Operation type
            src: Source path
            dest: Optional destination path

        Returns:
            JournalEntry instance
        """
        timeout_at = time.time() + self.journal.transaction_timeout

        return JournalEntry(
            op=op,
            src=str(src),
            dest=str(dest) if dest else None,
            state=TransactionState.PREPARED.value,
            timeout_at=timeout_at
        )

    def _execute_file_operation(
        self,
        op: Literal["rename", "mkdir", "copy", "delete"],
        src: Path,
        dest: Optional[Path] = None
    ) -> None:
        """
        Execute the actual file operation using shutil.

        Args:
            op: Operation type
            src: Source path
            dest: Destination path (for rename, copy)

        Raises:
            TransactionExecutionError: If operation fails
        """
        try:
            if op == "rename":
                if dest is None:
                    raise TransactionExecutionError("Rename requires destination")
                shutil.move(str(src), str(dest))

            elif op == "mkdir":
                src.mkdir(parents=True, exist_ok=False)

            elif op == "copy":
                if dest is None:
                    raise TransactionExecutionError("Copy requires destination")
                if src.is_dir():
                    shutil.copytree(str(src), str(dest))
                else:
                    shutil.copy2(str(src), str(dest))

            elif op == "delete":
                # SAFE DELETE: Move to trash instead of permanent deletion
                # Ensure we have enough headroom before moving to trash
                self.trash_manager.ensure_headroom()

                # Create trash subdirectory for this transaction
                tx_trash_dir = self.trash_dir / self.entry.tx_id
                tx_trash_dir.mkdir(parents=True, exist_ok=True)

                # Move to trash (preserve original structure)
                trash_path = tx_trash_dir / src.name

                # Move the file/directory to trash
                shutil.move(str(src), str(trash_path))

                # Store the trash path in the entry's dest field for rollback
                # (We repurpose dest field to store trash location)
                self.entry.dest = str(trash_path)

            logger.info(f"Executed {op}: {src} -> {dest}")

        except OSError as e:
            raise TransactionExecutionError(f"File operation failed: {e}")

    def _rollback_file_operation(
        self,
        op: Literal["rename", "mkdir", "copy", "delete"],
        src: Path,
        dest: Optional[Path] = None
    ) -> None:
        """
        Rollback a file operation by reversing it.

        Rollback strategies:
        - rename: Rename dest back to src
        - mkdir: Remove the created directory
        - copy: Delete the copied files/directories
        - delete: Cannot rollback (file is gone) - log warning

        Args:
            op: Operation type to rollback
            src: Original source path
            dest: Original destination path

        Raises:
            TransactionExecutionError: If rollback fails
        """
        try:
            if op == "rename":
                # Rollback: Rename dest back to src
                if dest and dest.exists():
                    shutil.move(str(dest), str(src))
                    logger.info(f"Rolled back rename: {dest} -> {src}")

            elif op == "mkdir":
                # Rollback: Remove the created directory
                if src.exists():
                    shutil.rmtree(str(src))
                    logger.info(f"Rolled back mkdir: removed {src}")

            elif op == "copy":
                # Rollback: Delete the copied files/directories
                if dest and dest.exists():
                    if dest.is_dir():
                        shutil.rmtree(str(dest))
                    else:
                        dest.unlink()
                    logger.info(f"Rolled back copy: removed {dest}")

            elif op == "delete":
                # Rollback: Restore from trash
                # The trash path is stored in entry.dest
                if dest and dest.exists():
                    # Restore from trash to original location
                    shutil.move(str(dest), str(src))
                    logger.info(f"Rolled back delete: restored {src} from trash")

                    # Clean up empty trash directory
                    try:
                        trash_parent = dest.parent
                        if trash_parent.is_dir() and not any(trash_parent.iterdir()):
                            trash_parent.rmdir()
                    except OSError:
                        pass  # Trash cleanup is best-effort

        except OSError as e:
            raise TransactionExecutionError(f"Rollback failed: {e}")

    def prepare(
        self,
        op: Literal["rename", "mkdir", "copy", "delete"],
        src: Path,
        dest: Optional[Path] = None
    ) -> "Transaction":
        """
        Prepare a transaction: validate paths and create journal entry.

        Args:
            op: Operation type
            src: Source path
            dest: Destination path (required for rename, copy)

        Returns:
            Self for method chaining

        Raises:
            TransactionValidationError: If paths are invalid
            TransactionError: If state is invalid or already prepared
        """
        # Check if transaction has already been prepared
        if self.entry is not None:
            raise TransactionError("Cannot prepare: transaction already prepared")

        if self.state != TransactionState.PREPARED:
            raise TransactionError(f"Cannot prepare: transaction already {self.state.value}")

        # Validate all paths first
        validated_src, validated_dest = self._validate_paths(src, dest)

        # Additional validation: dest must not exist for mkdir/copy/rename
        if op == "mkdir" and validated_src.exists():
            raise TransactionValidationError(f"Directory already exists: {validated_src}")

        if op in ["copy", "rename"] and validated_dest and validated_dest.exists():
            raise TransactionValidationError(f"Destination already exists: {validated_dest}")

        if op == "delete" and not validated_src.exists():
            raise TransactionValidationError(f"Source does not exist: {validated_src}")

        # Create journal entry
        self.entry = self._create_journal_entry(op, validated_src, validated_dest)

        # Write to journal (atomic) - CRITICAL: If this fails, transaction is NOT prepared
        try:
            self.journal.append(self.entry)
        except OSError as e:
            # Journal write failed - transaction is NOT valid
            self.entry = None
            self.state = TransactionState.FAILED
            self.error = e
            raise TransactionExecutionError(
                f"CRITICAL: Journal write failed for transaction {self.entry.tx_id if self.entry else 'unknown'}. "
                f"Operation ABORTED to prevent inconsistency. Error: {e}"
            )

        logger.info(f"Prepared {op} transaction: {self.entry.tx_id}")
        return self

    def commit(self) -> "Transaction":
        """
        Commit the transaction: execute the file operation.

        Returns:
            Self for method chaining

        Raises:
            TransactionError: If state is invalid
            TransactionExecutionError: If operation fails
        """
        if self.state != TransactionState.PREPARED:
            raise TransactionError(f"Cannot commit: transaction not prepared (state={self.state.value})")

        if not self.entry:
            raise TransactionError("No entry to commit")

        try:
            # Execute the file operation
            self._execute_file_operation(
                self.entry.op,
                Path(self.entry.src),
                Path(self.entry.dest) if self.entry.dest else None
            )

            # Update journal entry to committed
            self.entry.state = TransactionState.COMMITTED.value
            self.journal.append(self.entry)
            self.state = TransactionState.COMMITTED

            logger.info(f"Committed transaction: {self.entry.tx_id}")
            return self

        except Exception as e:
            # Mark as failed
            self.state = TransactionState.FAILED
            self.error = e
            logger.error(f"Transaction failed: {self.entry.tx_id} - {e}")
            raise

    def rollback(self) -> "Transaction":
        """
        Rollback the transaction: reverse the operation.

        Returns:
            Self for method chaining

        Raises:
            TransactionError: If rollback fails
        """
        if not self.entry:
            raise TransactionError("No entry to rollback")

        try:
            # Execute rollback
            self._rollback_file_operation(
                self.entry.op,
                Path(self.entry.src),
                Path(self.entry.dest) if self.entry.dest else None
            )

            # Update journal entry to rolled_back
            self.entry.state = TransactionState.ROLLED_BACK.value
            self.journal.append(self.entry)
            self.state = TransactionState.ROLLED_BACK

            logger.info(f"Rolled back transaction: {self.entry.tx_id}")
            return self

        except Exception as e:
            logger.error(f"Rollback failed for {self.entry.tx_id}: {e}")
            self.error = e
            raise TransactionError(f"Rollback failed: {e}")

    def get_state(self) -> TransactionState:
        """
        Get the current transaction state.

        Returns:
            Current TransactionState
        """
        return self.state

    @staticmethod
    def cleanup_old_trash(config_dir: Path, days_old: int = 30) -> int:
        """
        Clean up trash entries older than specified days.

        This is a maintenance task that should be called periodically
        to permanently delete old trash. Uses SmartTrashManager for cleanup.

        Args:
            config_dir: Configuration directory containing .trash folder
            days_old: Delete trash older than this many days (default: 30)

        Returns:
            Number of trash items deleted
        """
        trash_manager = SmartTrashManager(config_dir)
        return trash_manager.cleanup_by_retention()
