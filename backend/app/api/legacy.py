"""
Legacy API endpoints migrated from main.py for consistent routing.
"""

import logging
from pathlib import Path
from typing import List, Optional

from fastapi import APIRouter, HTTPException, status, Depends, Request
from pydantic import BaseModel, Field

from ..core import (
    JournalManager,
    ScannerMode,
    Sentinel,
    SmartTrashManager,
    Transaction,
    TransactionError,
    TransactionExecutionError,
    TransactionState,
    TransactionValidationError,
)
from ..core import is_safe_path
from ..models.journal import JournalEntry
from ..metadata import get_batch_manager, get_resource_manager
from ..metadata import get_vndb_provider
from ..metadata import LibraryStatus as PlayStatus
from .dependencies import verify_not_read_only

logger = logging.getLogger(__name__)

router = APIRouter(tags=["legacy"])


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


class ThrowToTrashRequest(BaseModel):
    """Request model for moving items to trash."""
    paths: List[str] = Field(..., description="List of paths to move to trash (relative to library root)")


# ============================================================================
# API Endpoints: Scanner Management
# ============================================================================

@router.post("/api/scan/mode", response_model=ScanStatusResponse)
async def set_scan_mode(
    request: ScanModeRequest,
    http_request: Request,
    _ok: None = Depends(verify_not_read_only())
) -> ScanStatusResponse:
    sentinel: Sentinel = http_request.app.state.sentinel

    try:
        new_mode = ScannerMode(request.mode.lower())
    except ValueError:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Invalid mode: {request.mode}. Must be 'realtime', 'scheduled', or 'manual'"
        )

    if request.scheduled_time:
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

    sentinel.configure(new_mode)
    logger.info(f"Scanner mode switched to {new_mode.value}")

    return ScanStatusResponse(
        mode=sentinel.mode.value,
        is_running=sentinel.is_running(),
        library_root=str(http_request.app.state.library_root),
        scheduled_time=sentinel.scheduled_time if sentinel.mode == ScannerMode.SCHEDULED else None
    )


@router.get("/api/scan/status", response_model=ScanStatusResponse)
async def get_scan_status(http_request: Request) -> ScanStatusResponse:
    sentinel: Sentinel = http_request.app.state.sentinel

    return ScanStatusResponse(
        mode=sentinel.mode.value,
        is_running=sentinel.is_running(),
        library_root=str(http_request.app.state.library_root),
        scheduled_time=sentinel.scheduled_time if sentinel.mode == ScannerMode.SCHEDULED else None
    )


# ============================================================================
# API Endpoints: Library Management
# ============================================================================

@router.get("/api/library/files", response_model=LibraryFilesResponse)
async def list_library_files(http_request: Request, limit: int = 1000) -> LibraryFilesResponse:
    library_root: Path = http_request.app.state.library_root
    files = []

    try:
        for item in library_root.rglob("*"):
            if len(files) >= limit:
                break

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


@router.post("/api/library/organize", response_model=OrganizeResponse)
async def organize_library(
    request: OrganizeRequest,
    http_request: Request,
    _ok: None = Depends(verify_not_read_only())
) -> OrganizeResponse:
    journal: JournalManager = http_request.app.state.journal_manager
    library_root: Path = http_request.app.state.library_root

    valid_ops = ["rename", "mkdir", "copy", "delete"]
    if request.operation not in valid_ops:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Invalid operation: {request.operation}. Must be one of {valid_ops}"
        )

    src = library_root / request.src
    dest = library_root / request.dest if request.dest else None

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

    try:
        tx = Transaction(journal, library_root)
        tx.prepare(request.operation, src, dest)
        tx.commit()

        return OrganizeResponse(
            success=True,
            transaction_id=tx.entry.tx_id,
            message=f"Operation '{request.operation}' completed successfully",
            state=tx.state.value
        )

    except TransactionValidationError as e:
        logger.error(f"Transaction validation failed: {e}")
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Validation failed: {str(e)}"
        )

    except TransactionExecutionError as e:
        logger.error(f"CRITICAL: Transaction execution failed: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Operation failed: {str(e)}"
        )

    except TransactionError as e:
        logger.error(f"Transaction error: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Transaction failed: {str(e)}"
        )

    except Exception as e:
        logger.error(f"Unexpected error during organize: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Operation failed: {str(e)}"
        )


