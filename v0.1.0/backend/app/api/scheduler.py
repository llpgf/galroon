"""
Scheduler API Endpoints.

Provides endpoints for managing the background task scheduler.
Phase 24.5: System Governance - Scheduled task management
"""

from typing import Dict, Any
from datetime import datetime

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel

from ..services.scheduler import get_scheduler


class SchedulerConfigResponse(BaseModel):
    """Response model for scheduler configuration."""
    scan_interval_min: int
    is_running: bool


class JobInfoResponse(BaseModel):
    """Response model for job information."""
    jobs: Dict[str, Dict[str, Any]]


class ScanIntervalRequest(BaseModel):
    """Request model for updating scan interval."""
    interval_min: int


class TriggerScanResponse(BaseModel):
    """Response model for triggering manual scan."""
    success: bool
    message: str


router = APIRouter(prefix="/api/settings/scheduler", tags=["scheduler"])


@router.get("/status", response_model=SchedulerConfigResponse)
async def get_scheduler_status() -> SchedulerConfigResponse:
    """
    Get scheduler status and configuration.

    Returns:
        Current scheduler configuration

    Example:
        GET /api/settings/scheduler/status
    """
    from ..config import get_config

    config = get_config()
    scheduler = get_scheduler()

    return SchedulerConfigResponse(
        scan_interval_min=config.scan_interval_min,
        is_running=scheduler.scheduler.running
    )


@router.get("/jobs", response_model=JobInfoResponse)
async def get_scheduled_jobs() -> JobInfoResponse:
    """
    Get all scheduled jobs and their next run times.

    Returns:
        Dict of jobs with details

    Example:
        GET /api/settings/scheduler/jobs
    """
    scheduler = get_scheduler()

    jobs = scheduler.get_job_info()

    return JobInfoResponse(jobs=jobs)


@router.post("/interval", response_model=SchedulerConfigResponse)
async def update_scan_interval(request: ScanIntervalRequest) -> SchedulerConfigResponse:
    """
    Update library scan interval.

    Args:
        request: New interval in minutes (0 = manual mode)

    Returns:
        Updated scheduler configuration

    Example:
        POST /api/settings/scheduler/interval
        {
            "interval_min": 60
        }
    """
    from ..config import get_config

    if request.interval_min < 0:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="Interval must be >= 0 (0 = manual mode)"
        )

    scheduler = get_scheduler()
    scheduler.update_scan_interval(request.interval_min)

    config = get_config()

    return SchedulerConfigResponse(
        scan_interval_min=config.scan_interval_min,
        is_running=scheduler.scheduler.running
    )


@router.post("/trigger", response_model=TriggerScanResponse)
async def trigger_manual_scan() -> TriggerScanResponse:
    """
    Trigger a manual library scan immediately.

    Returns:
        Success status and message

    Example:
        POST /api/settings/scheduler/trigger
    """
    scheduler = get_scheduler()

    if not scheduler.scheduler.running:
        return TriggerScanResponse(
            success=False,
            message="Scheduler is not running"
        )

    scheduler.trigger_scan_now()

    return TriggerScanResponse(
        success=True,
        message="Library scan triggered"
    )
