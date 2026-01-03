"""
Galgame Library Manager - FastAPI Backend Application

This is the main FastAPI application that provides REST API endpoints for managing
a Galgame library with transaction-based file operations and noise-resilient scanning.

Phases:
- Phase 1: JournalManager + Recovery Logic + is_safe_path Sandbox
- Phase 2: Transaction Engine (rename/mkdir/copy/delete + rollback)
- Phase 3: Sentinel (File System Watcher) with multiple modes
- Phase 4A: FastAPI Backend API
"""

import logging
import os
import tempfile
from contextlib import asynccontextmanager
from pathlib import Path
from typing import List, Optional

from fastapi import FastAPI, HTTPException, status, Depends, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.middleware.gzip import GZipMiddleware
from pydantic import BaseModel, Field

# ============================================================================
# Phase 19.8: Security & Hardening (Rate Limiting)
# ============================================================================
from slowapi import _rate_limit_exceeded_handler
from slowapi.errors import RateLimitExceeded
from .core.rate_limiter import limiter
# ============================================================================

from .core import (
    JournalManager,
    ScannerMode,
    Sentinel,
    SmartTrashManager,
    Transaction,
    TransactionError,
    TransactionState,
)
from .models.journal import JournalEntry
from .config import get_config

# ============================================================
# PHASE 26.0: PORTABLE LOGGING
# ============================================================

# Determine portable log path from environment variable
# If running in portable mode, VNITE_LOG_PATH will be set by Electron
PORTABLE_LOG_PATH = os.getenv('VNITE_LOG_PATH')
PORTABLE_DATA_PATH = os.getenv('VNITE_DATA_PATH')

# Create log directory if it doesn't exist
if PORTABLE_LOG_PATH:
    log_path = Path(PORTABLE_LOG_PATH)
    log_path.mkdir(parents=True, exist_ok=True)
    log_file = log_path / 'backend.log'
else:
    # Fallback to default behavior
    log_file = Path('backend.log')  # Current directory

# Configure logging with RotatingFileHandler for portable mode
from logging.handlers import RotatingFileHandler