@router.post("/api/library/scan")
async def trigger_manual_scan(http_request: Request) -> dict:
    sentinel: Sentinel = http_request.app.state.sentinel

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
# API Endpoints: Trash Management
# ============================================================================

@router.get("/api/trash/status", response_model=TrashStatusResponse)
async def get_trash_status(http_request: Request) -> TrashStatusResponse:
    config_dir: Path = http_request.app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)
    status_data = trash_manager.get_status()

    return TrashStatusResponse(**status_data)


@router.get("/api/trash/config", response_model=TrashConfigResponse)
async def get_trash_config(http_request: Request) -> TrashConfigResponse:
    config_dir: Path = http_request.app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)

    return TrashConfigResponse(
        max_size_gb=trash_manager.config.max_size_gb,
        retention_days=trash_manager.config.retention_days,
        min_disk_free_gb=trash_manager.config.min_disk_free_gb
    )


@router.post("/api/trash/config", response_model=TrashConfigResponse)
async def update_trash_config(
    request: TrashConfigRequest,
    http_request: Request,
    _ok: None = Depends(verify_not_read_only())
) -> TrashConfigResponse:
    config_dir: Path = http_request.app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)

    updated_config = trash_manager.update_config(
        max_size_gb=request.max_size_gb,
        retention_days=request.retention_days,
        min_disk_free_gb=request.min_disk_free_gb
    )

    logger.info(
        "Trash config updated: max_size=%sGB, retention=%sd, min_free=%sGB",
        updated_config.max_size_gb,
        updated_config.retention_days,
        updated_config.min_disk_free_gb
    )

    return TrashConfigResponse(
        max_size_gb=updated_config.max_size_gb,
        retention_days=updated_config.retention_days,
        min_disk_free_gb=updated_config.min_disk_free_gb
    )


@router.post("/api/trash/empty")
async def empty_trash(
    http_request: Request,
    _ok: None = Depends(verify_not_read_only())
) -> dict:
    config_dir: Path = http_request.app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)

    deleted_count = trash_manager.empty_trash()
    logger.warning(f"Trash emptied by user: {deleted_count} items deleted")

    return {
        "success": True,
        "deleted_count": deleted_count,
        "message": f"Emptied {deleted_count} trash items"
    }


@router.post("/api/trash/throw")
async def throw_to_trash(
    request: ThrowToTrashRequest,
    http_request: Request,
    _ok: None = Depends(verify_not_read_only())
) -> dict:
    config_dir: Path = http_request.app.state.config_dir
    trash_manager = SmartTrashManager(config_dir)
    library_root: Path = http_request.app.state.library_root

    moved = []
    skipped = []

    for rel_path in request.paths:
        target_path = library_root / rel_path
        if not is_safe_path(target_path, library_root):
            skipped.append({"path": rel_path, "reason": "unsafe_path"})
            continue
        if not target_path.exists():
            skipped.append({"path": rel_path, "reason": "not_found"})
            continue

        try:
            trash_manager.ensure_headroom()
            trash_dir = trash_manager.trash_dir / f"manual_{JournalEntry.generate_tx_id()}"
            trash_dir.mkdir(parents=True, exist_ok=True)
            trash_path = trash_dir / target_path.name
            trash_manager.safe_move_to_trash(target_path, trash_path)
            moved.append(rel_path)
        except Exception as e:
            skipped.append({"path": rel_path, "reason": str(e)})

    return {
        "success": True,
        "moved": moved,
        "skipped": skipped
    }


# ============================================================================
# Batch Metadata Scan Endpoints
# ============================================================================

@router.post("/api/metadata/batch/start", response_model=BatchStartResponse)
async def start_batch_scan(payload: BatchStartRequest, http_request: Request) -> BatchStartResponse:
    batch_manager = http_request.app.state.batch_manager

    if batch_manager._provider != payload.provider:
        library_root: Path = http_request.app.state.library_root
        batch_manager.configure(
            library_root=library_root,
            rate_limit=1.0,
            quota_gb=2.0,
            provider=payload.provider
        )

    result = batch_manager.start_scan(
        dry_run=payload.dry_run,
        download_screenshots=payload.download_screenshots,
        prefer_traditional=payload.prefer_traditional,
        targets=payload.targets
    )

    return BatchStartResponse(**result)


@router.post("/api/metadata/batch/pause", response_model=BatchControlResponse)
async def pause_batch_scan(http_request: Request) -> BatchControlResponse:
    batch_manager = http_request.app.state.batch_manager
    result = batch_manager.pause_scan()
    return BatchControlResponse(**result)


