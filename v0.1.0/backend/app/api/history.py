"""
History API endpoints for Galgame Library Manager.

**PHASE 19.6: Time Machine (Transaction History)**

Provides REST API endpoints for managing transaction history:
- List all transaction history from journal
- Undo specific transactions (rollback)
"""

import logging
from typing import List

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel, Field

from ..core import JournalManager, Transaction
from ..models.journal import JournalEntry

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/history", tags=["history"])


# ============================================================================
# Pydantic Models
# ============================================================================

class HistoryEntry(BaseModel):
    """Represents a journal entry."""
    tx_id: str = Field(..., description="Transaction ID")
    op: str = Field(..., description="Operation: rename, mkdir, copy, delete")
    src: str = Field(..., description="Source path")
    dest: str | None = Field(None, description="Destination path (for rename/copy) or trash path (for delete)")
    state: str = Field(..., description="Transaction state: prepared, committed, failed, rolled_back")
    timestamp: float = Field(..., description="Unix timestamp when transaction was created")
    timeout_at: float = Field(..., description="Unix timestamp when transaction becomes stale")


class HistoryResponse(BaseModel):
    """Response model for transaction history."""
    entries: List[HistoryEntry]
    total_count: int


class UndoResponse(BaseModel):
    """Response model for undo operation."""
    success: bool
    tx_id: str
    message: str
    state: str


# ============================================================================
# API Endpoints
# ============================================================================

@router.get("", response_model=HistoryResponse)
async def get_history(limit: int = 100):
    """
    Get transaction history from the journal.

    Returns all journal entries sorted by timestamp (newest first).

    Args:
        limit: Maximum number of entries to return (default: 100)

    Returns:
        HistoryResponse with list of all entries and total count

    Example:
        GET /api/history?limit=50
    """
    try:
        from ..config import get_config

        config = get_config()
        journal = JournalManager(config.config_dir)

        # Read all entries
        all_entries = journal.read_all()

        # Sort by timestamp descending (newest first)
        all_entries.sort(key=lambda e: e.timestamp, reverse=True)

        # Apply limit
        entries = all_entries[:limit]

        return HistoryResponse(
            entries=[
                HistoryEntry(
                    tx_id=e.tx_id,
                    op=e.op,
                    src=e.src,
                    dest=e.dest,
                    state=e.state,
                    timestamp=e.timestamp,
                    timeout_at=e.timeout_at
                )
                for e in entries
            ],
            total_count=len(all_entries)
        )

    except Exception as e:
        logger.error(f"Error reading journal: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to read history: {str(e)}"
        )


@router.post("/{tx_id}/undo", response_model=UndoResponse)
async def undo_transaction(tx_id: str):
    """
    Undo (rollback) a specific transaction.

    This reverses the operation performed by the transaction.
    Only transactions that were committed can be rolled back.

    Operations that can be undone:
    - rename: Move file back to original location
    - mkdir: Delete the created directory
    - copy: Delete the copied file/directory
    - delete: Restore from trash

    Args:
        tx_id: Transaction ID to undo

    Returns:
        UndoResponse with result status

    Example:
        POST /api/history/tx_1234567890/undo
    """
    try:
        from ..config import get_config

        config = get_config()
        journal = JournalManager(config.config_dir)
        library_root = config.get_paths()["library_root"]

        # Find the transaction in journal
        all_entries = journal.read_all()
        target_entry = None

        for entry in all_entries:
            if entry.tx_id == tx_id:
                target_entry = entry
                break

        if not target_entry:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Transaction not found: {tx_id}"
            )

        # Check if transaction can be rolled back
        if target_entry.state == "rolled_back":
            return UndoResponse(
                success=False,
                tx_id=tx_id,
                message="Transaction is already rolled back",
                state=target_entry.state
            )

        if target_entry.state == "prepared":
            return UndoResponse(
                success=False,
                tx_id=tx_id,
                message="Cannot rollback prepared transaction (use recovery instead)",
                state=target_entry.state
            )

        if target_entry.state != "committed":
            return UndoResponse(
                success=False,
                tx_id=tx_id,
                message=f"Cannot rollback transaction in state: {target_entry.state}",
                state=target_entry.state
            )

        # Create transaction for rollback
        tx = Transaction(journal, library_root)
        tx.entry = target_entry

        # Execute rollback
        tx.rollback()

        logger.info(f"Successfully rolled back transaction {tx_id}")

        return UndoResponse(
            success=True,
            tx_id=tx_id,
            message=f"Successfully undone {target_entry.op} operation",
            state="rolled_back"
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error undoing transaction {tx_id}: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to undo transaction: {str(e)}"
        )
