"""
Journal Data Models.

Defines the structure for transaction journal entries.
All journal entries are persisted to journal.jsonl in the config directory.
"""

from datetime import datetime
from typing import Literal
from pydantic import BaseModel, Field
from uuid import uuid4


class JournalEntry(BaseModel):
    """
    A single journal entry representing a file system operation.

    The journal records all operations in an append-only file (journal.jsonl).
    Each operation goes through states: prepared -> committed | rolled_back | failed.

    Attributes:
        tx_id: Unique transaction identifier (UUID4)
        op: Operation type (rename, mkdir, delete, copy)
        src: Source path (absolute, resolved)
        dest: Destination path (None for delete operations)
        state: Current state of the transaction
        timestamp: Unix timestamp when entry was created
        timeout_at: Unix timestamp after which transaction is considered stale
    """

    tx_id: str = Field(default_factory=lambda: str(uuid4()))
    op: Literal["rename", "mkdir", "delete", "copy"]
    src: str
    dest: str | None = None
    state: Literal["prepared", "committed", "rolled_back", "failed"]
    timestamp: float = Field(default_factory=lambda: datetime.now().timestamp())
    timeout_at: float

    def is_stale(self, current_time: float | None = None) -> bool:
        """
        Check if this transaction is stale (past its timeout).

        Args:
            current_time: Current unix timestamp (defaults to now)

        Returns:
            True if the transaction has timed out
        """
        if current_time is None:
            current_time = datetime.now().timestamp()
        return current_time > self.timeout_at

    def to_journal_line(self) -> str:
        """
        Serialize this entry to a single line for journal.jsonl.

        Returns:
            JSON string suitable for writing to journal file
        """
        return self.model_dump_json() + "\n"