@router.post("/api/metadata/batch/resume", response_model=BatchControlResponse)
async def resume_batch_scan(http_request: Request) -> BatchControlResponse:
    batch_manager = http_request.app.state.batch_manager
    result = batch_manager.resume_scan()
    return BatchControlResponse(**result)


@router.post("/api/metadata/batch/stop", response_model=BatchControlResponse)
async def stop_batch_scan(http_request: Request) -> BatchControlResponse:
    batch_manager = http_request.app.state.batch_manager
    result = batch_manager.stop_scan()
    return BatchControlResponse(**result)


@router.get("/api/metadata/batch/status", response_model=BatchStatusResponse)
async def get_batch_status(http_request: Request) -> BatchStatusResponse:
    batch_manager = http_request.app.state.batch_manager
    status_data = batch_manager.get_status()
    return BatchStatusResponse(**status_data)


# ============================================================================
# Metadata Field Lock Endpoints
# ============================================================================

@router.post("/api/metadata/field/lock", response_model=FieldLockResponse)
async def lock_metadata_field(payload: FieldLockRequest, http_request: Request) -> FieldLockResponse:
    library_root: Path = http_request.app.state.library_root
    resource_manager = get_resource_manager(library_root, 2.0)

    game_dir = library_root / payload.game_path
    if not is_safe_path(game_dir, library_root):
        logger.warning(f"Metadata lock rejected unsafe path: {payload.game_path}")
        return FieldLockResponse(
            success=False,
            message="Invalid game path",
            locked=False
        )

    if not game_dir.exists():
        return FieldLockResponse(
            success=False,
            message=f"Game directory not found: {payload.game_path}",
            locked=False
        )

    metadata_dict = resource_manager.load_metadata(game_dir)
    if not metadata_dict:
        return FieldLockResponse(
            success=False,
            message=f"No metadata found for: {payload.game_path}",
            locked=False
        )

    try:
        if payload.field_name not in metadata_dict:
            return FieldLockResponse(
                success=False,
                message=f"Field '{payload.field_name}' not found in metadata",
                locked=False
            )

        field_data = metadata_dict[payload.field_name]

        if isinstance(field_data, dict) and "locked" in field_data:
            field_data["locked"] = payload.lock
        elif isinstance(field_data, dict) and "value" in field_data:
            field_data["locked"] = payload.lock
        else:
            metadata_dict[payload.field_name] = {
                "value": field_data,
                "source": "manual",
                "locked": payload.lock
            }

        success = resource_manager.save_metadata(metadata_dict, game_dir)

        if success:
            action = "locked" if payload.lock else "unlocked"
            return FieldLockResponse(
                success=True,
                message=f"Field '{payload.field_name}' {action}",
                locked=payload.lock
            )
        return FieldLockResponse(
            success=False,
            message="Failed to save metadata",
            locked=not payload.lock
        )

    except Exception as e:
        return FieldLockResponse(
            success=False,
            message=f"Error: {str(e)}",
            locked=False
        )


@router.get("/api/metadata/field/status", response_model=dict)
async def get_field_lock_status(http_request: Request, game_path: str) -> dict:
    library_root: Path = http_request.app.state.library_root
    resource_manager = get_resource_manager(library_root, 2.0)

    game_dir = library_root / game_path
    if not is_safe_path(game_dir, library_root):
        logger.warning(f"Metadata field status rejected unsafe path: {game_path}")
        return {"success": False, "message": "Invalid game path", "fields": {}}

    if not game_dir.exists():
        return {"success": False, "message": f"Game directory not found: {game_path}", "fields": {}}

    metadata_dict = resource_manager.load_metadata(game_dir)
    if not metadata_dict:
        return {"success": False, "message": f"No metadata found for: {game_path}", "fields": {}}

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

    return {"success": True, "game_path": game_path, "fields": field_locks}


# ============================================================================
# Game Metadata Endpoints
# ============================================================================