# Create formatters
file_formatter = logging.Formatter(
    '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
console_formatter = logging.Formatter(
    '%(levelname)s - %(name)s - %(message)s'
)

# Setup root logger
root_logger = logging.getLogger()
root_logger.setLevel(logging.INFO)

# File handler (with rotation for portable mode)
file_handler = RotatingFileHandler(
    log_file,
    maxBytes=10*1024*1024,  # 10MB
    backupCount=5,
    encoding='utf-8'
)
file_handler.setFormatter(file_formatter)
root_logger.addHandler(file_handler)

# Console handler
console_handler = logging.StreamHandler()
console_handler.setFormatter(console_formatter)
root_logger.addHandler(console_handler)

logger = logging.getLogger(__name__)

# Log portable mode status
if PORTABLE_LOG_PATH:
    logger.info("=" * 70)
    logger.info("PHASE 26.0: PORTABLE LOGGING MODE")
    logger.info("=" * 70)
    logger.info(f"Log Path: {PORTABLE_LOG_PATH}")
    logger.info(f"Data Path: {PORTABLE_DATA_PATH}")
    logger.info("=" * 70)

# Default paths (can be overridden by environment variables)
DEFAULT_LIBRARY_ROOT = Path.home() / "Galgames"
DEFAULT_CONFIG_DIR = Path.home() / ".galgame-manager" / "config"


# ============================================================================
# Pydantic Models for Request/Response
# ============================================================================

class ScanModeRequest(BaseModel):
    """Request model for setting scanner mode."""
    mode: str = Field(..., description="Scanner mode: realtime, scheduled, or manual")
    scheduled_time: Optional[str] = Field(None, description="Scheduled time in HH:MM format (for scheduled mode)")


class ScanStatusResponse(BaseModel):
    """Response model for scanner status."""
    mode: str
    is_running: bool
    library_root: str
    scheduled_time: Optional[str] = None


class LibraryFile(BaseModel):
    """Represents a file in the library."""
    path: str
    name: str
    is_dir: bool
    size: Optional[int] = None
    modified_time: Optional[float] = None


class LibraryFilesResponse(BaseModel):
    """Response model for library files listing."""
    files: List[LibraryFile]
    total_count: int


class OrganizeRequest(BaseModel):
    """Request model for file organization operations."""
    operation: str = Field(..., description="Operation: rename, mkdir, copy, delete")
    src: str = Field(..., description="Source path")
    dest: Optional[str] = Field(None, description="Destination path (for rename, copy)")


class OrganizeResponse(BaseModel):
    """Response model for organize operations."""
    success: bool
    transaction_id: str
    message: str
    state: str


class HistoryEntry(BaseModel):
    """Represents a journal entry."""
    tx_id: str
    op: str
    src: str
    dest: Optional[str]
    state: str
    timestamp: float
    timeout_at: float


class HistoryResponse(BaseModel):
    """Response model for transaction history."""
    entries: List[HistoryEntry]
    total_count: int


class TrashConfigRequest(BaseModel):
    """Request model for updating trash configuration."""
    max_size_gb: Optional[float] = Field(None, ge=0, description="Max trash size in GB (0 = unlimited)")
    retention_days: Optional[int] = Field(None, ge=1, description="Days to keep trash")
    min_disk_free_gb: Optional[float] = Field(None, ge=0, description="Min free disk space in GB")


class TrashConfigResponse(BaseModel):
    """Response model for trash configuration."""
    max_size_gb: float
    retention_days: int
    min_disk_free_gb: float


class TrashStatusResponse(BaseModel):
    """Response model for trash status."""
    trash_items: int
    trash_size_gb: float
    max_size_gb: float
    disk_free_gb: float
    min_disk_free_gb: float
    retention_days: int
    oldest_item: Optional[str]


# ============================================================================
# Batch Metadata Scan Models
# ============================================================================

class BatchStartRequest(BaseModel):
    """Request model for starting batch metadata scan."""
    dry_run: bool = Field(True, description="Simulate without actual downloads")
    download_screenshots: bool = Field(True, description="Download screenshots (only when dry_run=False)")
    prefer_traditional: bool = Field(True, description="Prefer Traditional Chinese over Simplified")
    targets: Optional[List[str]] = Field(None, description="Optional list of specific game paths")
    provider: str = Field("vndb", description="Metadata provider: 'vndb' or 'bangumi'")


class BatchStartResponse(BaseModel):
    """Response model for starting batch scan."""
    success: bool
    message: str
    total_items: Optional[int] = None


class BatchStatusResponse(BaseModel):
    """Response model for batch scan status."""
    status: str
    progress_percent: float
    processed_count: int
    total_count: int
    current_item: str
    eta_seconds: Optional[int]
    logs: List[dict]
    results: dict
    dry_run: bool
    quota: dict


class BatchControlResponse(BaseModel):
    """Response model for batch control operations."""
    success: bool
    message: str


class FieldLockRequest(BaseModel):
    """Request model for locking/unlocking metadata fields."""
    game_path: str = Field(..., description="Path to game directory (relative to library root)")
    field_name: str = Field(..., description="Field name to lock/unlock (e.g., 'title', 'description')")
    lock: bool = Field(True, description="True to lock, False to unlock")


class FieldLockResponse(BaseModel):
    """Response model for field lock operations."""
    success: bool
    message: str
    locked: bool


# ============================================================================
# Game Metadata Models
# ============================================================================

class GameMetadataResponse(BaseModel):
    """Response model for game metadata."""
    success: bool
    metadata: Optional[dict] = None


class PlayStatusRequest(BaseModel):
    """Request model for updating play status."""
    game_path: str = Field(..., description="Path to game directory (relative to library root)")
    play_status: str = Field(..., description="Play status: unplayed, playing, completed, dropped, paused, wishlist")


class PlayStatusResponse(BaseModel):
    """Response model for play status update."""
    success: bool
    message: str
    play_status: str


class ApplyMetadataRequest(BaseModel):
    """Request model for applying selected candidate metadata."""
    game_path: str = Field(..., description="Path to game directory (relative to library root)")
    match_id: str = Field(..., description="Match ID from candidate (e.g., vndb ID)")
    source: str = Field(..., description="Source of metadata: vndb, local, manual")


class ApplyMetadataResponse(BaseModel):
    """Response model for applying metadata."""
    success: bool
    message: str
    metadata: Optional[dict] = None



# ============================================================================
# Application Lifecycle
# ============================================================================

def _create_rollback_handler(journal: JournalManager, library_root: Path, logger):
    """
    Create a rollback handler for recovering incomplete transactions.

    This handler instantiates a Transaction and calls rollback() to reverse
    incomplete operations from previous crashes.

    Args:
        journal: JournalManager instance
        library_root: Library root directory
        logger: Logger instance

    Returns:
        Rollback handler function for journal.recover()
    """
    def rollback_handler(entry):
        """
        Rollback a single incomplete transaction.

        Args:
            entry: JournalEntry to rollback

        Returns:
            True if rollback succeeded, False otherwise
        """
        try:
            logger.info(f"Rolling back transaction {entry.tx_id}: {entry.op} on {entry.src}")

            # Create a transaction for rollback
            tx = Transaction(journal, library_root)

            # Manually set the entry (transaction was never committed)
            tx.entry = entry
            tx.state = TransactionState.FAILED  # It's in prepared state, needs rollback

            # Execute rollback
            tx.rollback()

            logger.info(f"Successfully rolled back transaction {entry.tx_id}")
            return True

        except Exception as e:
            logger.error(f"Failed to rollback transaction {entry.tx_id}: {e}")
            return False

    return rollback_handler


def check_read_only():
    """
    Dependency that blocks write operations when system is in READ-ONLY mode.

    DOOMSDAY FUSE: If recovery failed, system is locked to prevent data corruption.
    All POST/PUT/DELETE endpoints that modify data should use this dependency.

    Raises:
        HTTPException: 503 Service Unavailable if system is read-only
    """
    # We need to get the app state, but this is called before request
    # We'll check it in the endpoint itself
    pass


def verify_not_read_only():
    """
    Dependency function to check if system is in read-only mode.

    Use this in endpoints that modify data:
        @app.post("/api/modify")
        async def modify_data(request, _ok: None = Depends(verify_not_read_only)):

    Raises:
        HTTPException: 503 if system is read-only
    """
    from fastapi import Request

    async def _check(request: Request):
        if getattr(request.app.state, 'is_read_only', False):
            raise HTTPException(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                detail=(
                    "System is in READ-ONLY mode due to recovery failure. "
                    "No write operations are allowed. "
                    "Please contact administrator to resolve journal corruption."
                )
            )
        return None

    return _check


@asynccontextmanager
async def lifespan(app: FastAPI):
    """
    Manage application lifecycle.

    Initializes JournalManager and Sentinel on startup,
    ensures clean shutdown on application exit.
    """
    # Startup
    logger.info("Starting Galgame Library Manager API...")

    # Get configuration (with sandbox support)
    config = get_config()
    paths = config.get_paths()

    library_root = paths["library_root"]
    config_dir = paths["config_dir"]
    journal_dir = paths["journal_dir"]
    trash_dir = paths["trash_dir"]

    logger.info(f"Library root: {library_root}")
    logger.info(f"Config directory: {config_dir}")
    logger.info(f"Journal directory: {journal_dir}")
    logger.info(f"Trash directory: {trash_dir}")

    # Initialize JournalManager
    try:
        journal_manager = JournalManager(config_dir)
        logger.info("JournalManager initialized")
    except Exception as e:
        logger.error(f"Failed to initialize JournalManager: {e}")
        raise

    # CRITICAL: Perform journal recovery on startup with Doomsday Fuse
    # This rolls back any incomplete transactions from previous crashes
    # IF RECOVERY FAILS: System enters READ-ONLY mode to prevent data corruption
    logger.info("Checking for incomplete transactions...")

    # Initialize read-only mode flag
    is_read_only = False

    try:
        recovery_result = journal_manager.recover(
            rollback_handler=_create_rollback_handler(journal_manager, library_root, logger)
        )

        if recovery_result['stale']:
            logger.info(f"Recovery: Found {len(recovery_result['stale'])} stale transaction(s)")
            for entry in recovery_result['stale']:
                logger.info(f"  - Rolled back {entry.op}: {entry.src} -> {entry.dest}")

        if recovery_result['active']:
            logger.warning(f"Recovery: Found {len(recovery_result['active'])} active prepared transaction(s)")
            for entry in recovery_result['active']:
                logger.warning(f"  - Active transaction {entry.tx_id}: {entry.op} on {entry.src}")

    except Exception as e:
        # DOOMSDAY FUSE: Recovery failed - lock system to prevent data corruption
        logger.critical(f"RECOVERY FAILED. SYSTEM LOCKED. Error: {e}")
        logger.critical("All write operations are BLOCKED. System is in READ-ONLY mode.")
        logger.critical("Administrator intervention required to resolve journal corruption.")
        is_read_only = True

    # Initialize Sentinel callback
    scan_results = []

    def on_directories_changed(directories: List[Path]):
        """Handle directory change events from Sentinel."""
        logger.info(f"Scan detected {len(directories)} changed director(y/ies)")
        for directory in directories:
            logger.info(f"  - {directory}")
        # In production, this would trigger metadata updates
        scan_results.extend(directories)

    # Initialize Sentinel
    try:
        # Default to MANUAL mode for API usage
        sentinel = Sentinel(
            library_roots=library_root,  # Changed from library_root to library_roots
            callback=on_directories_changed,
            initial_mode=ScannerMode.MANUAL
        )
        logger.info("Sentinel initialized")
    except Exception as e:
        logger.error(f"Failed to initialize Sentinel: {e}")
        raise

    # Store in app.state
    app.state.library_root = library_root
    app.state.config_dir = config_dir
    app.state.journal_manager = journal_manager
    app.state.sentinel = sentinel
    app.state.is_read_only = is_read_only  # DOOMSDAY FUSE: Read-only mode if recovery failed

    # Initialize BatchManager for metadata scanning
    from .metadata import get_batch_manager
    batch_manager = get_batch_manager()
    batch_manager.configure(library_root=library_root, rate_limit_delay=1.0)
    app.state.batch_manager = batch_manager
    logger.info("BatchManager initialized")

    # ========================================================================
    # PHASE 20.0: Initialize SQLite Database
    # ========================================================================
    from .core.database import get_database
    db = get_database()
    app.state.database = db
    logger.info("SQLite database initialized (Instant Index)")

    # ========================================================================
    # PHASE 20.0: Start Background Scanner
    # ========================================================================
    from .services.scanner import get_scanner
    scanner = get_scanner()
    app.state.scanner = scanner

    # Trigger initial scan in background (non-blocking)
    if config.scan_on_startup:
        logger.info("Triggering initial library scan in background...")
        scanner.scan_library(background=True)
    else:
        logger.info("Scan on startup disabled (manual mode)")

    # Start Sentinel
    try:
        # PHASE 19.6: Load snapshot on startup for instant boot
        # NOTE: load_snapshot not available in current Sentinel version
        # loaded = sentinel.load_snapshot()
        # if loaded:
        #     logger.info("Loaded polling snapshot from previous session")

        sentinel.start()
        logger.info(f"Sentinel started in {sentinel.mode.value} mode")
    except Exception as e:
        logger.error(f"Failed to start Sentinel: {e}")
        raise

    # Phase 24.5: Start Scheduler
    try:
        from .services.scheduler import startup_hook
        startup_hook()
        logger.info("Task scheduler started")
    except Exception as e:
        logger.error(f"Failed to start scheduler: {e}")
        # Don't raise - scheduler is non-critical

    yield

    # Shutdown
    logger.info("Shutting down Galgame Library Manager API...")

    # Phase 24.5: Shutdown Scheduler
    try:
        from .services.scheduler import shutdown_hook
        shutdown_hook()
        logger.info("Task scheduler stopped")
    except Exception as e:
        logger.error(f"Failed to shutdown scheduler: {e}")

    # PHASE 19.6: Save snapshot on shutdown for persistence
    # NOTE: save_snapshot not available in current Sentinel version
    # try:
    #     sentinel.save_snapshot()
    #     logger.info("Saved polling snapshot for next startup")
    # except Exception as e:
    #     logger.error(f"Failed to save snapshot: {e}")

    sentinel.stop()
    logger.info("Sentinel stopped")
    logger.info("Shutdown complete")


# ============================================================================
# FastAPI Application
# ============================================================================

app = FastAPI(
    title="Galgame Library Manager API",
    description="Backend API for managing Galgame library with transaction-based file operations",
    version="1.0.0",
    lifespan=lifespan
)

# ============================================================================
# Phase 19.8: Security Integration (Rate Limiting)
# ============================================================================
# Mount limiter to app state
app.state.limiter = limiter
# Register 429 error handler
app.add_exception_handler(RateLimitExceeded, _rate_limit_exceeded_handler)
# ============================================================================

# Configure CORS for frontend development
app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:5173",  # Vite default
        "http://localhost:3000",  # Alternative dev server
        "http://localhost:5174",  # Alternative Vite port
        "http://127.0.0.1:5173",
        "http://127.0.0.1:3000",
        "http://127.0.0.1:5174",
    ],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# ============================================================================
