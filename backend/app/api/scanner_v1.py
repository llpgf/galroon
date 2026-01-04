"""
Scanner API v1 Endpoints

Intelligent file scanning with candidate confirmation workflow.
"""

import logging
import os
from pathlib import Path
from typing import Optional, Dict, Any, List
from datetime import datetime
from fastapi import APIRouter, HTTPException, status, Query, BackgroundTasks
from pydantic import BaseModel, Field
from ..services.scanner import get_scanner
from ..core.database import get_database, ScanCandidate, ScanStatus

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/v1", tags=["Scanner V1"])


# ============================================================================
# Pydantic Request/Response Models
# ============================================================================

class ScanRequest(BaseModel):
    """Request model for starting a scan."""
    path: str = Field(..., description="Directory path to scan (e.g., D:\\\\Games)")
    auto_confirm: bool = Field(default=False, description="Auto-confirm all candidates")


class CandidateResponse(BaseModel):
    """Scan candidate for frontend review."""
    id: int
    path: str
    detected_title: str
    detected_engine: Optional[str] = None
    confidence_score: float
    game_indicators: List[str] = Field(default_factory=list)
    status: str
    detected_at: str
    confirmed_at: Optional[str] = None
    manual_correction: Optional[str] = None


class CandidatesListResponse(BaseModel):
    """Response with all scan candidates."""
    candidates: List[CandidateResponse] = []
    total: int = 0
    auto_confirmed: int = 0


class ScanResponse(BaseModel):
    """Response model for Scan operations."""
    success: bool
    message: str
    status: str = Field(..., description="Current scan status: scan_started, already_scanning, completed, pending_confirmation, error")
    phase: Optional[str] = Field(None, description="Current scan phase")
    candidates: Optional[List[CandidateResponse]] = Field(None, description="Candidates (if status=pending_confirmation)")


class ScanStatusResponse(BaseModel):
    """Response model for scan status."""
    scanning: bool = Field(..., description="True if scan is currently running")
    current: int = Field(0, description="Current progress count")
    total: int = Field(0, description="Total items to process")
    percent_complete: float = Field(0.0, description="Percentage complete")
    message: str = Field("", description="Current status message")
    phase: str = Field("", description="Current scan phase")


# ============================================================================
# API Endpoints
# ============================================================================

@router.post("/library/scan", response_model=ScanResponse)
async def start_scan(
    request: ScanRequest,
    background_tasks: BackgroundTasks
) -> ScanResponse:
    """
    Start a directory Scan with candidate confirmation workflow.
    
    NON-BLOCKING: Returns immediately with 202 Accepted.
    Scan runs in background, generates Candidates.
    
    Args:
        request: Scan request with directory path and auto_confirm option
        
    Returns:
        ScanResponse indicating scan started
    """
    # Validate path exists
    scan_path = Path(request.path)
    
    if not scan_path.exists():
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Directory does not exist: {request.path}"
        )
    
    if not scan_path.is_dir():
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=f"Path is not a directory: {request.path}"
        )
    
    # Security: Resolve to absolute path and check it's safe
    try:
        Scan_path = Scan_path.resolve(strict=True)
    except (FileNotFoundError, PermissionError):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Cannot access directory: {request.path}"
        )
    
    # Get scanner instance
    scanner = get_scanner()
    db = get_database()
    
    # Check if already scanning
    if scanner.is_scanning():
        return ScanResponse(
            success=False,
            message="Scan already in progress",
            status="already_scanning"
        )
    
    logger.info(f"Starting background scan of: {request.path}")
    
    # Start scan in background task (non-blocking)
    async def run_scan():
        result = await scanner.scan_directory(str(scan_path), session=db, auto_confirm=request.auto_confirm)
        logger.info(f"Background scan completed: {result['message']}")
    
    # Add background task
    background_tasks.add_task(run_scan)
    
    return ScanResponse(
        success=True,
        message=f"Scan started for: {request.path}",
        status="scan_started"
    )


