"""
Decision Engine API for Galroon v0.2.0

Sprint 5: Decision Engine - Command API

Architecture:
- Command endpoints for accepting/rejecting clusters
- Promote clusters to CanonicalGame (truth layer)
- Detach instances from canonical (no undo, only detach)

This implements CQRS Lite: Separate Command API from Query API.
"""

import logging
import json
import uuid
from typing import Dict, Any, List, Optional
from datetime import datetime

from fastapi import APIRouter, HTTPException, Depends
from pydantic import BaseModel

from ...core.database import get_database
from .dto import (
    ClusterDecisionCommand,
    DetachInstanceCommand,
    ClusterDecisionResponse,
    DetachInstanceResponse,
    ClusterDetailResponse,
    CanonicalGameDetail,
)

logger = logging.getLogger(__name__)

# Create router (prefix added by parent api_v1_router)
router = APIRouter(tags=["decisions"])


# ============================================================================
# Cluster Decision Endpoints
# ============================================================================

@router.post("/clusters/{cluster_id}/commit", response_model=ClusterDecisionResponse)
async def commit_cluster_decision(
    cluster_id: str,
    command: ClusterDecisionCommand,
    db = Depends(get_database)
):
    """
    Accept or reject a suggested cluster.

    **Accept Decision:**
    - Promotes cluster to CanonicalGame
    - Links all instances to new canonical entity
    - Marks cluster status as 'accepted'

    **Reject Decision:**
    - Marks cluster status as 'rejected'
    - Instances remain as orphans (no canonical link)

    Args:
        cluster_id: ID of cluster to decide on
        command: Decision command ('accept' or 'reject')

    Returns:
        ClusterDecisionResponse with operation details
    """
    logger.info(f"Cluster decision: {command.decision} for cluster {cluster_id}")

    # Get cluster details
    cluster = _get_cluster(db, cluster_id)
    if not cluster:
        raise HTTPException(status_code=404, detail=f"Cluster {cluster_id} not found")

    # Get cluster members
    members = _get_cluster_members(db, cluster_id)

    # Execute decision
    if command.decision == 'accept':
        return await _accept_cluster(db, cluster, members, command.custom_title)
    else:
        return await _reject_cluster(db, cluster, members)


@router.get("/clusters/{cluster_id}", response_model=ClusterDetailResponse)
async def get_cluster_detail(
    cluster_id: str,
    db = Depends(get_database)
):
    """
    Get detailed view of a suggested cluster.

    Shows all instances in cluster with match scores and metadata.

    Args:
        cluster_id: ID of cluster

    Returns:
        ClusterDetailResponse with cluster details
    """
    cluster = _get_cluster(db, cluster_id)
    if not cluster:
        raise HTTPException(status_code=404, detail=f"Cluster {cluster_id} not found")

    members = _get_cluster_members(db, cluster_id)

    return ClusterDetailResponse(
        cluster_id=cluster['id'],
        status=cluster['status'],
        suggested_title=cluster['suggested_title'],
        confidence_score=cluster['confidence_score'],
        suggested_canonical_id=cluster.get('suggested_canonical_id'),
        instances=members,
        metadata=json.loads(cluster.get('metadata_snapshot') or '{}'),
        created_at=cluster.get('created_at')
    )


# ============================================================================
# Canonical Detach Endpoint (No Undo)
# ============================================================================

@router.post("/canonical/{canonical_id}/detach", response_model=DetachInstanceResponse)
async def detach_instance(
    canonical_id: str,
    command: DetachInstanceCommand,
    db = Depends(get_database)
):
    """
    Detach an instance from its canonical entity.

    This removes the game_id FK from LocalInstance (games table),
    converting it to an orphan that can be re-clustered.

    **IMPORTANT**: No "Undo Canonical" operation - only detach is supported.

    Args:
        canonical_id: ID of canonical entity
        command: Detach command with instance path

    Returns:
        DetachInstanceResponse with operation details
    """
    logger.info(f"Detaching instance {command.instance_path} from canonical {canonical_id}")

    # Validate canonical exists
    canonical = _get_canonical(db, canonical_id)
    if not canonical:
        raise HTTPException(status_code=404, detail=f"Canonical {canonical_id} not found")

    # Validate instance exists and is linked to this canonical
    with db.get_cursor() as cursor:
        cursor.execute(
            "SELECT folder_path FROM games WHERE folder_path = ? AND game_id = ?",
            (command.instance_path, canonical_id)
        )
        instance = cursor.fetchone()

        if not instance:
            raise HTTPException(
                status_code=400,
                detail=f"Instance {command.instance_path} not linked to canonical {canonical_id}"
            )

        # Detach: Set game_id to NULL
        cursor.execute(
            "UPDATE games SET game_id = NULL, updated_at = CURRENT_TIMESTAMP WHERE folder_path = ?",
            (command.instance_path,)
        )

        # Verify update succeeded
        if cursor.rowcount == 0:
            raise HTTPException(
                status_code=500,
                detail=f"Failed to detach instance {command.instance_path}"
            )

    logger.info(f"Detached instance {command.instance_path} from canonical {canonical_id}")

    return DetachInstanceResponse(
        instance_path=command.instance_path,
        previous_canonical_id=canonical_id,
        status="detached"
    )