# PHASE 20.0: Enable Gzip Compression
# ============================================================================
# Compress API responses to reduce payload size
# Minimum size: 1000 bytes (compress responses larger than 1KB)
app.add_middleware(GZipMiddleware, minimum_size=1000)
# ============================================================================

# ============================================================================
# PHASE 9.5: Include Organizer Router
# ============================================================================
from .api.organizer import router as organizer_router
app.include_router(organizer_router)

# ============================================================================
# PHASE 10: Include Curator & System Routers
# ============================================================================
from .api.curator import router as curator_router
from .api.system import router as system_router
app.include_router(curator_router)
app.include_router(system_router)

# ============================================================================
# PHASE 11: Include Analytics & Search Routers
# ============================================================================
from .api.analytics import router as analytics_router
from .api.search import router as search_router
app.include_router(analytics_router)
app.include_router(search_router)

# ============================================================================
# PHASE 12: Include Connectors Router
# ============================================================================
from .api.connectors import router as connectors_router
app.include_router(connectors_router)

# ============================================================================
# UTILITIES BELT: Helper Tools Router
# ============================================================================
from .api.utilities import router as utilities_router
app.include_router(utilities_router)

# ============================================================================
# PHASE 19.6: History & Settings Routers
# ============================================================================
from .api.history import router as history_router
from .api.settings import router as settings_router
app.include_router(history_router)
app.include_router(settings_router)

