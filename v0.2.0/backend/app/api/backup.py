"""
Backup API Endpoints.

Provides endpoints for managing backups (Time Machine).
Phase 24.5: System Governance - Backup/restore management
"""

from typing import List, Dict, Any
from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel

from ..services.backup import get_backup_manager, BackupMetadata


class BackupResponse(BaseModel):
    """Response model for backup creation."""
    filename: str
    created_at: str
    size_bytes: int
    size_mb: float
    version: str


class BackupListResponse(BaseModel):
    """Response model for backup list."""
    backups: List[BackupResponse]


class RestoreRequest(BaseModel):
    """Request model for restore operation."""
    filename: str


class RestoreResponse(BaseModel):
    """Response model for restore operation."""
    success: bool
    message: str


class DeleteBackupResponse(BaseModel):
    """Response model for delete operation."""
    success: bool
    message: str


class BackupStatsResponse(BaseModel):
    """Response model for backup statistics."""
    total_backups: int
    total_size_bytes: int
    total_size_mb: float
    max_backups: int
    oldest_backup: str | None
    newest_backup: str | None


class SetMaxBackupsRequest(BaseModel):
    """Request model for setting max backups."""
    max_backups: int


router = APIRouter(prefix="/api/settings/backup", tags=["backup"])


@router.post("/create", response_model=BackupResponse)
async def create_backup() -> BackupResponse:
    """
    Create a new backup.

    Returns:
        Backup metadata

    Example:
        POST /api/settings/backup/create
    """
    backup_manager = get_backup_manager()

    try:
        backup_meta = backup_manager.create_backup()

        return BackupResponse(
            filename=backup_meta.filename,
            created_at=backup_meta.created_at,
            size_bytes=backup_meta.size_bytes,
            size_mb=backup_meta.size_mb,
            version=backup_meta.version
        )
    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to create backup: {str(e)}"
        )


@router.get("/list", response_model=BackupListResponse)
async def list_backups() -> BackupListResponse:
    """
    List all backups.

    Returns:
        List of backups sorted by creation time (newest first)

    Example:
        GET /api/settings/backup/list
    """
    backup_manager = get_backup_manager()

    backups = backup_manager.list_backups()

    return BackupListResponse(
        backups=[
            BackupResponse(
                filename=b.filename,
                created_at=b.created_at,
                size_bytes=b.size_bytes,
                size_mb=b.size_mb,
                version=b.version
            )
            for b in backups
        ]
    )


@router.post("/restore", response_model=RestoreResponse)
async def restore_backup(request: RestoreRequest) -> RestoreResponse:
    """
    Restore a backup.

    Args:
        request: Restore request with filename

    Returns:
        Success status and message

    Example:
        POST /api/settings/backup/restore
        {
            "filename": "backup_20240103_120000.zip"
        }
    """
    backup_manager = get_backup_manager()

    try:
        success = backup_manager.restore_backup(request.filename)

        if success:
            return RestoreResponse(
                success=True,
                message=f"Backup restored successfully: {request.filename}"
            )
        else:
            return RestoreResponse(
                success=False,
                message="Restore failed"
            )
    except FileNotFoundError:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Backup not found: {request.filename}"
        )
    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to restore backup: {str(e)}"
        )


@router.delete("/{filename}", response_model=DeleteBackupResponse)
async def delete_backup(filename: str) -> DeleteBackupResponse:
    """
    Delete a backup.

    Args:
        filename: Name of backup file to delete

    Returns:
        Success status and message

    Example:
        DELETE /api/settings/backup/backup_20240103_120000.zip
    """
    backup_manager = get_backup_manager()

    try:
        success = backup_manager.delete_backup(filename)

        if success:
            return DeleteBackupResponse(
                success=True,
                message=f"Backup deleted: {filename}"
            )
        else:
            return DeleteBackupResponse(
                success=False,
                message=f"Backup not found: {filename}"
            )
    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to delete backup: {str(e)}"
        )


@router.get("/stats", response_model=BackupStatsResponse)
async def get_backup_stats() -> BackupStatsResponse:
    """
    Get backup statistics.

    Returns:
        Backup statistics

    Example:
        GET /api/settings/backup/stats
    """
    backup_manager = get_backup_manager()

    stats = backup_manager.get_backup_stats()

    return BackupStatsResponse(**stats)


@router.post("/max-backups", response_model=BackupStatsResponse)
async def set_max_backups(request: SetMaxBackupsRequest) -> BackupStatsResponse:
    """
    Set maximum number of backups to keep.

    Args:
        request: Max backups setting (1-100)

    Returns:
        Updated backup statistics

    Example:
        POST /api/settings/backup/max-backups
        {
            "max_backups": 10
        }
    """
    if not (1 <= request.max_backups <= 100):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="max_backups must be between 1 and 100"
        )

    backup_manager = get_backup_manager()

    backup_manager.set_max_backups(request.max_backups)

    stats = backup_manager.get_backup_stats()

    return BackupStatsResponse(**stats)
