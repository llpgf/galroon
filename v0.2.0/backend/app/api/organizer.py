"""
Organizer API endpoints for Galgame Library Manager.

**PHASE 9.5: The Curator Workbench**

Provides REST API endpoints for:
- Generating organization proposals
- Reviewing proposals
- Executing proposals
- Rolling back changes
"""

import logging
from pathlib import Path
from typing import Dict, Any, Optional, List

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel, Field

from ..organizer import (
    generate_proposal,
    save_proposal,
    load_proposal,
    execute_plan,
    rollback,
    OrganizationStandard
)
from ..metadata.inventory import AssetDetector

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/organizer", tags=["organizer"])


# ============================================================================
# Pydantic Models
# ============================================================================

class GenerateProposalRequest(BaseModel):
    """Request model for generating organization proposal."""
    source_path: str = Field(..., description="Messy source directory path")
    target_root: str = Field(..., description="Target library root path")
    vndb_metadata: Dict[str, Any] = Field(..., description="VNDB metadata for naming")


class FileMoveResponse(BaseModel):
    """Response model for a file move."""
    source: str
    target: str
    status: str  # safe, unresolved, skip, warning
    category: str
    reason: str
    size: int


class ArchiveGroupResponse(BaseModel):
    """Response model for an archive group."""
    base_name: str
    files: List[FileMoveResponse]
    target_dir: str


class ProposalResponse(BaseModel):
    """Response model for organization proposal."""
    proposal_id: str
    source_path: str
    target_structure: Dict[str, str]
    vndb_metadata: Dict[str, Any]
    moves: List[FileMoveResponse]
    categorized_moves: Dict[str, List[FileMoveResponse]]
    archive_groups: List[ArchiveGroupResponse]
    unresolved_files: List[FileMoveResponse]
    summary: Dict[str, Any]
    created_at: str


class ExecuteProposalRequest(BaseModel):
    """Request model for executing a proposal."""
    proposal: ProposalResponse
    skip_unresolved: bool = Field(True, description="Skip files with UNRESOLVED status")
    cleanup_empty_dirs: bool = Field(True, description="Remove empty source directories")


class ExecuteProposalResponse(BaseModel):
    """Response model for execution result."""
    success: bool
    proposal_id: str
    moved_count: int
    skipped_count: int
    failed_count: int
    errors: List[str]
    undo_log_path: Optional[str]
    created_at: str


class RollbackRequest(BaseModel):
    """Request model for rolling back a proposal."""
    undo_log_path: str = Field(..., description="Path to undo log JSON file")


class RollbackResponse(BaseModel):
    """Response model for rollback result."""
    success: bool
    message: str


class AnalyzeRequest(BaseModel):
    """Request model for analyzing a directory."""
    path: str = Field(..., description="Directory path to analyze")


class AnalyzeResponse(BaseModel):
    """Response model for directory analysis."""
    path: str
    detected_assets: List[str]
    version_label: str
    file_count: int
    matched_patterns: Dict[str, int]


# ============================================================================
# API Endpoints
# ============================================================================

@router.post("/generate", response_model=ProposalResponse)
async def generate_organization_proposal(request: GenerateProposalRequest):
    """
    Generate organization proposal for a messy game directory.

    **READ-ONLY OPERATION** - No files are moved.

    Args:
        request: GenerateProposalRequest with source_path, target_root, vndb_metadata

    Returns:
        ProposalResponse with complete organization plan
    """
    try:
        source_path = Path(request.source_path)
        target_root = Path(request.target_root)

        # Validate paths
        if not source_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Source path not found: {source_path}"
            )

        if not target_root.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Target root not found: {target_root}"
            )

        # Generate proposal
        asset_detector = AssetDetector()
        proposal = generate_proposal(
            source_path=source_path,
            target_root=target_root,
            vndb_metadata=request.vndb_metadata,
            asset_detector=asset_detector
        )

        # Convert to response format
        moves_response = [
            FileMoveResponse(
                source=str(move.source),
                target=str(move.target),
                status=move.status.value,
                category=move.category,
                reason=move.reason,
                size=move.size
            )
            for move in proposal.moves
        ]

        categorized_moves_response = {}
        for category, moves in proposal.categorized_moves.items():
            categorized_moves_response[category] = [
                FileMoveResponse(
                    source=str(move.source),
                    target=str(move.target),
                    status=move.status.value,
                    category=move.category,
                    reason=move.reason,
                    size=move.size
                )
                for move in moves
            ]

        archive_groups_response = [
            ArchiveGroupResponse(
                base_name=group.base_name,
                files=[
                    FileMoveResponse(
                        source=str(f.source),
                        target=str(f.target),
                        status=f.status.value,
                        category=f.category,
                        reason=f.reason,
                        size=f.size
                    )
                    for f in group.files
                ],
                target_dir=group.target_dir
            )
            for group in proposal.archive_groups
        ]

        unresolved_files_response = [
            FileMoveResponse(
                source=str(f.source),
                target=str(f.target),
                status=f.status.value,
                category=f.category,
                reason=f.reason,
                size=f.size
            )
            for f in proposal.unresolved_files
        ]

        target_structure_response = {
            k: str(v) for k, v in proposal.target_structure.items()
        }

        return ProposalResponse(
            proposal_id=proposal.proposal_id,
            source_path=str(proposal.source_path),
            target_structure=target_structure_response,
            vndb_metadata=proposal.vndb_metadata,
            moves=moves_response,
            categorized_moves=categorized_moves_response,
            archive_groups=archive_groups_response,
            unresolved_files=unresolved_files_response,
            summary=proposal.get_summary(),
            created_at=proposal.created_at
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error generating proposal: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error generating proposal: {str(e)}"
        )


