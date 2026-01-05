"""
Core safety and infrastructure modules.
"""

from .journal import JournalManager
from .path_safety import is_safe_path, is_safe_config_dir, validate_path_or_raise
from .sentinel import (
    EventCoalescer,
    FileEvent,
    PollingWatcher,
    ScannerMode,
    Sentinel,
    SentinelEventHandler,
    StabilityTracker,
)
from .trash import SmartTrashManager, TrashConfig
from .transaction import (
    Transaction,
    TransactionError,
    TransactionExecutionError,
    TransactionState,
    TransactionValidationError,
)

__all__ = [
    "is_safe_path",
    "is_safe_config_dir",
    "validate_path_or_raise",
    "JournalManager",
    "Transaction",
    "TransactionState",
    "TransactionError",
    "TransactionValidationError",
    "TransactionExecutionError",
    "Sentinel",
    "ScannerMode",
    "StabilityTracker",
    "EventCoalescer",
    "PollingWatcher",
    "FileEvent",
    "SentinelEventHandler",
    "SmartTrashManager",
    "TrashConfig",
]
