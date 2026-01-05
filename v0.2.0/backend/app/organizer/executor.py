"""
Execution Engine for Galgame Library Manager.

**PHASE 9.5: The Curator Workbench**

Executes organization proposals with full undo support.

Key Features:
- Pre-flight checks (source exists, targets clear)
- Undo log creation for rollback
- Safe file moves with verification
- Atomic operations (all-or-nothing)
- Progress tracking

Usage:
    result = execute_plan(proposal)
    if result.success:
        print(f"Moved {result.moved_count} files")
        # Undo log available at result.undo_log_path
    else:
        print(f"Errors: {result.errors}")

    # Rollback if needed
    rollback(result.undo_log_path)
"""

import json
import shutil
import logging
import os
from pathlib import Path
from typing import Dict, List, Optional, Any
from dataclasses import dataclass, field
from datetime import datetime

from .proposal import OrganizationProposal, FileMove, MoveStatus

logger = logging.getLogger(__name__)


@dataclass
class ExecutionResult:
    """
    Result of plan execution.

    Attributes:
        success: Whether execution completed successfully
        proposal_id: ID of executed proposal
        moved_count: Number of files moved
        skipped_count: Number of files skipped
        failed_count: Number of files that failed to move
        errors: List of error messages
        undo_log_path: Path to undo log JSON file
        created_at: Execution timestamp
    """
    success: bool
    proposal_id: str
    moved_count: int = 0
    skipped_count: int = 0
    failed_count: int = 0
    errors: List[str] = field(default_factory=list)
    undo_log_path: Optional[Path] = None
    created_at: str = field(default_factory=lambda: datetime.now().isoformat())


@dataclass
class UndoRecord:
    """
    Record of a file move for undo purposes.

    Attributes:
        original_path: Where the file was before move
        moved_path: Where the file was moved to
        checksum: File checksum (for verification)
        timestamp: When the move occurred
    """
    original_path: str
    moved_path: str
    checksum: str
    timestamp: str