# ============================================================================
# PHASE 24.5: Scheduler Router
# ============================================================================
from .api.scheduler import router as scheduler_router
app.include_router(scheduler_router)

# ============================================================================
# PHASE 24.5: Backup Router
# ============================================================================
from .api.backup import router as backup_router
app.include_router(backup_router)

# ============================================================================
# PHASE 24.5: Update Router
# ============================================================================
from .api.update import router as update_router
app.include_router(update_router)

# ============================================================================
# PHASE 19.7: Games Router
# ============================================================================
from .api.games import router as games_router
app.include_router(games_router)

# ============================================================================
# API Endpoints: Scanner Management
# ============================================================================

@app.post("/api/scan/mode", response_model=ScanStatusResponse)
async def set_scan_mode(
    request: ScanModeRequest,
    _ok: None = Depends(verify_not_read_only)
) -> ScanStatusResponse:
    """
    Switch the Sentinel scanner mode.

    Modes:
    - realtime: Uses watchdog + Stability Pact (45s debounce) + Coalescing
    - scheduled: Daily scan at specified time (default 03:00 AM)
    - manual: Idle mode, manual trigger only

    Args:
        request: Scan mode request with mode and optional scheduled_time

    Returns:
        Current scanner status
    """
    sentinel: Sentinel = app.state.sentinel

    # Parse mode
    try:
        new_mode = ScannerMode(request.mode.lower())
    except ValueError:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Invalid mode: {request.mode}. Must be 'realtime', 'scheduled', or 'manual'"
        )

    # Update scheduled time if provided
    if request.scheduled_time:
        # Validate time format
        try:
            hour, minute = map(int, request.scheduled_time.split(":"))
            if not (0 <= hour <= 23 and 0 <= minute <= 59):
                raise ValueError()
            sentinel.scheduled_time = request.scheduled_time
        except (ValueError, AttributeError):
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Invalid scheduled_time format. Use 'HH:MM'"
            )

    # Switch mode
    sentinel.configure(new_mode)
    logger.info(f"Scanner mode switched to {new_mode.value}")

    return ScanStatusResponse(
        mode=sentinel.mode.value,
        is_running=sentinel.is_running(),
        library_root=str(app.state.library_root),
        scheduled_time=sentinel.scheduled_time if sentinel.mode == ScannerMode.SCHEDULED else None
    )


@app.get("/api/scan/status", response_model=ScanStatusResponse)
async def get_scan_status() -> ScanStatusResponse:
    """
    Get the current scanner status.

    Returns:
        Current scanner status including mode, running state, and paths
    """
    sentinel: Sentinel = app.state.sentinel

    return ScanStatusResponse(
        mode=sentinel.mode.value,
        is_running=sentinel.is_running(),
        library_root=str(app.state.library_root),
        scheduled_time=sentinel.scheduled_time if sentinel.mode == ScannerMode.SCHEDULED else None
    )


# ============================================================================
# API Endpoints: Library Management
# ============================================================================

@app.get("/api/library/files", response_model=LibraryFilesResponse)
async def list_library_files(limit: int = 1000) -> LibraryFilesResponse:
    """
    List all files in the library directory.

    Uses is_safe_path to validate all paths before returning them.

    Args:
        limit: Maximum number of files to return (default: 1000)

    Returns:
        List of library files with metadata
    """
    from .core import is_safe_path

    library_root: Path = app.state.library_root
    files = []

    try:
        # Walk the library directory
        for item in library_root.rglob("*"):
            if len(files) >= limit:
                break

            # Validate path safety
            if not is_safe_path(item, library_root):
                logger.warning(f"Skipping unsafe path: {item}")
                continue

            try:
                stat = item.stat()
                files.append(LibraryFile(
                    path=str(item.relative_to(library_root)),
                    name=item.name,
                    is_dir=item.is_dir(),
                    size=stat.st_size if not item.is_dir() else None,
                    modified_time=stat.st_mtime
                ))
            except OSError as e:
                logger.error(f"Error accessing {item}: {e}")

    except Exception as e:
        logger.error(f"Error listing library files: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error listing files: {str(e)}"
        )

    return LibraryFilesResponse(
        files=files,
        total_count=len(files)
    )


@app.post("/api/library/organize", response_model=OrganizeResponse)
async def organize_library(
    request: OrganizeRequest,
    _ok: None = Depends(verify_not_read_only)
) -> OrganizeResponse:
    """
    Execute a file organization operation using the Transaction engine.

    Supported operations:
    - rename: Move/rename a file or directory
    - mkdir: Create a new directory
    - copy: Copy a file or directory
    - delete: Delete a file or directory

    All operations use the Transaction engine with journaling and rollback capability.

    Args:
        request: Organize request with operation, src, and dest

    Returns:
        Operation result with transaction ID and state
    """
    from .core import is_safe_path

    journal: JournalManager = app.state.journal_manager
    library_root: Path = app.state.library_root

    # Validate operation type
    valid_ops = ["rename", "mkdir", "copy", "delete"]
    if request.operation not in valid_ops:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Invalid operation: {request.operation}. Must be one of {valid_ops}"
        )

    # Build paths
    src = library_root / request.src
    dest = library_root / request.dest if request.dest else None

    # Validate paths
    if not is_safe_path(src, library_root):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Source path is not safe: {request.src}"
        )

    if dest and not is_safe_path(dest, library_root):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Destination path is not safe: {request.dest}"
        )

    # Execute transaction with explicit journal failure handling
    try:
        tx = Transaction(journal, library_root)

        # CRITICAL: prepare() MUST succeed before any operation
        # If journal write fails here, TransactionExecutionError is raised
        # and we MUST NOT proceed to commit()
        tx.prepare(request.operation, src, dest)

        # Only execute file operation if journal write succeeded
        tx.commit()

        return OrganizeResponse(
            success=True,
            transaction_id=tx.entry.tx_id,
            message=f"Operation '{request.operation}' completed successfully",
            state=tx.state.value
        )

    except TransactionValidationError as e:
        # Path validation failed - user error
        logger.error(f"Transaction validation failed: {e}")
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Validation failed: {str(e)}"
        )

    except TransactionExecutionError as e:
        # CRITICAL: Journal write or operation execution failed
        # This includes disk full, permission errors, etc.
        # No file operation was executed (journal write failed before commit)
        logger.error(f"CRITICAL: Transaction execution failed: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Operation failed: {str(e)}"
        )

    except TransactionError as e:
        # Other transaction errors
        logger.error(f"Transaction error: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Transaction failed: {str(e)}"
        )

    except Exception as e:
        # Unexpected errors
        logger.error(f"Unexpected error during organize: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Operation failed: {str(e)}"
        )


