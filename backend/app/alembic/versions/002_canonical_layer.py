"""Add Canonical Layer and MatchCluster tables

Revision ID: 002_canonical_layer
Revises: 001_initial_schema
Create Date: 2026-01-05

Sprint 4 & 5: Canonical Layer & MatchCluster Architecture

This migration adds:
- CanonicalGames table: Single source of truth for game entities
- IdentityLinks table: Links to external sources (VNDB, Bangumi, etc.)
- MatchClusters table: Suggested groupings awaiting user decision
- MatchClusterMembers table: Links LocalInstances to clusters
- Update games table: Add game_id FK and scan_signature

This implements Roon-style knowledge management with truth layer separation.
"""
from alembic import op
import sqlalchemy as sa
from pathlib import Path

# revision identifiers, used by Alembic.
revision = '002_canonical_layer'
down_revision = '001_initial_schema'
branch_labels = None
depends_on = None


def upgrade() -> None:
    """Create canonical layer and match cluster tables."""

    # 1. Create canonical_games table
    op.execute("""
        CREATE TABLE IF NOT EXISTS canonical_games (
            id TEXT PRIMARY KEY,
            display_title TEXT NOT NULL,
            metadata_snapshot TEXT NOT NULL,
            cover_image_url TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    """)

    # 2. Create identity_links table
    op.execute("""
        CREATE TABLE IF NOT EXISTS identity_links (
            id TEXT PRIMARY KEY,
            canonical_id TEXT NOT NULL,
            source_type TEXT NOT NULL,
            external_id TEXT NOT NULL,
            external_url TEXT,
            metadata_snapshot TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (canonical_id) REFERENCES canonical_games(id) ON DELETE CASCADE
        )
    """)

    # 3. Create match_clusters table
    op.execute("""
        CREATE TABLE IF NOT EXISTS match_clusters (
            id TEXT PRIMARY KEY,
            status TEXT NOT NULL CHECK(status IN ('suggested', 'accepted', 'rejected')),
            confidence_score REAL NOT NULL CHECK(confidence_score >= 0.0 AND confidence_score <= 1.0),
            suggested_title TEXT NOT NULL,
            suggested_canonical_id TEXT,
            metadata_snapshot TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (suggested_canonical_id) REFERENCES canonical_games(id) ON DELETE SET NULL
        )
    """)

    # 4. Create match_cluster_members table
    op.execute("""
        CREATE TABLE IF NOT EXISTS match_cluster_members (
            id TEXT PRIMARY KEY,
            cluster_id TEXT NOT NULL,
            instance_path TEXT NOT NULL,
            match_score REAL NOT NULL CHECK(match_score >= 0.0 AND match_score <= 1.0),
            is_primary BOOLEAN NOT NULL DEFAULT 0,
            metadata_snapshot TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (cluster_id) REFERENCES match_clusters(id) ON DELETE CASCADE,
            UNIQUE(cluster_id, instance_path)
        )
    """)

    # 5. Add columns to games table (LocalInstance)
    # First check if columns exist (idempotent migration)
    conn = op.get_bind()
    inspector = sa.inspect(conn)

    # Check existing columns in games table
    existing_columns = [col['name'] for col in inspector.get_columns('games')]

    # Add game_id FK if not exists
    if 'game_id' not in existing_columns:
        op.execute("""
            ALTER TABLE games ADD COLUMN game_id TEXT
        """)
        op.execute("""
            CREATE INDEX IF NOT EXISTS idx_games_game_id ON games(game_id)
        """)
        # Add foreign key constraint
        op.execute("""
            CREATE TRIGGER IF NOT EXISTS fk_games_game_id
            BEFORE INSERT ON games BEGIN
                SELECT CASE
                    WHEN new.game_id IS NOT NULL AND
                         (SELECT COUNT(*) FROM canonical_games WHERE id = new.game_id) = 0
                    THEN RAISE(ABORT, 'Foreign key violation: game_id')
                END;
            END
        """)
        op.execute("""
            CREATE TRIGGER IF NOT EXISTS fk_games_game_id_update
            BEFORE UPDATE OF game_id ON games BEGIN
                SELECT CASE
                    WHEN new.game_id IS NOT NULL AND
                         (SELECT COUNT(*) FROM canonical_games WHERE id = new.game_id) = 0
                    THEN RAISE(ABORT, 'Foreign key violation: game_id')
                END;
            END
        """)

    # Add scan_signature if not exists
    if 'scan_signature' not in existing_columns:
        op.execute("""
            ALTER TABLE games ADD COLUMN scan_signature TEXT
        """)
        op.execute("""
            CREATE INDEX IF NOT EXISTS idx_games_scan_signature ON games(scan_signature)
        """)

    # 6. Create indexes for canonical tables
    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_canonical_display_title
        ON canonical_games(display_title COLLATE NOCASE)
    """)

    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_identity_canonical_id
        ON identity_links(canonical_id)
    """)

    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_identity_source_type
        ON identity_links(source_type)
    """)

    op.execute("""
        CREATE UNIQUE INDEX IF NOT EXISTS idx_identity_unique_source
        ON identity_links(source_type, external_id)
    """)

    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_match_cluster_status
        ON match_clusters(status)
    """)

    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_match_cluster_confidence
        ON match_clusters(confidence_score DESC)
    """)

    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_match_cluster_canonical_id
        ON match_clusters(suggested_canonical_id)
    """)

    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_match_member_cluster_id
        ON match_cluster_members(cluster_id)
    """)

    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_match_member_instance_path
        ON match_cluster_members(instance_path)
    """)

    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_match_member_primary
        ON match_cluster_members(is_primary)
    """)


def downgrade() -> None:
    """Revert canonical layer and match cluster tables."""

    # Drop tables in reverse order of creation (to respect FK constraints)
    op.execute("DROP TABLE IF EXISTS match_cluster_members")
    op.execute("DROP TABLE IF EXISTS match_clusters")
    op.execute("DROP TABLE IF EXISTS identity_links")
    op.execute("DROP TABLE IF EXISTS canonical_games")

    # Drop added columns from games table
    conn = op.get_bind()
    inspector = sa.inspect(conn)

    # Note: SQLite doesn't support DROP COLUMN directly
    # In practice, we'd need to recreate the table, but for this migration
    # we'll keep the columns to preserve data

    # Drop triggers for game_id FK
    op.execute("DROP TRIGGER IF EXISTS fk_games_game_id")
    op.execute("DROP TRIGGER IF EXISTS fk_games_game_id_update")

    # Drop indexes
    op.execute("DROP INDEX IF EXISTS idx_games_game_id")
    op.execute("DROP INDEX IF EXISTS idx_games_scan_signature")
    op.execute("DROP INDEX IF EXISTS idx_canonical_display_title")
    op.execute("DROP INDEX IF EXISTS idx_identity_canonical_id")
    op.execute("DROP INDEX IF EXISTS idx_identity_source_type")
    op.execute("DROP INDEX IF EXISTS idx_identity_unique_source")
    op.execute("DROP INDEX IF EXISTS idx_match_cluster_status")
    op.execute("DROP INDEX IF EXISTS idx_match_cluster_confidence")
    op.execute("DROP INDEX IF EXISTS idx_match_cluster_canonical_id")
    op.execute("DROP INDEX IF EXISTS idx_match_member_cluster_id")
    op.execute("DROP INDEX IF EXISTS idx_match_member_instance_path")
    op.execute("DROP INDEX IF EXISTS idx_match_member_primary")