@router.get("/canonical/{canonical_id}", response_model=CanonicalGameDetail)
async def get_canonical_detail(
    canonical_id: str,
    db = Depends(get_database)
):
    """
    Get detailed view of a canonical game.

    Shows canonical entity with all linked instances and external sources.

    Args:
        canonical_id: ID of canonical entity

    Returns:
        CanonicalGameDetail with canonical details
    """
    canonical = _get_canonical(db, canonical_id)
    if not canonical:
        raise HTTPException(status_code=404, detail=f"Canonical {canonical_id} not found")

    # Get identity links
    identity_links = _get_identity_links(db, canonical_id)

    # Get linked instances
    linked_instances = _get_linked_instances(db, canonical_id)

    return CanonicalGameDetail(
        id=canonical['id'],
        display_title=canonical['display_title'],
        metadata_snapshot=json.loads(canonical['metadata_snapshot']),
        cover_image_url=canonical.get('cover_image_url'),
        identity_links=identity_links,
        linked_instances=linked_instances,
        created_at=canonical.get('created_at'),
        updated_at=canonical.get('updated_at')
    )


# ============================================================================
# Decision Engine Logic (Private Functions)
# ============================================================================

async def _accept_cluster(
    db,
    cluster: Dict[str, Any],
    members: List[Dict[str, Any]],
    custom_title: Optional[str] = None
) -> ClusterDecisionResponse:
    """
    Promote cluster to CanonicalGame.

    Steps:
    1. Create CanonicalGame with cluster metadata
    2. Create IdentityLink if external ID present
    3. Link all instances to new canonical entity
    4. Mark cluster status as 'accepted'
    """
    logger.info(f"Promoting cluster {cluster['id']} to canonical")

    # Generate canonical ID
    canonical_id = str(uuid.uuid4())

    # Determine title
    display_title = custom_title or cluster['suggested_title']

    # Aggregate metadata from cluster members
    metadata_snapshot = _aggregate_metadata_from_cluster(cluster, members)

    # Create CanonicalGame - wrapped in transaction
    with db.get_cursor() as cursor:
        try:
            cursor.execute("""
                INSERT INTO canonical_games (id, display_title, metadata_snapshot, cover_image_url)
                VALUES (?, ?, ?, ?)
            """, (
                canonical_id,
                display_title,
                json.dumps(metadata_snapshot),
                _get_cover_image_from_members(members)
            ))

            # If cluster suggests existing canonical, create identity link
            if cluster.get('suggested_canonical_id'):
                cursor.execute("""
                    INSERT INTO identity_links (id, canonical_id, source_type, external_id, external_url)
                    VALUES (?, ?, ?, ?, ?)
                """, (
                    str(uuid.uuid4()),
                    canonical_id,
                    'canonical_merge',
                    cluster['suggested_canonical_id'],
                    f"/api/v1/canonical/{cluster['suggested_canonical_id']}"
                ))

            # Link all instances to canonical
            for member in members:
                cursor.execute("""
                    UPDATE games SET game_id = ?, updated_at = CURRENT_TIMESTAMP WHERE folder_path = ?
                """, (canonical_id, member['instance_path']))

            # Update cluster status
            cursor.execute("""
                UPDATE match_clusters
                SET status = 'accepted', updated_at = CURRENT_TIMESTAMP
                WHERE id = ?
            """, (cluster['id'],))

        except Exception as e:
            # Transaction will be rolled back by get_cursor context manager
            logger.error(f"Failed to accept cluster: {e}")
            raise HTTPException(
                status_code=500,
                detail=f"Failed to accept cluster: {str(e)}"
            )

    logger.info(f"Promoted cluster {cluster['id']} to canonical {canonical_id}")

    return ClusterDecisionResponse(
        cluster_id=cluster['id'],
        decision='accept',
        canonical_id=canonical_id,
        affected_instances=[m['instance_path'] for m in members],
        status="promoted_to_canonical"
    )