@router.get("/library/scan/candidates", response_model=CandidatesListResponse)
async def get_scan_candidates(
    limit: int = Query(100, ge=1, le=500),
    status: Optional[str] = Query(None, description="Filter by status: pending, confirmed, ignored, rejected, merged")
) -> CandidatesListResponse:
    """
    Get all scan candidates for library confirmation.
    
    Args:
        limit: Maximum candidates to return
        status: Optional filter by status
        
    Returns:
        List of scan candidates with metadata
    """
    scanner = get_scanner()
    
    # Prevent get during active scan
    if scanner.is_scanning():
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail="Cannot retrieve candidates during active scan"
        )
    
    db = get_database()
    
    # Parse status filter
    status_filter = None
    if status:
        try:
            status_filter = ScanStatus[status.upper()]
        except ValueError:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"Invalid status: {status}. Must be one of: pending, confirmed, ignored, rejected, merged"
            )
    
    candidates = db.get_scan_candidates(status=status_filter, limit=limit)
    
    # Convert to response models
    candidate_responses = []
    for candidate in candidates:
        candidate_responses.append(CandidateResponse(
            id=candidate.id,
            path=candidate.path,
            detected_title=candidate.detected_title,
            detected_engine=candidate.detected_engine,
            confidence_score=candidate.confidence_score,
            game_indicators=candidate.game_indicators,
            status=candidate.status.value,
            detected_at=candidate.detected_at.isoformat(),
            confirmed_at=candidate.confirmed_at.isoformat() if candidate.confirmed_at else None,
            manual_correction=candidate.manual_correction
        ))
    
    return CandidatesListResponse(
        candidates=candidate_responses,
        total=len(candidates)
    )


@router.put("/library/scan/candidates/{candidate_id}/confirm")
async def confirm_candidate(
    candidate_id: int
) -> dict:
    """
    Confirm a scan candidate and convert to Game.
    
    Args:
        candidate_id: ID of candidate to confirm
        
    Returns:
        Success message
    """
    scanner = get_scanner()
    db = get_database()
    
    # Prevent operations during active scan
    if scanner.is_scanning():
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail="Cannot confirm candidates during active scan"
        )
    
    # Get candidate
    candidate = db.get_scan_candidate_by_id(candidate_id)
    
    if not candidate:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Candidate {candidate_id} not found"
        )
    
    # Update status to confirmed
    success = db.update_candidate_status(candidate_id, ScanStatus.CONFIRMED)
    
    if not success:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Failed to confirm candidate"
        )
    
    # TODO: In future, convert confirmed candidate to Game entity
    # For now, just update status to confirmed
    
    logger.info(f"Confirmed candidate {candidate_id}: {candidate.detected_title}")
    
    return {
        "success": True,
        "message": f"Candidate {candidate_id} confirmed"
    }


@router.delete("/library/scan/candidates/{candidate_id}")
async def delete_candidate(
    candidate_id: int
) -> dict:
    """
    Delete a scan candidate.
    
    Args:
        candidate_id: ID of candidate to delete
        
    Returns:
        Success message
    """
    scanner = get_scanner()
    db = get_database()
    
    # Prevent operations during active scan
    if scanner.is_scanning():
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail="Cannot delete candidates during active scan"
        )
    
    success = db.delete_scan_candidate(candidate_id)
    
    if not success:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Failed to delete candidate"
        )
    
    logger.info(f"Deleted candidate {candidate_id}")
    
    return {
        "success": True,
        "message": f"Candidate {candidate_id} deleted"
    }


@router.get("/library/scan/status", response_model=ScanStatusResponse)
async def get_scan_status() -> ScanStatusResponse:
    """
    Get current scan status with semantic progress.
    
    Returns:
        Current scanning status and progress
    """
    scanner = get_scanner()
    
    return ScanStatusResponse(
        scanning=scanner.is_scanning(),
        current=0,
        total=0,
        percent_complete=0.0,
        message="Ready to Scan" if not scanner.is_scanning() else "Scan in progress",
        phase=""
    )


__all__ = ["router"]
