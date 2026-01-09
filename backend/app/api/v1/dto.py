"""
DTOs (Data Transfer Objects) for Decision Engine API

Sprint 5: Decision Engine - Command & Query Models

This module defines Pydantic models for:
- Commands: Accept/Reject clusters, Detach instances
- Queries: Library view entries (UI isolation)
- Responses: Command execution results
"""

from typing import Dict, Any, List, Optional, Literal
from pydantic import BaseModel, Field
from datetime import datetime


# ============================================================================
# Command DTOs (Decision Engine)
# ============================================================================

class ClusterDecisionCommand(BaseModel):
    """
    Command to accept or reject a suggested cluster.

    Accepting a cluster:
    - Promotes cluster to CanonicalGame
    - Links all instances to the new canonical entity
    - Marks cluster status as 'accepted'

    Rejecting a cluster:
    - Marks cluster status as 'rejected'
    - Instances remain as orphans (no canonical link)
    """

    decision: Literal['accept', 'reject'] = Field(
        ...,
        description="Decision: 'accept' to promote to canonical, 'reject' to dissolve"
    )
    custom_title: Optional[str] = Field(
        None,
        description="Optional: override suggested title when accepting"
    )


class DetachInstanceCommand(BaseModel):
    """
    Command to detach an instance from its canonical entity.

    This removes the game_id FK from the LocalInstance (games table),
    converting it to an orphan that can be re-clustered.

    Note: No "Undo Canonical" operation - only detach is supported.
    """

    instance_path: str = Field(
        ...,
        description="Path to the LocalInstance to detach (games.folder_path)"
    )


# ============================================================================
# Query DTOs (Read Views - UI Isolation)
# ============================================================================

LibraryEntryType = Literal['canonical', 'suggested', 'orphan']


class LibraryEntry(BaseModel):
    """
    Unified library entry for UI (CQRS Lite - Read Model).

    This DTO enforces UI isolation: NO raw scanner paths unless orphan.
    Canonical and suggested entries show canonical/cluster titles, not folder paths.

    Attributes:
        entry_id: Unique identifier (depends on type)
        entry_type: Type of this entry
        display_title: Human-readable title (never raw path)
        cover_image_url: Cover image (from canonical or primary instance)
        metadata: UI-safe metadata (no raw paths)
        cluster_id: For 'suggested' entries
        canonical_id: For 'canonical' entries
        instance_count: Number of instances in this entry
        confidence_score: For 'suggested' entries
        created_at: Creation timestamp
    """

    entry_id: str = Field(..., description="Unique entry ID")
    entry_type: LibraryEntryType = Field(..., description="Type of this entry")
    display_title: str = Field(..., description="Human-readable title (never raw path)")
    cover_image_url: Optional[str] = Field(None, description="Cover image URL or path")
    metadata: Dict[str, Any] = Field(default_factory=dict, description="UI-safe metadata")
    cluster_id: Optional[str] = Field(None, description="Cluster ID (for suggested entries)")
    canonical_id: Optional[str] = Field(None, description="Canonical ID (for canonical entries)")
    instance_count: int = Field(1, description="Number of instances in this entry")
    confidence_score: Optional[float] = Field(None, description="Confidence (for suggested entries)")
    created_at: Optional[datetime] = Field(None, description="Creation timestamp")


class LibraryListResponse(BaseModel):
    """
    Response model for library list endpoint.

    Aggregates all three entry types in a single, UI-safe view.
    """

    entries: List[LibraryEntry] = Field(..., description="All library entries")
    total: int = Field(..., description="Total number of entries")


# ============================================================================
# Response DTOs (Command Results)
# ============================================================================

class ClusterDecisionResponse(BaseModel):
    """
    Response after accepting or rejecting a cluster.

    Accepting:
    - canonical_id: ID of newly created CanonicalGame
    - affected_instances: List of instance paths now linked

    Rejecting:
    - canonical_id: None (no canonical created)
    - affected_instances: List of instance paths remaining as orphans
    """

    cluster_id: str = Field(..., description="Cluster ID that was decided")
    decision: Literal['accept', 'reject'] = Field(..., description="Decision made")
    canonical_id: Optional[str] = Field(None, description="Canonical ID created (if accepted)")
    affected_instances: List[str] = Field(..., description="Instance paths affected")
    status: str = Field(..., description="Operation status")


class DetachInstanceResponse(BaseModel):
    """
    Response after detaching an instance from canonical.

    The instance becomes an orphan and can be re-clustered.
    """

    instance_path: str = Field(..., description="Instance that was detached")
    previous_canonical_id: str = Field(..., description="Previous canonical ID")
    status: str = Field(..., description="Operation status")


class ClusterDetailResponse(BaseModel):
    """
    Detailed view of a suggested cluster.

    Shows all instances in the cluster with match scores.
    """

    cluster_id: str = Field(..., description="Cluster ID")
    status: Literal['suggested', 'accepted', 'rejected'] = Field(..., description="Cluster status")
    suggested_title: str = Field(..., description="Suggested canonical title")
    confidence_score: float = Field(..., description="Cluster confidence score")
    suggested_canonical_id: Optional[str] = Field(None, description="Suggested canonical ID")
    instances: List[Dict[str, Any]] = Field(..., description="Cluster members with match scores")
    metadata: Optional[Dict[str, Any]] = Field(None, description="Cluster metadata")
    created_at: Optional[datetime] = Field(None, description="Creation timestamp")


# ============================================================================
# Canonical Detail DTOs
# ============================================================================

class CanonicalGameDetail(BaseModel):
    """
    Detailed view of a canonical game.

    Shows canonical entity with all linked instances and external sources.
    """

    id: str = Field(..., description="Canonical ID")
    display_title: str = Field(..., description="Canonical display title")
    metadata_snapshot: Dict[str, Any] = Field(..., description="Canonical metadata")
    cover_image_url: Optional[str] = Field(None, description="Canonical cover image")
    identity_links: List[Dict[str, Any]] = Field(default_factory=list, description="External source links")
    linked_instances: List[Dict[str, Any]] = Field(default_factory=list, description="Linked LocalInstances")
    created_at: Optional[datetime] = Field(None, description="Creation timestamp")
    updated_at: Optional[datetime] = Field(None, description="Last update timestamp")


__all__ = [
    # Commands
    "ClusterDecisionCommand",
    "DetachInstanceCommand",

    # Queries
    "LibraryEntryType",
    "LibraryEntry",
    "LibraryListResponse",

    # Responses
    "ClusterDecisionResponse",
    "DetachInstanceResponse",
    "ClusterDetailResponse",

    # Canonical Details
    "CanonicalGameDetail",
]