@app.post("/api/library/scan")
async def trigger_manual_scan() -> dict:
    """
    Trigger a manual library scan (useful in MANUAL mode).

    Returns:
        Scan results with directory count
    """
    sentinel: Sentinel = app.state.sentinel

    try:
        directories = sentinel.trigger_scan()
        return {
            "success": True,
            "directories_scanned": len(directories),
            "message": f"Manual scan completed: {len(directories)} director(y/ies)"
        }
    except Exception as e:
        logger.error(f"Error during manual scan: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Scan failed: {str(e)}"
        )


# ============================================================================
# API Endpoints: History & Audit (Moved to /api/history.py in Phase 19.6)
# ============================================================================

# ============================================================================
# API Endpoints: Trash Management
# ============================================================================

@app.get("/api/trash/status", response_model=TrashStatusResponse)
async def get_trash_status() -> TrashStatusResponse:
    """
    Get current trash status.

    Returns:
        Trash statistics including size, count, disk space
    """
    config_dir: Path = app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)
    status = trash_manager.get_status()

    return TrashStatusResponse(**status)


@app.get("/api/trash/config", response_model=TrashConfigResponse)
async def get_trash_config() -> TrashConfigResponse:
    """
    Get current trash configuration.

    Returns:
        Current trash configuration
    """
    config_dir: Path = app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)

    return TrashConfigResponse(
        max_size_gb=trash_manager.config.max_size_gb,
        retention_days=trash_manager.config.retention_days,
        min_disk_free_gb=trash_manager.config.min_disk_free_gb
    )


@app.post("/api/trash/config", response_model=TrashConfigResponse)
async def update_trash_config(
    request: TrashConfigRequest,
    _ok: None = Depends(verify_not_read_only)
) -> TrashConfigResponse:
    """
    Update trash configuration.

    Allows user to customize:
    - Maximum trash size (default: 50GB, 0 = unlimited)
    - Retention days (default: 30)
    - Minimum disk free space (default: 5GB)

    Args:
        request: Configuration updates

    Returns:
        Updated configuration
    """
    config_dir: Path = app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)

    updated_config = trash_manager.update_config(
        max_size_gb=request.max_size_gb,
        retention_days=request.retention_days,
        min_disk_free_gb=request.min_disk_free_gb
    )

    logger.info(f"Trash config updated: max_size={updated_config.max_size_gb}GB, "
                f"retention={updated_config.retention_days}d, "
                f"min_free={updated_config.min_disk_free_gb}GB")

    return TrashConfigResponse(
        max_size_gb=updated_config.max_size_gb,
        retention_days=updated_config.retention_days,
        min_disk_free_gb=updated_config.min_disk_free_gb
    )


@app.post("/api/trash/empty")
async def empty_trash(
    _ok: None = Depends(verify_not_read_only)
) -> dict:
    """
    Empty all trash immediately.

    WARNING: This permanently deletes all trash. Use with caution!

    Returns:
        Number of trash items deleted
    """
    config_dir: Path = app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)

    deleted_count = trash_manager.empty_trash()

    logger.warning(f"Trash emptied by user: {deleted_count} items deleted")

    return {
        "success": True,
        "deleted_count": deleted_count,
        "message": f"Emptied {deleted_count} trash items"
    }


# ============================================================================
# Batch Metadata Scan Endpoints
# ============================================================================

@app.post("/api/metadata/batch/start", response_model=BatchStartResponse)
async def start_batch_scan(request: BatchStartRequest) -> BatchStartResponse:
    """
    Start a batch metadata scan.

    Supports dry run mode for simulation without actual downloads.
    Processes games in chunks with rate limiting to prevent API bans.

    Args:
        request: Batch start request with configuration

    Returns:
        Start response with success status and total items to process

    Example:
        POST /api/metadata/batch/start
        {
            "dry_run": true,
            "download_screenshots": true,
            "prefer_traditional": true,
            "targets": null  # null = auto-discover all games
        }
    """
    batch_manager = request.app.state.batch_manager

    # Reconfigure with selected provider if different
    if batch_manager._provider != request.provider:
        library_root: Path = request.app.state.library_root
        config_dir: Path = request.app.state.config_dir
        batch_manager.configure(
            library_root=library_root,
            rate_limit=1.0,
            quota_gb=2.0,
            provider=request.provider
        )

    result = batch_manager.start_scan(
        dry_run=request.dry_run,
        download_screenshots=request.download_screenshots,
        prefer_traditional=request.prefer_traditional,
        targets=request.targets
    )

    return BatchStartResponse(**result)


@app.post("/api/metadata/batch/pause", response_model=BatchControlResponse)
async def pause_batch_scan(request) -> BatchControlResponse:
    """
    Pause the current batch scan after current item finishes.

    Returns:
        Control response with success status

    Example:
        POST /api/metadata/batch/pause
    """
    batch_manager = request.app.state.batch_manager

    result = batch_manager.pause_scan()

    return BatchControlResponse(**result)


