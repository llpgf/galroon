"""
Canonicalization API v1 Endpoints

Sprint 4: Turns accepted IdentityMatchCandidates into Canonical Entities.

POST /api/v1/matches/{match_id}/canonicalize
- Strict algorithm with full provenance tracking
- Returns canonical_game_id and all created entity IDs
"""

import logging
from typing import Optional, Dict, Any
from fastapi import APIRouter, HTTPException, status, BackgroundTasks
from pydantic import BaseModel, Field
from datetime import datetime

from ..services.canonicalization.service import get_canonicalization_service
from ..core.database import get_database, CanonicalizationResult

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/api/v1", tags=["Canonicalization V1"])


# ============================================================================
# Pydantic Request/Response Models
# ============================================================================

class CanonicalizationRequest(BaseModel):
    """Request for canonicalization (optional)."""
    # Future: Add parameters for manual corrections or merges
    pass


class CanonicalizationResponse(BaseModel):
    """Response from canonicalization operation."""
    success: bool
    message: str
    canonical_game_id: Optional[int] = None
    canonical_company_ids: list[int] = Field(default_factory=list)
    canonical_person_ids: list[int] = Field(default_factory=list)
    canonical_character_ids: list[int] = Field(default_factory=list)
    canonical_link_ids: list[int] = Field(default_factory=list)
    source_links_created: int = 0


# ============================================================================
# API Endpoints
# ============================================================================

@router.post("/matches/{match_id:int}/canonicalize")
async def canonicalize_match(
    match_id: int,
    background_tasks: BackgroundTasks
) -> CanonicalizationResponse:
    """
    Turn an accepted IdentityMatchCandidate into canonical entities.
    
    STRICT ALGORITHM:
    - STEP 0: Preconditions (fail fast if invalid)
    - STEP 1: Lock ScanCandidate to prevent concurrent ops
    - STEP 2: Canonical Game Resolution (check VNDB ID, reuse OR create)
    - STEP 3: Canonical Company/Person/Role Resolution
    - STEP 4: Create CanonicalSourceLink for game
    - STEP 5: Canonical Character Resolution
    - STEP 6: Create CanonicalSourceLinks for all other entities
    - STEP 7: Final state transitions (match.status=canonicalized, scan_candidate.status=merged)
    - STEP 8: Background task for async DB operations
    - Returns canonical_game_id immediately
    """
    db = get_database()
    canonicalization_service = get_canonicalization_service()
    
    logger.info(f"Starting canonicalization of match {match_id}")
    
    # Run canonicalization in background task (non-blocking)
    async def run_canonicalization():
        result = await canonicalization_service.canonicalize_match(match_id, session=db)
        logger.info(f"Canonicalization completed: {result}")
        return result
    
    # Add background task
    background_tasks.add_task(run_canonicalization)
    
    return CanonicalizationResponse(
        success=True,
        message=f"Canonicalization started for match {match_id}",
        canonical_game_id=result.canonical_game_id,
        canonical_company_ids=result.canonical_company_ids,
        canonical_person_ids=result.canonical_person_ids,
        canonical_character_ids=result.canonical_character_ids,
        canonical_link_ids=result.source_links_created
    )


__all__ = ["router"]