@router.get("/api/metadata/game/{game_path:path}", response_model=GameMetadataResponse)
async def get_game_metadata(http_request: Request, game_path: str) -> GameMetadataResponse:
    library_root: Path = http_request.app.state.library_root

    game_dir = library_root / game_path
    if not is_safe_path(game_dir, library_root):
        logger.warning(f"Metadata fetch rejected unsafe path: {game_path}")
        return GameMetadataResponse(success=False, metadata=None)

    if not game_dir.exists():
        return GameMetadataResponse(success=False, metadata=None)

    try:
        resource_manager = get_resource_manager(library_root, 2.0)
        metadata_dict = resource_manager.load_metadata(game_dir)

        if not metadata_dict:
            return GameMetadataResponse(success=False, metadata=None)

        return GameMetadataResponse(success=True, metadata=metadata_dict)

    except Exception as e:
        logger.error(f"Error loading metadata for {game_path}: {e}")
        return GameMetadataResponse(success=False, metadata=None)


@router.post("/api/metadata/play_status", response_model=PlayStatusResponse)
async def update_play_status(
    request: PlayStatusRequest,
    http_request: Request,
    _ok: None = Depends(verify_not_read_only())
) -> PlayStatusResponse:
    library_root: Path = http_request.app.state.library_root

    valid_statuses = [status.value for status in PlayStatus]
    if request.play_status not in valid_statuses:
        return PlayStatusResponse(
            success=False,
            message=f"Invalid play status. Must be one of: {', '.join(valid_statuses)}",
            play_status=""
        )

    game_dir = library_root / request.game_path
    if not is_safe_path(game_dir, library_root):
        logger.warning(f"Play status rejected unsafe path: {request.game_path}")
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
            from ..metadata import create_empty_metadata, MetadataField
            metadata = create_empty_metadata()
            metadata.play_status = MetadataField(
                value=PlayStatus(request.play_status),
                source="manual",
                locked=False
            )
            metadata_dict = metadata.model_dump()
        else:
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

        success = resource_manager.save_metadata(metadata_dict, game_dir)

        if success:
            return PlayStatusResponse(
                success=True,
                message=f"Play status updated to '{request.play_status}'",
                play_status=request.play_status
            )

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


@router.post("/api/metadata/apply", response_model=ApplyMetadataResponse)
async def apply_metadata(
    request: ApplyMetadataRequest,
    http_request: Request,
    _ok: None = Depends(verify_not_read_only())
) -> ApplyMetadataResponse:
    library_root: Path = http_request.app.state.library_root

    game_dir = library_root / request.game_path
    if not is_safe_path(game_dir, library_root):
        logger.warning(f"Apply metadata rejected unsafe path: {request.game_path}")
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
        if request.source == "vndb":
            vndb_id = request.match_id if request.match_id.startswith('v') else f'v{request.match_id}'
            vndb_provider = get_vndb_provider(rate_limit=1.0)
            vndb_data = vndb_provider.get_metadata_by_id(vndb_id)

            if not vndb_data:
                return ApplyMetadataResponse(
                    success=False,
                    message=f"Failed to fetch metadata from VNDB for ID: {vndb_id}",
                    metadata=None
                )

            metadata = vndb_provider._parse_vndb_data(vndb_data, prefer_traditional=True)
            metadata_dict = metadata.model_dump()

        elif request.source == "local":
            resource_manager = get_resource_manager(library_root, 2.0)
            metadata_dict = resource_manager.load_metadata(game_dir)

            if not metadata_dict:
                return ApplyMetadataResponse(
                    success=False,
                    message=f"No local metadata found for: {request.game_path}",
                    metadata=None
                )

        elif request.source == "manual":
            vndb_id = request.match_id if request.match_id.startswith('v') else f'v{request.match_id}'
            vndb_provider = get_vndb_provider(rate_limit=1.0)
            vndb_data = vndb_provider.get_metadata_by_id(vndb_id)

            if not vndb_data:
                return ApplyMetadataResponse(
                    success=False,
                    message=f"Failed to fetch metadata from VNDB for ID: {vndb_id}",
                    metadata=None
                )

            metadata = vndb_provider._parse_vndb_data(vndb_data, prefer_traditional=True)
            metadata_dict = metadata.model_dump()

        else:
            return ApplyMetadataResponse(
                success=False,
                message=f"Unknown source: {request.source}",
                metadata=None
            )

        resource_manager = get_resource_manager(library_root, 2.0)
        success = resource_manager.save_metadata(metadata_dict, game_dir)

        if success:
            logger.info(f"Applied metadata from {request.source} to {request.game_path}")
            return ApplyMetadataResponse(
                success=True,
                message=f"Metadata applied from {request.source}",
                metadata=metadata_dict
            )

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
