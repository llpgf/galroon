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
from contextlib import asynccontextmanager
from pathlib import Path


from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.middleware.gzip import GZipMiddleware
from starlette.middleware.base import BaseHTTPMiddleware

# ============================================================================
# Phase 19.8: Security & Hardening (Rate Limiting)
# ============================================================================
from slowapi import _rate_limit_exceeded_handler
from slowapi.errors import RateLimitExceeded
from slowapi.middleware import SlowAPIMiddleware
from .core.rate_limiter import limiter
from .core.config import settings
# ============================================================================

from .core import (
    JournalManager,
    ScannerMode,
    Sentinel,
    Transaction,
    TransactionState,
)
from .models.journal import JournalEntry
from .config import get_config

# ============================================================
# PHASE 26.0: PORTABLE LOGGING
# ============================================================

# Determine portable log path from environment variable
# If running in portable mode, VNITE_LOG_PATH will be set by Electron
PORTABLE_LOG_PATH = settings.VNITE_LOG_PATH
PORTABLE_DATA_PATH = settings.VNITE_DATA_PATH

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

    def on_directories_changed(directories: list[Path]):
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

    # ============================================================================
    # Startup Database View Check (Safety Assertion)
    # ============================================================================
    check_db_views()

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
# Apply rate limiting middleware
app.add_middleware(SlowAPIMiddleware)
# Register 429 error handler
app.add_exception_handler(RateLimitExceeded, _rate_limit_exceeded_handler)
# ============================================================================



# ============================================================================
# PHASE 27.0: API Token Authentication Middleware
# ============================================================================

class TokenAuthMiddleware(BaseHTTPMiddleware):
    """
    Middleware to validate API requests using session token.

    This prevents unauthorized external access to the API.
    Only requests with valid X-Vnite-Token header are allowed.
    """

    def __init__(self, app):
        super().__init__(app)
        # Get configuration
        from .core.config import settings
        
        # Get token from settings
        self.valid_token = settings.SESSION_TOKEN

        # Allow requests without token in development mode
        # (for testing without Electron launcher)
        self.dev_mode = settings.GALGAME_ENV == 'sandbox'

        if self.valid_token:
            logger.info("[PHASE 27.0] Token authentication enabled.")
        elif self.dev_mode:
            logger.warning("[PHASE 27.0] Running in sandbox mode - token authentication DISABLED")
        else:
            logger.error("[PHASE 27.0] SESSION_TOKEN not set - API is insecure!")

    async def dispatch(self, request: Request, call_next):
        # Skip token check for health endpoint (allows monitoring)
        if request.url.path in ["/", "/api/health", "/docs", "/openapi.json"]:
            return await call_next(request)

        # In sandbox/dev mode, skip token validation
        if self.dev_mode:
            return await call_next(request)

        # Check for valid token
        if not self.valid_token:
            logger.warning("[PHASE 27.0] No token configured - rejecting all requests")
            return JSONResponse(
                status_code=500,
                content={"error": "Server misconfiguration: SESSION_TOKEN not set"}
            )

        token = request.headers.get("X-Vnite-Token")

        if not token:
            logger.warning(f"[PHASE 27.0] Request rejected: No token provided (IP: {request.client.host})")
            return JSONResponse(
                status_code=401,
                content={"error": "Unauthorized: Missing X-Vnite-Token header"}
            )

        if token != self.valid_token:
            logger.warning(f"[PHASE 27.0] Request rejected: Invalid token (IP: {request.client.host})")
            return JSONResponse(
                status_code=403,
                content={"error": "Forbidden: Invalid X-Vnite-Token"}
            )

        # Token is valid, proceed
        return await call_next(request)

# Add token authentication middleware
from starlette.responses import JSONResponse
app.add_middleware(TokenAuthMiddleware)

# ============================================================================
# PHASE 20.0: Enable Gzip Compression
# ============================================================================
# Compress API responses to reduce payload size
# Minimum size: 1000 bytes (compress responses larger than 1KB)
app.add_middleware(GZipMiddleware, minimum_size=1000)

# ============================================================================
# PHASE 19.8: Security CORSMiddleware (Must be last/outermost)
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
# PHASE 19.8: Security CORSMiddleware (Must be last/outermost)
# ============================================================================

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
# PHASE 26.0: PORTABLE LOGGING
# ============================================================================

# ============================================================================
# Startup Database View Check (Safety Assertion)
# ============================================================================

def check_db_views():
    """
    Check that required database views exist on startup.

    Raises RuntimeError if library_entry_view is missing.
    This prevents API crashes due to missing views.
    """
    from .core.database import get_database

    try:
        db = get_database()
        with db.get_cursor() as cursor:
            # Check if library_entry_view exists
            cursor.execute(
                "SELECT name FROM sqlite_master WHERE type='view' AND name='library_entry_view'"
            )
            result = cursor.fetchone()

            if not result:
                print("CRITICAL ERROR: 'library_entry_view' is missing in database.")
                print("Please run 'alembic upgrade head' to fix this.")
                # In dev environment, we can raise an error
                # In production, we might want to just log a warning
                raise RuntimeError("Database schema invalid: missing library_entry_view")
            else:
                logger.info("Database view check passed: library_entry_view exists")

    except RuntimeError:
        # Re-raise RuntimeError
        raise
    except Exception as e:
        logger.error(f"Startup database check failed: {e}")
        # Don't block startup for non-critical errors
        pass

# ============================================================================
# PHASE 26.0: PORTABLE LOGGING
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
# PHASE 3: API v1 Router (Modular Routing)
# ============================================================================
# Include all routes under /api/v1 prefix for better versioning
# NOTE: Routers are registered directly in v1/__init__.py (no lazy loading)
from .api import api_v1_router
app.include_router(api_v1_router)

# ============================================================================
# Legacy API Routes (pre-v1)
# ============================================================================
from .api.legacy import router as legacy_router
app.include_router(legacy_router)

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
 
