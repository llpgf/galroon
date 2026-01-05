"""
Canonical Data Models for Galroon v0.2.0

Sprint 4 & 5: Canonical Layer & MatchCluster Architecture

Architecture Overview:
- CanonicalGame: The "single source of truth" for a game entity
- IdentityLink: Links CanonicalGame to external sources (VNDB, Bangumi, etc.)
- LocalInstance: Scan-detected game folders, linked to CanonicalGame

This implements Roon-style knowledge management with truth layer separation.
"""

import json
from typing import Dict, Any, List, Optional
from dataclasses import dataclass, asdict


@dataclass
class CanonicalGame:
    """
    Canonical Game Entity - The "single source of truth".

    This represents the canonical identity of a game, independent of file locations.
    Multiple LocalInstances can point to the same CanonicalGame.

    Attributes:
        id: Unique canonical ID (UUID)
        display_title: Canonical display title (user-editable)
        metadata_snapshot: JSON blob of aggregated metadata from all sources
        cover_image_url: URL or path to canonical cover image
        created_at: Creation timestamp
        updated_at: Last update timestamp
    """

    id: str
    display_title: str
    metadata_snapshot: Dict[str, Any]
    cover_image_url: Optional[str] = None
    created_at: str = ""
    updated_at: str = ""

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for database storage."""
        return asdict(self)

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'CanonicalGame':
        """Create instance from dictionary."""
        if isinstance(data.get('metadata_snapshot'), str):
            data['metadata_snapshot'] = json.loads(data['metadata_snapshot'])
        return cls(**data)


@dataclass
class IdentityLink:
    """
    Link between CanonicalGame and external sources.

    Establishes provenance links to authoritative sources (VNDB, Bangumi, etc.).
    One CanonicalGame can have multiple IdentityLinks.

    Attributes:
        id: Unique link ID
        canonical_id: Foreign key to CanonicalGame.id
        source_type: External source type ('vndb', 'bangumi', 'steam', etc.)
        external_id: ID in the external source system
        external_url: Full URL to the external source page
        metadata_snapshot: JSON blob of source-specific metadata
        created_at: Creation timestamp
    """

    id: str
    canonical_id: str
    source_type: str
    external_id: str
    external_url: str
    metadata_snapshot: Optional[Dict[str, Any]] = None
    created_at: str = ""

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for database storage."""
        return asdict(self)

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'IdentityLink':
        """Create instance from dictionary."""
        if isinstance(data.get('metadata_snapshot'), str):
            data['metadata_snapshot'] = json.loads(data['metadata_snapshot'])
        return cls(**data)


# Table definitions for Database class
CANONICAL_GAMES_TABLE_SQL = """
    CREATE TABLE IF NOT EXISTS canonical_games (
        id TEXT PRIMARY KEY,
        display_title TEXT NOT NULL,
        metadata_snapshot TEXT NOT NULL,  -- JSON blob
        cover_image_url TEXT,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
"""

IDENTITY_LINKS_TABLE_SQL = """
    CREATE TABLE IF NOT EXISTS identity_links (
        id TEXT PRIMARY KEY,
        canonical_id TEXT NOT NULL,
        source_type TEXT NOT NULL,  -- 'vndb', 'bangumi', 'steam', etc.
        external_id TEXT NOT NULL,
        external_url TEXT,
        metadata_snapshot TEXT,  -- JSON blob
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (canonical_id) REFERENCES canonical_games(id) ON DELETE CASCADE
    )
"""

# Indexes for canonical tables
CANONICAL_INDEXES_SQL = [
    "CREATE INDEX IF NOT EXISTS idx_canonical_display_title ON canonical_games(display_title COLLATE NOCASE)",
    "CREATE INDEX IF NOT EXISTS idx_identity_canonical_id ON identity_links(canonical_id)",
    "CREATE INDEX IF NOT EXISTS idx_identity_source_type ON identity_links(source_type)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_identity_unique_source ON identity_links(source_type, external_id)",
]

__all__ = [
    "CanonicalGame",
    "IdentityLink",
    "CANONICAL_GAMES_TABLE_SQL",
    "IDENTITY_LINKS_TABLE_SQL",
    "CANONICAL_INDEXES_SQL",
]