@app.post("/api/metadata/batch/resume", response_model=BatchControlResponse)
async def resume_batch_scan(request) -> BatchControlResponse:
    """
    Resume a paused batch scan.

    Returns:
        Control response with success status

    Example:
        POST /api/metadata/batch/resume
    """
    batch_manager = request.app.state.batch_manager

    result = batch_manager.resume_scan()

    return BatchControlResponse(**result)


@app.post("/api/metadata/batch/stop", response_model=BatchControlResponse)
async def stop_batch_scan(request) -> BatchControlResponse:
    """
    Stop the current batch scan completely.

    This will abort the scan and it cannot be resumed.
    Use pause if you want to resume later.

    Returns:
        Control response with success status

    Example:
        POST /api/metadata/batch/stop
    """
    batch_manager = request.app.state.batch_manager

    result = batch_manager.stop_scan()

    return BatchControlResponse(**result)


@app.get("/api/metadata/batch/status", response_model=BatchStatusResponse)
async def get_batch_status(request) -> BatchStatusResponse:
    """
    Get current batch scan status and progress.

    Returns:
        Status response with progress, logs, and results

    Example:
        GET /api/metadata/batch/status
    """
    batch_manager = request.app.state.batch_manager

    status = batch_manager.get_status()

    return BatchStatusResponse(**status)


# ============================================================================
# Metadata Field Lock Endpoints
# ============================================================================

@app.post("/api/metadata/field/lock", response_model=FieldLockResponse)
async def lock_metadata_field(request: FieldLockRequest) -> FieldLockResponse:
    """
    Lock or unlock a specific metadata field for a game.

    Locked fields will never be overwritten by batch scans (Curator feature).

    Args:
        request: Field lock request with game_path, field_name, and lock flag

    Returns:
        Lock response with new lock status

    Example:
        POST /api/metadata/field/lock
        {
            "game_path": "Fate/stay night",
            "field_name": "description",
            "lock": true
        }
    """
    from .metadata import get_resource_manager

    resource_manager = get_resource_manager(request.app.state.library_root, 2.0)

    # Build game directory path
    game_dir = request.app.state.library_root / request.game_path

    if not game_dir.exists():
        return FieldLockResponse(
            success=False,
            message=f"Game directory not found: {request.game_path}",
            locked=False
        )

    # Load existing metadata
    metadata_dict = resource_manager.load_metadata(game_dir)

    if not metadata_dict:
        return FieldLockResponse(
            success=False,
            message=f"No metadata found for: {request.game_path}",
            locked=False
        )

    # Lock or unlock the field
    try:
        if request.field_name not in metadata_dict:
            return FieldLockResponse(
                success=False,
                message=f"Field '{request.field_name}' not found in metadata",
                locked=False
            )

        # Handle nested MetadataField structure
        field_data = metadata_dict[request.field_name]

        if isinstance(field_data, dict) and "locked" in field_data:
            field_data["locked"] = request.lock
        elif isinstance(field_data, dict) and "value" in field_data:
            # This is a MetadataField, add/update locked key
            field_data["locked"] = request.lock
        else:
            # This is a direct value, wrap it in MetadataField structure
            metadata_dict[request.field_name] = {
                "value": field_data,
                "source": "manual",
                "locked": request.lock
            }

        # Save updated metadata
        success = resource_manager.save_metadata(metadata_dict, game_dir)

        if success:
            action = "locked" if request.lock else "unlocked"
            return FieldLockResponse(
                success=True,
                message=f"Field '{request.field_name}' {action}",
                locked=request.lock
            )
        else:
            return FieldLockResponse(
                success=False,
                message="Failed to save metadata",
                locked=not request.lock  # Return current state
            )

    except Exception as e:
        return FieldLockResponse(
            success=False,
            message=f"Error: {str(e)}",
            locked=False
        )


@app.get("/api/metadata/field/status", response_model=dict)
async def get_field_lock_status(request, game_path: str) -> dict:
    """
    Get lock status for all fields in a game's metadata.

    Args:
        game_path: Path to game directory (relative to library root)

    Returns:
        Dictionary with lock status for each field

    Example:
        GET /api/metadata/field/status?game_path=Fate/stay night
    """
    from .metadata import get_resource_manager

    resource_manager = get_resource_manager(request.app.state.library_root, 2.0)

    # Build game directory path
    game_dir = request.app.state.library_root / game_path

    if not game_dir.exists():
        return {
            "success": False,
            "message": f"Game directory not found: {game_path}",
            "fields": {}
        }

    # Load existing metadata
    metadata_dict = resource_manager.load_metadata(game_dir)

    if not metadata_dict:
        return {
            "success": False,
            "message": f"No metadata found for: {game_path}",
            "fields": {}
        }

    # Extract lock status for each field
    field_locks = {}
    for field_name, field_data in metadata_dict.items():
        if isinstance(field_data, dict) and "locked" in field_data:
            field_locks[field_name] = {
                "locked": field_data["locked"],
                "source": field_data.get("source", "unknown")
            }
        elif isinstance(field_data, dict) and "value" in field_data:
            field_locks[field_name] = {
                "locked": field_data.get("locked", False),
                "source": field_data.get("source", "unknown")
            }

    return {
        "success": True,
        "game_path": game_path,
        "fields": field_locks
    }


# ============================================================================
# Game Metadata Endpoints
# ============================================================================

@app.get("/api/metadata/game/{game_path:path}", response_model=GameMetadataResponse)
async def get_game_metadata(game_path: str) -> GameMetadataResponse:
    """
    Get metadata for a specific game.

    Args:
        game_path: Path to game directory (relative to library root)

    Returns:
        Game metadata with all fields

    Example:
        GET /api/metadata/game/Fate/stay%20night
    """
    from .metadata import get_resource_manager
    from .core import is_safe_path

    library_root: Path = app.state.library_root

    # Validate path safety
    game_dir = library_root / game_path
    if not is_safe_path(game_dir, library_root):
        return GameMetadataResponse(
            success=False,
            metadata=None
        )

    if not game_dir.exists():
        return GameMetadataResponse(
            success=False,
            metadata=None
        )

    try:
        resource_manager = get_resource_manager(library_root, 2.0)
        metadata_dict = resource_manager.load_metadata(game_dir)

        if not metadata_dict:
            return GameMetadataResponse(
                success=False,
                metadata=None
            )

        return GameMetadataResponse(
            success=True,
            metadata=metadata_dict
        )

    except Exception as e:
        logger.error(f"Error loading metadata for {game_path}: {e}")
        return GameMetadataResponse(
            success=False,
            metadata=None
        )