async def _reject_cluster(
    db,
    cluster: Dict[str, Any],
    members: List[Dict[str, Any]]
) -> ClusterDecisionResponse:
    """
    Dissolve cluster (reject suggestion).

    Steps:
    1. Mark cluster status as 'rejected'
    2. Instances remain as orphans (no canonical link)

    Note: This is NOT "undo canonical" - it's only for rejecting suggested clusters.
    """
    logger.info(f"Rejecting cluster {cluster['id']}")

    with db.get_cursor() as cursor:
        # Update cluster status
        cursor.execute("""
            UPDATE match_clusters
            SET status = 'rejected', updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
        """, (cluster['id'],))

    logger.info(f"Rejected cluster {cluster['id']}")

    return ClusterDecisionResponse(
        cluster_id=cluster['id'],
        decision='reject',
        canonical_id=None,
        affected_instances=[m['instance_path'] for m in members],
        status="cluster_rejected"
    )


# ============================================================================
# Database Query Helpers
# ============================================================================

def _get_cluster(db, cluster_id: str) -> Optional[Dict[str, Any]]:
    """Get cluster by ID."""
    with db.get_cursor() as cursor:
        cursor.execute("SELECT * FROM match_clusters WHERE id = ?", (cluster_id,))
        row = cursor.fetchone()
        return dict(row) if row else None


def _get_cluster_members(db, cluster_id: str) -> List[Dict[str, Any]]:
    """Get all members of a cluster."""
    with db.get_cursor() as cursor:
        cursor.execute("""
            SELECT * FROM match_cluster_members
            WHERE cluster_id = ?
            ORDER BY is_primary DESC, match_score DESC
        """, (cluster_id,))
        return [dict(row) for row in cursor.fetchall()]


def _get_canonical(db, canonical_id: str) -> Optional[Dict[str, Any]]:
    """Get canonical game by ID."""
    with db.get_cursor() as cursor:
        cursor.execute("SELECT * FROM canonical_games WHERE id = ?", (canonical_id,))
        row = cursor.fetchone()
        return dict(row) if row else None


def _get_identity_links(db, canonical_id: str) -> List[Dict[str, Any]]:
    """Get all identity links for a canonical game."""
    with db.get_cursor() as cursor:
        cursor.execute("""
            SELECT * FROM identity_links
            WHERE canonical_id = ?
            ORDER BY created_at DESC
        """, (canonical_id,))
        return [dict(row) for row in cursor.fetchall()]


def _get_linked_instances(db, canonical_id: str) -> List[Dict[str, Any]]:
    """Get all instances linked to a canonical game."""
    with db.get_cursor() as cursor:
        cursor.execute("""
            SELECT folder_path, title, cover_image, library_status, rating, badges
            FROM games
            WHERE game_id = ?
            ORDER BY title COLLATE NOCASE
        """, (canonical_id,))
        results = cursor.fetchall()
        instances = []
        for row in results:
            inst = dict(row)
            if inst.get('badges'):
                inst['badges'] = json.loads(inst['badges'])
            instances.append(inst)
        return instances


def _aggregate_metadata_from_cluster(
    cluster: Dict[str, Any],
    members: List[Dict[str, Any]]
) -> Dict[str, Any]:
    """Aggregate metadata from cluster and its members."""
    cluster_metadata = json.loads(cluster.get('metadata_snapshot') or '{}')

    # Get primary instance metadata
    primary_member = next((m for m in members if m.get('is_primary')), None)

    aggregated = {
        'cluster_id': cluster['id'],
        'confidence_score': cluster['confidence_score'],
        'cluster_metadata': cluster_metadata,
    }

    if primary_member:
        member_metadata = json.loads(primary_member.get('metadata_snapshot') or '{}')
        aggregated.update(member_metadata)

    return aggregated


def _get_cover_image_from_members(members: List[Dict[str, Any]]) -> Optional[str]:
    """Get cover image from primary member or any member."""
    # Try primary member first
    primary = next((m for m in members if m.get('is_primary')), None)
    if primary:
        member_metadata = json.loads(primary.get('metadata_snapshot') or '{}')
        if 'cover_image' in member_metadata:
            return member_metadata['cover_image']

    # Fallback: any member with cover
    for member in members:
        member_metadata = json.loads(member.get('metadata_snapshot') or '{}')
        if 'cover_image' in member_metadata:
            return member_metadata['cover_image']

    return None
