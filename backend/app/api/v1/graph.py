"""
Graph Discovery API - Sprint 11: The Discovery Lens
Knowledge Graph for curated games with D3-compatible format.

Constitutional Red Lines:
- Only is_curated=true data
- Read-only (no edit operations)
- No inference (explicit relationships only)
"""

from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel
from typing import List, Optional, Dict, Any
import logging
import json
import time
from functools import lru_cache
from datetime import datetime

from ...core.database import get_db, Database

logger = logging.getLogger(__name__)
router = APIRouter(prefix="/graph", tags=["graph"])


# ============================================================================
# In-Memory Cache (TTL 5 minutes)
# ============================================================================

class GraphCache:
    def __init__(self, ttl_seconds: int = 300):
        self.ttl = ttl_seconds
        self.data: Optional[Dict] = None
        self.timestamp: float = 0
    
    def is_valid(self) -> bool:
        return self.data is not None and (time.time() - self.timestamp) < self.ttl
    
    def set(self, data: Dict):
        self.data = data
        self.timestamp = time.time()
    
    def get(self) -> Optional[Dict]:
        if self.is_valid():
            return self.data
        return None
    
    def invalidate(self):
        self.data = None
        self.timestamp = 0


# Global cache instance
_graph_cache = GraphCache(ttl_seconds=300)


# ============================================================================
# Response Models (D3-Compatible)
# ============================================================================

class GraphNode(BaseModel):
    id: str
    label: str
    type: str  # 'game', 'staff', 'developer'
    img: Optional[str] = None
    role: Optional[str] = None  # For staff nodes
    metadata: Dict[str, Any] = {}


class GraphLink(BaseModel):
    source: str
    target: str
    value: int = 1  # Link weight


class DiscoveryGraph(BaseModel):
    nodes: List[GraphNode]
    links: List[GraphLink]
    stats: Dict[str, int]


# ============================================================================
# API Endpoints
# ============================================================================

@router.get("/discovery", response_model=DiscoveryGraph)
async def get_discovery_graph(
    force_refresh: bool = False,
    db: Database = Depends(get_db)
):
    """
    Generate D3-compatible discovery graph from curated games only.
    
    Args:
        force_refresh: Bypass cache and regenerate graph
    
    Returns:
        DiscoveryGraph with nodes (games, staff, developers) and links
    """
    global _graph_cache
    
    # Check cache first
    if not force_refresh:
        cached = _graph_cache.get()
        if cached:
            logger.info("[Graph] Returning cached graph data")
            return DiscoveryGraph(**cached)
    
    logger.info("[Graph] Building discovery graph from database...")
    
    nodes: List[GraphNode] = []
    links: List[GraphLink] = []
    seen_nodes: set = set()
    
    stats = {
        "games": 0,
        "developers": 0,
        "staff": 0,
        "links": 0
    }

    try:
        # Fetch only curated canonical games (Constitutional Red Line)
        rows = db.conn.execute("""
            SELECT id, display_title, metadata_snapshot, cover_image_url
            FROM canonical_games
            WHERE is_curated = 1
        """).fetchall()

        for row in rows:
            game_id = f"game_{row['id']}"
            title = row['display_title']
            cover = row['cover_image_url']
            metadata = row['metadata_snapshot'] or {}

            # Parse metadata if stored as string
            if isinstance(metadata, str):
                try:
                    metadata = json.loads(metadata)
                except:
                    metadata = {}

            # Add Game node
            if game_id not in seen_nodes:
                nodes.append(GraphNode(
                    id=game_id,
                    label=title,
                    type="game",
                    img=cover,
                    metadata={"vndb_id": metadata.get("vndb_id")}
                ))
                seen_nodes.add(game_id)
                stats["games"] += 1

            # Extract Developer (explicit relationship only)
            developer = metadata.get("developer")
            if developer and isinstance(developer, str):
                dev_id = f"dev_{developer.lower().replace(' ', '_').replace('.', '')}"
                if dev_id not in seen_nodes:
                    nodes.append(GraphNode(
                        id=dev_id,
                        label=developer,
                        type="developer",
                        metadata={}
                    ))
                    seen_nodes.add(dev_id)
                    stats["developers"] += 1
                
                links.append(GraphLink(
                    source=dev_id,
                    target=game_id,
                    value=1
                ))
                stats["links"] += 1

            # Extract Staff (explicit relationships only - no inference)
            staff_list = metadata.get("staff", [])
            for staff_entry in staff_list:
                if isinstance(staff_entry, dict):
                    staff_name = staff_entry.get("name")
                    staff_role = staff_entry.get("role", "unknown")
                    
                    if staff_name and isinstance(staff_name, str):
                        staff_id = f"staff_{staff_name.lower().replace(' ', '_').replace('.', '')}"
                        if staff_id not in seen_nodes:
                            nodes.append(GraphNode(
                                id=staff_id,
                                label=staff_name,
                                type="staff",
                                role=staff_role,
                                metadata={"primary_role": staff_role}
                            ))
                            seen_nodes.add(staff_id)
                            stats["staff"] += 1
                        
                        links.append(GraphLink(
                            source=staff_id,
                            target=game_id,
                            value=1
                        ))
                        stats["links"] += 1

        # Build result
        result = {
            "nodes": [n.dict() for n in nodes],
            "links": [l.dict() for l in links],
            "stats": stats
        }
        
        # Cache the result
        _graph_cache.set(result)
        
        logger.info(f"[Graph] Built graph: {stats['games']} games, {stats['staff']} staff, {stats['links']} links")
        
        return DiscoveryGraph(**result)

    except Exception as e:
        logger.error(f"Graph discovery failed: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/discovery/stats")
async def get_discovery_stats(db: Database = Depends(get_db)):
    """
    Get statistics about the discovery graph without building full graph.
    """
    try:
        curated = db.conn.execute(
            "SELECT COUNT(*) as count FROM canonical_games WHERE is_curated = 1"
        ).fetchone()['count']
        
        total = db.conn.execute(
            "SELECT COUNT(*) as count FROM canonical_games"
        ).fetchone()['count']
        
        return {
            "curated_games": curated,
            "total_games": total,
            "curation_rate": round(curated / total * 100, 1) if total > 0 else 0,
            "cache_valid": _graph_cache.is_valid(),
            "cache_age_seconds": int(time.time() - _graph_cache.timestamp) if _graph_cache.timestamp else None
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/discovery/invalidate")
async def invalidate_cache():
    """
    Invalidate the graph cache. Useful after curation changes.
    """
    global _graph_cache
    _graph_cache.invalidate()
    return {"success": True, "message": "Graph cache invalidated"}