@app.post("/api/metadata/play_status", response_model=PlayStatusResponse)
async def update_play_status(
    request: PlayStatusRequest,
    _ok: None = Depends(verify_not_read_only)
) -> PlayStatusResponse:
    """
    Update the play status for a game.

    Args:
        request: Play status request with game_path and play_status

    Returns:
        Updated play status

    Example:
        POST /api/metadata/play_status
        {
            "game_path": "Fate/stay night",
            "play_status": "playing"
        }
    """
    from .metadata import get_resource_manager, PlayStatus
    from .core import is_safe_path

    library_root: Path = app.state.library_root

    # Validate play status
    valid_statuses = [status.value for status in PlayStatus]
    if request.play_status not in valid_statuses:
        return PlayStatusResponse(
            success=False,
            message=f"Invalid play status. Must be one of: {', '.join(valid_statuses)}",
            play_status=""
        )

    # Build game directory path
    game_dir = library_root / request.game_path
    if not is_safe_path(game_dir, library_root):
        return PlayStatusResponse(
            success=False,
            message="Invalid game path",
            play_status=""
        )

    if not game_dir.exists():
        return PlayStatusResponse(
            success=False,
            message=f"Game directory not found: {request.game_path}",
            play_status=""
        )

    try:
        resource_manager = get_resource_manager(library_root, 2.0)
        metadata_dict = resource_manager.load_metadata(game_dir)

        if not metadata_dict:
            # Create new metadata with play status
            from .metadata import create_empty_metadata, MetadataField
            metadata = create_empty_metadata()
            metadata.play_status = MetadataField(
                value=PlayStatus(request.play_status),
                source="manual",
                locked=False
            )
            metadata_dict = metadata.model_dump()
        else:
            # Update existing play status
            if "play_status" in metadata_dict:
                if isinstance(metadata_dict["play_status"], dict):
                    metadata_dict["play_status"]["value"] = request.play_status
                    metadata_dict["play_status"]["source"] = "manual"
                else:
                    metadata_dict["play_status"] = {
                        "value": request.play_status,
                        "source": "manual",
                        "locked": False
                    }
            else:
                metadata_dict["play_status"] = {
                    "value": request.play_status,
                    "source": "manual",
                    "locked": False
                }

        # Save updated metadata
        success = resource_manager.save_metadata(metadata_dict, game_dir)

        if success:
            return PlayStatusResponse(
                success=True,
                message=f"Play status updated to '{request.play_status}'",
                play_status=request.play_status
            )
        else:
            return PlayStatusResponse(
                success=False,
                message="Failed to save metadata",
                play_status=""
            )

    except Exception as e:
        logger.error(f"Error updating play status: {e}")
        return PlayStatusResponse(
            success=False,
            message=f"Error: {str(e)}",
            play_status=""
        )


@app.post("/api/metadata/apply", response_model=ApplyMetadataResponse)
async def apply_metadata(
    request: ApplyMetadataRequest,
    _ok: None = Depends(verify_not_read_only)
) -> ApplyMetadataResponse:
    """
    Apply metadata from a selected candidate (e.g., from VNDB match).

    This fetches metadata from the specified source and applies it to the game.

    Args:
        request: Apply metadata request with game_path, match_id, and source

    Returns:
        Applied metadata

    Example:
        POST /api/metadata/apply
        {
            "game_path": "Fate/stay night",
            "match_id": "v12345",
            "source": "vndb"
        }
    """
    from .metadata import get_resource_manager, get_vndb_provider
    from .core import is_safe_path

    library_root: Path = app.state.library_root

    # Build game directory path
    game_dir = library_root / request.game_path
    if not is_safe_path(game_dir, library_root):
        return ApplyMetadataResponse(
            success=False,
            message="Invalid game path",
            metadata=None
        )

    if not game_dir.exists():
        return ApplyMetadataResponse(
            success=False,
            message=f"Game directory not found: {request.game_path}",
            metadata=None
        )

    try:
        # Fetch metadata based on source
        if request.source == "vndb":
            # Ensure match_id starts with 'v'
            vndb_id = request.match_id if request.match_id.startswith('v') else f'v{request.match_id}'

            # Fetch from VNDB
            vndb_provider = get_vndb_provider(rate_limit=1.0)
            vndb_data = vndb_provider.get_metadata_by_id(vndb_id)

            if not vndb_data:
                return ApplyMetadataResponse(
                    success=False,
                    message=f"Failed to fetch metadata from VNDB for ID: {vndb_id}",
                    metadata=None
                )

            # Parse VNDB data into UnifiedMetadata
            metadata = vndb_provider._parse_vndb_data(vndb_data, prefer_traditional=True)
            metadata_dict = metadata.model_dump()

        elif request.source == "local":
            # Load from local metadata.json
            resource_manager = get_resource_manager(library_root, 2.0)
            metadata_dict = resource_manager.load_metadata(game_dir)

            if not metadata_dict:
                return ApplyMetadataResponse(
                    success=False,
                    message=f"No local metadata found for: {request.game_path}",
                    metadata=None
                )

        elif request.source == "manual":
            # For manual entry, we treat match_id as VNDB ID
            vndb_id = request.match_id if request.match_id.startswith('v') else f'v{request.match_id}'

            # Fetch from VNDB
            vndb_provider = get_vndb_provider(rate_limit=1.0)
            vndb_data = vndb_provider.get_metadata_by_id(vndb_id)

            if not vndb_data:
                return ApplyMetadataResponse(
                    success=False,
                    message=f"Failed to fetch metadata from VNDB for ID: {vndb_id}",
                    metadata=None
                )

            # Parse VNDB data into UnifiedMetadata
            metadata = vndb_provider._parse_vndb_data(vndb_data, prefer_traditional=True)
            metadata_dict = metadata.model_dump()

        else:
            return ApplyMetadataResponse(
                success=False,
                message=f"Unknown source: {request.source}",
                metadata=None
            )

        # Save metadata to game directory
        resource_manager = get_resource_manager(library_root, 2.0)
        success = resource_manager.save_metadata(metadata_dict, game_dir)

        if success:
            logger.info(f"Applied metadata from {request.source} to {request.game_path}")
            return ApplyMetadataResponse(
                success=True,
                message=f"Metadata applied from {request.source}",
                metadata=metadata_dict
            )
        else:
            return ApplyMetadataResponse(
                success=False,
                message="Failed to save metadata",
                metadata=None
            )

    except Exception as e:
        logger.error(f"Error applying metadata: {e}")
        return ApplyMetadataResponse(
            success=False,
            message=f"Error: {str(e)}",
            metadata=None
        )


