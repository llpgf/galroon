"""
Scanner Settings API Endpoints.

Provides endpoints for managing the Sentinel scanner mode and visual scanner progress.
Phase 24.5: Added visual scanner endpoints for progress tracking and control.
"""

from pathlib import Path
from typing import List, Dict, Any

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel

# Global sentinel instance (will be set by the application)
_sentinel_instance = None


# Phase 24.5: Visual scanner progress models
class ScanProgressResponse(BaseModel):
    """Response model for scan progress."""
    stage: str  # idle, scanning, diffing, processing
    current_file: str
    processed_count: int
    total_changes: int
    is_paused: bool
    added_count: int
    modified_count: int
    removed_count: int


class ScanControlResponse(BaseModel):
    """Response model for scan control operations."""
    success: bool
    message: str


class ScannerModeRequest(BaseModel):
    """Request model for setting scanner mode."""
    mode: str  # "realtime", "scheduled", or "manual"
    scheduled_time: str | None = None  # "HH:MM" format, optional


class ScannerModeResponse(BaseModel):
    """Response model for scanner mode queries."""
    mode: str
    is_running: bool
    scheduled_time: str | None = None


class ManualScanResponse(BaseModel):
    """Response model for manual scan trigger."""
    success: bool
    directories_scanned: int
    message: str


router = APIRouter(prefix="/api/settings/scanner", tags=["scanner"])


def set_sentinel_instance(sentinel):
    """
    Set the global Sentinel instance.

    This should be called during application initialization.

    Args:
        sentinel: The Sentinel instance to manage
    """
    global _sentinel_instance
    _sentinel_instance = sentinel


def get_sentinel_instance():
    """
    Get the global Sentinel instance.

    Returns:
        The Sentinel instance

    Raises:
        HTTPException: If sentinel is not initialized
    """
    if _sentinel_instance is None:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="Scanner not initialized"
        )
    return _sentinel_instance


@router.post("/mode", response_model=ScannerModeResponse)
async def set_scanner_mode(request: ScannerModeRequest) -> ScannerModeResponse:
    """
    Set the scanner mode.

    Args:
        request: Scanner mode request with mode and optional scheduled_time

    Returns:
        Current scanner mode and running state

    Modes:
        - realtime: Uses watchdog + Stability Pact (45s debounce) + Coalescing
        - scheduled: Daily scan at specified time (default 03:00 AM)
        - manual: Idle mode, use trigger_scan endpoint for manual scans

    Example:
        POST /api/settings/scanner/mode
        {
            "mode": "realtime"
        }
    """
    from app.core import ScannerMode

    sentinel = get_sentinel_instance()

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

    return ScannerModeResponse(
        mode=sentinel.mode.value,
        is_running=sentinel.is_running(),
        scheduled_time=sentinel.scheduled_time if new_mode == ScannerMode.SCHEDULED else None
    )


@router.get("/mode", response_model=ScannerModeResponse)
async def get_scanner_mode() -> ScannerModeResponse:
    """
    Get the current scanner mode.

    Returns:
        Current scanner mode and running state
    """
    sentinel = get_sentinel_instance()

    return ScannerModeResponse(
        mode=sentinel.mode.value,
        is_running=sentinel.is_running(),
        scheduled_time=sentinel.scheduled_time if sentinel.mode.value == "scheduled" else None
    )


@router.post("/scan", response_model=ManualScanResponse)
async def trigger_manual_scan() -> ManualScanResponse:
    """
    Trigger a manual library scan.

    Can be called in any mode, but primarily useful in MANUAL mode.

    Returns:
        Scan result with directory count and message

    Example:
        POST /api/settings/scanner/scan
    """
    sentinel = get_sentinel_instance()

    try:
        directories = sentinel.trigger_scan()

        return ManualScanResponse(
            success=True,
            directories_scanned=len(directories),
            message=f"Scan completed successfully"
        )
    except Exception as e:
        return ManualScanResponse(
            success=False,
            directories_scanned=0,
            message=f"Scan failed: {str(e)}"
        )


@router.post("/start")
async def start_scanner() -> ScannerModeResponse:
    """
    Start the scanner.

    Returns:
        Current scanner mode and running state

    Example:
        POST /api/settings/scanner/start
    """
    sentinel = get_sentinel_instance()
    sentinel.start()

    return ScannerModeResponse(
        mode=sentinel.mode.value,
        is_running=sentinel.is_running(),
        scheduled_time=sentinel.scheduled_time if sentinel.mode.value == "scheduled" else None
    )


@router.post("/stop")
async def stop_scanner() -> ScannerModeResponse:
    """
    Stop the scanner.

    Returns:
        Current scanner mode and running state

    Example:
        POST /api/settings/scanner/stop
    """
    sentinel = get_sentinel_instance()
    sentinel.stop()

    return ScannerModeResponse(
        mode=sentinel.mode.value,
        is_running=sentinel.is_running(),
        scheduled_time=sentinel.scheduled_time if sentinel.mode.value == "scheduled" else None
    )


# Phase 24.5: Visual Scanner Endpoints

@router.get("/progress", response_model=ScanProgressResponse)
async def get_scan_progress() -> ScanProgressResponse:
    """
    Get current scan progress.

    Returns:
        Current scan progress state including stage, current file, counts

    Example:
        GET /api/settings/scanner/progress
    """
    from ..services.scanner import get_scanner

    scanner = get_scanner()
    progress = scanner.get_progress()

    return ScanProgressResponse(**progress)


@router.post("/pause", response_model=ScanControlResponse)
async def pause_scan() -> ScanControlResponse:
    """
    Pause the current scan.

    Returns:
        Success status and message

    Example:
        POST /api/settings/scanner/pause
    """
    from ..services.scanner import get_scanner

    scanner = get_scanner()

    if not scanner.is_scanning():
        return ScanControlResponse(
            success=False,
            message="No scan is currently running"
        )

    scanner.pause_scan()

    return ScanControlResponse(
        success=True,
        message="Scan paused"
    )


@router.post("/resume", response_model=ScanControlResponse)
async def resume_scan() -> ScanControlResponse:
    """
    Resume the paused scan.

    Returns:
        Success status and message

    Example:
        POST /api/settings/scanner/resume
    """
    from ..services.scanner import get_scanner

    scanner = get_scanner()

    progress = scanner.get_progress()
    if not progress["is_paused"]:
        return ScanControlResponse(
            success=False,
            message="Scan is not paused"
        )

    scanner.resume_scan()

    return ScanControlResponse(
        success=True,
        message="Scan resumed"
    )


@router.post("/cancel", response_model=ScanControlResponse)
async def cancel_scan() -> ScanControlResponse:
    """
    Cancel the current scan.

    Returns:
        Success status and message

    Example:
        POST /api/settings/scanner/cancel
    """
    from ..services.scanner import get_scanner

    scanner = get_scanner()

    if not scanner.is_scanning():
        return ScanControlResponse(
            success=False,
            message="No scan is currently running"
        )

    scanner.stop_scan()

    return ScanControlResponse(
        success=True,
        message="Scan cancelled"
    )