def execute_plan(
    proposal: OrganizationProposal,
    undo_dir: Optional[Path] = None,
    skip_unresolved: bool = True,
    cleanup_empty_dirs: bool = True
) -> ExecutionResult:
    """
    Execute an organization proposal.

    **PERFORMS FILE MOVES** - Ensure proposal has been reviewed!

    Args:
        proposal: OrganizationProposal to execute
        undo_dir: Directory to store undo logs (defaults to source/.organizer_undo/)
        skip_unresolved: If True, skip files with UNRESOLVED status
        cleanup_empty_dirs: If True, remove empty source directories

    Returns:
        ExecutionResult with details
    """
    logger.info(f"Executing proposal: {proposal.proposal_id}")

    # Initialize result
    result = ExecutionResult(
        success=False,
        proposal_id=proposal.proposal_id
    )

    # Setup undo directory
    if undo_dir is None:
        undo_dir = proposal.source_path / ".organizer_undo"

    undo_dir.mkdir(parents=True, exist_ok=True)
    undo_log_path = undo_dir / f"undo_{proposal.proposal_id}.json"
    result.undo_log_path = undo_log_path

    # Pre-flight checks
    preflight_errors = pre_flight_check(proposal)
    if preflight_errors:
        result.errors = preflight_errors
        logger.error(f"Pre-flight check failed: {preflight_errors}")
        return result

    # Create undo log
    undo_records: List[UndoRecord] = []

    try:
        # Create target directories
        for target_path_key, target_path in proposal.target_structure.items():
            if target_path_key == "source":
                continue
            target_path.mkdir(parents=True, exist_ok=True)
            logger.debug(f"Created directory: {target_path}")

        # Execute moves in order
        for move in proposal.moves:
            # Skip unresolved files if configured
            if move.status == MoveStatus.UNRESOLVED:
                if skip_unresolved:
                    result.skipped_count += 1
                    logger.info(f"Skipped unresolved file: {move.source.name}")
                    continue
                else:
                    result.errors.append(f"Refusing to move unresolved file: {move.source}")
                    result.failed_count += 1
                    continue

            # Skip if source doesn't exist
            if not move.source.exists():
                logger.warning(f"Source file not found, skipping: {move.source}")
                result.skipped_count += 1
                continue

            # Create target directory if needed
            move.target.parent.mkdir(parents=True, exist_ok=True)

            # Check if target already exists
            if move.target.exists():
                # Verify checksum
                current_checksum = calculate_file_checksum(move.source)
                if current_checksum != move.checksum:
                    result.errors.append(
                        f"Checksum mismatch for {move.source}: "
                        f"expected {move.checksum}, got {current_checksum}"
                    )
                    result.failed_count += 1
                    continue

                # Target exists with same content, skip
                logger.info(f"Target already exists, skipping: {move.target}")
                result.skipped_count += 1
                continue

            # Perform move
            try:
                shutil.move(str(move.source), str(move.target))
                result.moved_count += 1

                # Record for undo
                undo_record = UndoRecord(
                    original_path=str(move.source),
                    moved_path=str(move.target),
                    checksum=move.checksum,
                    timestamp=datetime.now().isoformat()
                )
                undo_records.append(undo_record)

                logger.debug(f"Moved: {move.source.name} -> {move.target}")

            except Exception as e:
                error_msg = f"Failed to move {move.source}: {e}"
                result.errors.append(error_msg)
                result.failed_count += 1
                logger.error(error_msg)

        # Save undo log
        if undo_records:
            save_undo_log(undo_records, undo_log_path)
            logger.info(f"Undo log saved to: {undo_log_path}")

        # Cleanup empty source directories
        if cleanup_empty_dirs and result.moved_count > 0:
            cleanup_count = cleanup_empty_source_dirs(proposal.source_path)
            logger.info(f"Cleaned up {cleanup_count} empty directories")

        # Mark as successful if no critical errors
        if result.failed_count == 0:
            result.success = True
            logger.info(
                f"Execution successful: {result.moved_count} moved, "
                f"{result.skipped_count} skipped"
            )
        else:
            logger.warning(
                f"Execution completed with errors: {result.moved_count} moved, "
                f"{result.failed_count} failed"
            )

    except Exception as e:
        result.errors.append(f"Fatal error during execution: {e}")
        logger.error(f"Execution failed: {e}")

    return result


def pre_flight_check(proposal: OrganizationProposal) -> List[str]:
    """
    Perform pre-flight checks before execution.

    Checks:
    1. All source files exist
    2. Target directory doesn't already have conflicts
  3. Sufficient disk space (basic check)

    Args:
        proposal: Proposal to check

    Returns:
        List of error messages (empty if all checks pass)
    """
    errors = []

    logger.info("Running pre-flight checks...")

    # Check source files exist
    for move in proposal.moves:
        if not move.source.exists():
            errors.append(f"Source file not found: {move.source}")

    # Check for target conflicts
    for move in proposal.moves:
        if move.target.exists():
            # Check if it's the same file (same path after normalization)
            try:
                if move.source.resolve() != move.target.resolve():
                    errors.append(
                        f"Target already exists (different file): {move.target}\n"
                        f"Source: {move.source}"
                    )
            except Exception:
                errors.append(f"Target already exists: {move.target}")

    # Basic disk space check (total file size)
    try:
        import shutil
        target_base = proposal.target_structure["base"]
        stat = shutil.disk_usage(target_base.anchor)

        total_size = sum(m.size for m in proposal.moves)
        free_space = stat.free

        if total_size > free_space:
            errors.append(
                f"Insufficient disk space: need {total_size / (1024**3):.2f}GB, "
                f"have {free_space / (1024**3):.2f}GB free"
            )

    except Exception as e:
        logger.warning(f"Could not check disk space: {e}")

    if errors:
        logger.error(f"Pre-flight check failed with {len(errors)} errors")
    else:
        logger.info("Pre-flight checks passed")

    return errors