# ============================================================================
# System Operations Endpoints
# ============================================================================

class OpenFolderRequest(BaseModel):
    """Request model for opening a folder in system explorer."""
    path: str = Field(..., description="Path to open (relative to library root)")


@app.post("/api/system/open_folder")
async def open_folder(request: OpenFolderRequest) -> dict:
    """
    Open a folder in the system's default file explorer.

    Security: Validates path with is_safe_path before opening.

    Args:
        request: Folder path to open

    Returns:
        Success status

    Example:
        POST /api/system/open_folder
        {
            "path": "Fate/stay night"
        }
    """
    from .core import is_safe_path
    import platform
    import subprocess

    library_root: Path = app.state.library_root

    # Build and validate path
    target_path = library_root / request.path
    if not is_safe_path(target_path, library_root):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Path is not safe: {request.path}"
        )

    if not target_path.exists():
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Path does not exist: {request.path}"
        )

    try:
        # Open in system file explorer
        system = platform.system()

        if system == "Windows":
            # Use os.startfile on Windows
            os.startfile(str(target_path))
        elif system == "Darwin":  # macOS
            subprocess.run(["open", str(target_path)], check=True)
        else:  # Linux and others
            subprocess.run(["xdg-open", str(target_path)], check=True)

        logger.info(f"Opened folder in system explorer: {target_path}")

        return {
            "success": True,
            "message": f"Opened '{request.path}' in system explorer",
            "path": str(target_path)
        }

    except Exception as e:
        logger.error(f"Failed to open folder: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to open folder: {str(e)}"
        )


class ThrowToTrashRequest(BaseModel):
    """Request model for moving items to trash."""
    paths: List[str] = Field(..., description="List of paths to move to trash (relative to library root)")


@app.post("/api/trash/throw")
async def throw_to_trash(
    request: ThrowToTrashRequest,
    _ok: None = Depends(verify_not_read_only)
) -> dict:
    """
    Move multiple files or folders to trash.

    Uses the Smart Trash Manager for safe deletion.

    Args:
        request: List of paths to move to trash

    Returns:
        Success status with count

    Example:
        POST /api/trash/throw
        {
            "paths": ["Old Game 1", "Old Game 2"]
        }
    """
    from .core import is_safe_path

    library_root: Path = app.state.library_root
    config_dir: Path = app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)

    success_count = 0
    failed_items = []

    for path_str in request.paths:
        try:
            target_path = library_root / path_str

            # Validate path
            if not is_safe_path(target_path, library_root):
                failed_items.append({"path": path_str, "error": "Unsafe path"})
                continue

            if not target_path.exists():
                failed_items.append({"path": path_str, "error": "Path does not exist"})
                continue

            # Move to trash
            if trash_manager.throw_to_trash(target_path):
                success_count += 1
                logger.info(f"Moved to trash: {target_path}")
            else:
                failed_items.append({"path": path_str, "error": "Failed to move to trash"})

        except Exception as e:
            logger.error(f"Error moving {path_str} to trash: {e}")
            failed_items.append({"path": path_str, "error": str(e)})

    return {
        "success": success_count > 0,
        "moved_count": success_count,
        "total_requested": len(request.paths),
        "failed": failed_items,
        "message": f"Moved {success_count}/{len(request.paths)} items to trash"
    }


# ============================================================================
# Health Check Endpoint
# ============================================================================

@app.get("/api/health")
async def health_check() -> dict:
    """
    Health check endpoint.

    Returns:
        API health status with environment info
    """
    config = get_config()
    return {
        "status": "healthy",
        "service": "Galgame Library Manager API",
        "version": "1.0.0",
        "env": config.get_info()["mode"],
        "sandbox": config.sandbox_mode,
    }


# ============================================================================
# Root Endpoint
# ============================================================================

@app.get("/")
async def root() -> dict:
    """
    Root endpoint with API information.

    Returns:
        API information and available endpoints
    """
    return {
        "name": "Galgame Library Manager API",
        "version": "1.0.0",
        "description": "Backend API for managing Galgame library with transaction-based file operations",
        "endpoints": {
            "scan": {
                "POST /api/scan/mode": "Switch scanner mode (realtime/scheduled/manual)",
                "GET /api/scan/status": "Get current scanner status",
                "POST /api/library/scan": "Trigger manual scan"
            },
            "library": {
                "GET /api/library/files": "List all files in library",
                "POST /api/library/organize": "Execute file operation (rename/mkdir/copy/delete)"
            },
            "metadata": {
                "POST /api/metadata/batch/start": "Start batch metadata scan (supports dry run)",
                "POST /api/metadata/batch/pause": "Pause batch scan",
                "POST /api/metadata/batch/resume": "Resume paused batch scan",
                "POST /api/metadata/batch/stop": "Stop batch scan completely",
                "GET /api/metadata/batch/status": "Get batch scan status and progress",
                "GET /api/metadata/game/{game_path}": "Get metadata for a specific game",
                "POST /api/metadata/play_status": "Update play status for a game",
                "POST /api/metadata/apply": "Apply metadata from selected candidate",
                "POST /api/metadata/field/lock": "Lock/unlock metadata fields",
                "GET /api/metadata/field/status": "Get lock status for all fields"
            },
            "history": {
                "GET /api/history": "Get transaction history from journal"
            },
            "health": {
                "GET /api/health": "Health check"
            }
        }
    }
 