@router.post("/execute", response_model=ExecuteProposalResponse)
async def execute_organization_proposal(request: ExecuteProposalRequest):
    """
    Execute an organization proposal.

    **MOVES FILES** - Ensure proposal has been reviewed!

    Args:
        request: ExecuteProposalRequest with proposal and execution options

    Returns:
        ExecuteProposalResponse with execution results
    """
    try:
        # Reconstruct proposal from response
        from ..organizer.proposal import OrganizationProposal, FileMove, MoveStatus

        # Reconstruct moves
        moves = []
        for move_resp in request.proposal.moves:
            move = FileMove(
                source=Path(move_resp.source),
                target=Path(move_resp.target),
                status=MoveStatus(move_resp.status),
                category=move_resp.category,
                reason=move_resp.reason,
                size=move_resp.size,
                checksum=""  # Will be recalculated during execution
            )
            moves.append(move)

        # Reconstruct proposal
        proposal = OrganizationProposal(
            proposal_id=request.proposal.proposal_id,
            source_path=Path(request.proposal.source_path),
            target_structure={
                k: Path(v) for k, v in request.proposal.target_structure.items()
            },
            vndb_metadata=request.proposal.vndb_metadata,
            created_at=request.proposal.created_at
        )
        proposal.moves = moves

        # Execute proposal
        result = execute_plan(
            proposal=proposal,
            skip_unresolved=request.skip_unresolved,
            cleanup_empty_dirs=request.cleanup_empty_dirs
        )

        return ExecuteProposalResponse(
            success=result.success,
            proposal_id=result.proposal_id,
            moved_count=result.moved_count,
            skipped_count=result.skipped_count,
            failed_count=result.failed_count,
            errors=result.errors,
            undo_log_path=str(result.undo_log_path) if result.undo_log_path else None,
            created_at=result.created_at
        )

    except Exception as e:
        logger.error(f"Error executing proposal: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error executing proposal: {str(e)}"
        )


@router.post("/rollback", response_model=RollbackResponse)
async def rollback_organization(request: RollbackRequest):
    """
    Rollback an executed proposal using undo log.

    **WARNING:** This will undo file moves, potentially overwriting existing files.

    Args:
        request: RollbackRequest with undo_log_path

    Returns:
        RollbackResponse with result
    """
    try:
        undo_log_path = Path(request.undo_log_path)

        if not undo_log_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Undo log not found: {undo_log_path}"
            )

        success = rollback(undo_log_path)

        if success:
            return RollbackResponse(
                success=True,
                message="Rollback completed successfully"
            )
        else:
            return RollbackResponse(
                success=False,
                message="Rollback completed with errors (check logs)"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error during rollback: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error during rollback: {str(e)}"
        )


@router.post("/analyze", response_model=AnalyzeResponse)
async def analyze_directory(request: AnalyzeRequest):
    """
    Analyze a directory to detect assets.

    Args:
        request: AnalyzeRequest with directory path

    Returns:
        AnalyzeResponse with detected assets and metadata
    """
    try:
        dir_path = Path(request.path)

        if not dir_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Directory not found: {dir_path}"
            )

        # Detect assets
        asset_detector = AssetDetector()
        detection_result = asset_detector.detect_directory(dir_path)

        return AnalyzeResponse(
            path=str(dir_path),
            detected_assets=detection_result.assets,
            version_label=detection_result.version_label,
            file_count=detection_result.file_count,
            matched_patterns=detection_result.matched_patterns
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error analyzing directory: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error analyzing directory: {str(e)}"
        )


@router.get("/standards")
async def get_organization_standards():
    """
    Get information about the organization standards.

    Returns:
        Dictionary with standard structure information
    """
    return {
        "standard_format": "{Library_Root}/{Developer}/{Year} {Title} [{VNDB_ID}]/",
        "subdirectories": {
            "Game": "Extracted game files and executables",
            "Repository": "ISOs, installers, and archives",
            "Patch_Work": "Patches, cracks, translations",
            "Extras": "OSTs, artbooks, manuals",
            "Metadata": "System metadata and cached images"
        },
        "features": [
            "Scene Standard folder structure",
            "Automatic asset categorization",
            "Split archive grouping",
            "Undo support via rollback",
            "Read-only proposal generation"
        ]
    }