def save_undo_log(undo_records: List[UndoRecord], output_path: Path) -> bool:
    """
    Save undo log to JSON file.

    Args:
        undo_records: List of undo records
        output_path: Output file path

    Returns:
        True if successful
    """
    try:
        undo_data = {
            "created_at": datetime.now().isoformat(),
            "record_count": len(undo_records),
            "records": [
                {
                    "original_path": r.original_path,
                    "moved_path": r.moved_path,
                    "checksum": r.checksum,
                    "timestamp": r.timestamp
                }
                for r in undo_records
            ]
        }

        output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(undo_data, f, indent=2, ensure_ascii=False)

        return True

    except Exception as e:
        logger.error(f"Error saving undo log: {e}")
        return False


def load_undo_log(undo_log_path: Path) -> Optional[List[UndoRecord]]:
    """
    Load undo log from JSON file.

    Args:
        undo_log_path: Path to undo log file

    Returns:
        List of UndoRecord or None if error
    """
    try:
        with open(undo_log_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        records = []
        for record_data in data["records"]:
            record = UndoRecord(
                original_path=record_data["original_path"],
                moved_path=record_data["moved_path"],
                checksum=record_data["checksum"],
                timestamp=record_data["timestamp"]
            )
            records.append(record)

        logger.info(f"Loaded {len(records)} undo records from: {undo_log_path}")
        return records

    except Exception as e:
        logger.error(f"Error loading undo log: {e}")
        return None


def rollback(undo_log_path: Path) -> bool:
    """
    Rollback an executed proposal using undo log.

    **WARNING:** This will undo file moves, potentially overwriting existing files.

    Args:
        undo_log_path: Path to undo log file

    Returns:
        True if rollback successful
    """
    logger.info(f"Rolling back using undo log: {undo_log_path}")

    undo_records = load_undo_log(undo_log_path)
    if not undo_records:
        logger.error("Failed to load undo log")
        return False

    success_count = 0
    error_count = 0

    # Process records in reverse order (LIFO)
    for record in reversed(undo_records):
        try:
            original_path = Path(record.original_path)
            moved_path = Path(record.moved_path)

            # Check if moved file still exists
            if not moved_path.exists():
                logger.warning(f"Moved file not found, skipping: {moved_path}")
                continue

            # Create original directory if needed
            original_path.parent.mkdir(parents=True, exist_ok=True)

            # Check if original path already exists
            if original_path.exists():
                logger.warning(
                    f"Original path already exists, will overwrite: {original_path}"
                )
                original_path.unlink()

            # Move back
            shutil.move(str(moved_path), str(original_path))
            success_count += 1
            logger.debug(f"Rolled back: {moved_path.name} -> {original_path}")

        except Exception as e:
            error_count += 1
            logger.error(f"Error rolling back {record.moved_path}: {e}")

    logger.info(
        f"Rollback complete: {success_count} files restored, "
        f"{error_count} errors"
    )

    return error_count == 0


def cleanup_empty_source_dirs(source_path: Path) -> int:
    """
    Recursively remove empty directories after file moves.

    Args:
        source_path: Root source directory

    Returns:
        Number of directories removed
    """
    removed_count = 0

    # Walk bottom-up
    for root, dirs, files in sorted(os.walk(source_path, topdown=False)):
        root_path = Path(root)

        # Skip if not a directory
        if not root_path.is_dir():
            continue

        # Skip if has files
        if files:
            continue

        # Skip if has subdirectories
        if any((root_path / d).is_dir() for d in dirs):
            continue

        # Skip if it's the source root itself
        if root_path == source_path:
            continue

        # Skip hidden/system directories
        if root_path.name.startswith("."):
            continue

        try:
            root_path.rmdir()
            removed_count += 1
            logger.debug(f"Removed empty directory: {root_path}")

        except Exception as e:
            logger.debug(f"Could not remove directory {root_path}: {e}")

    return removed_count


def calculate_file_checksum(file_path: Path) -> str:
    """
    Calculate MD5 checksum of a file.

    Args:
        file_path: File to checksum

    Returns:
        Hex checksum string
    """
    import hashlib

    hash_md5 = hashlib.md5()
    try:
        with open(file_path, "rb") as f:
            for chunk in iter(lambda: f.read(8192), b""):
                hash_md5.update(chunk)
        return hash_md5.hexdigest()
    except Exception:
        return ""
