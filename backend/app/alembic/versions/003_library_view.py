"""Create library_entry_view for CQRS Lite read model

Revision ID: 003_library_view
Revises: 002_canonical_layer
Create Date: 2026-01-05

Sprint 5: CQRS Lite - Read View

This migration creates library_entry_view SQL View that aggregates:
1. Canonical entries: Games linked to canonical entities
2. Suggested clusters: Pending cluster suggestions
3. Orphan entries: Games without canonical link or cluster

The view implements UI ISOLATION:
- Canonical entries show canonical.display_title (not folder path)
- Suggested entries show cluster.suggested_title (not folder path)
- Orphan entries show title (can be folder name)

NO RAW SCANNER PATHS exposed to frontend unless orphan.
"""
from alembic import op
import sqlalchemy as sa

# revision identifiers, used by Alembic.
revision = '003_library_view'
down_revision = '002_canonical_layer'
branch_labels = None
depends_on = None


def upgrade() -> None:
    """Create library_entry_view SQL View."""

    # Drop view if it exists (for idempotent migration)
    op.execute("DROP VIEW IF EXISTS library_entry_view")

    # Create library_entry_view
    op.execute("""
        CREATE VIEW library_entry_view AS
        SELECT
            -- Canonical entries
            cg.id as entry_id,
            'canonical' as entry_type,
            cg.display_title,
            cg.cover_image_url,
            cg.metadata_snapshot as metadata,
            NULL as cluster_id,
            cg.id as canonical_id,
            COUNT(g.folder_path) as instance_count,
            NULL as confidence_score,
            cg.created_at
        FROM canonical_games cg
        LEFT JOIN games g ON g.game_id = cg.id
        GROUP BY cg.id

        UNION ALL

        SELECT
            -- Suggested cluster entries
            mc.id as entry_id,
            'suggested' as entry_type,
            mc.suggested_title as display_title,
            (SELECT cover_image FROM games WHERE folder_path = (
                SELECT instance_path FROM match_cluster_members
                WHERE cluster_id = mc.id AND is_primary = 1 LIMIT 1
            )) as cover_image_url,
            mc.metadata_snapshot as metadata,
            mc.id as cluster_id,
            mc.suggested_canonical_id as canonical_id,
            COUNT(mcm.id) as instance_count,
            mc.confidence_score,
            mc.created_at
        FROM match_clusters mc
        INNER JOIN match_cluster_members mcm ON mcm.cluster_id = mc.id
        WHERE mc.status = 'suggested'
        GROUP BY mc.id

        UNION ALL

        SELECT
            -- Orphan entries (no canonical, no cluster)
            g.folder_path as entry_id,
            'orphan' as entry_type,
            g.title as display_title,
            g.cover_image as cover_image_url,
            json_object(
                'developer', g.developer,
                'library_status', g.library_status,
                'rating', g.rating,
                'release_date', g.release_date,
                'badges', g.badges,
                'tags', g.tags,
                'user_tags', g.user_tags
            ) as metadata,
            NULL as cluster_id,
            NULL as canonical_id,
            1 as instance_count,
            NULL as confidence_score,
            g.created_at
        FROM games g
        WHERE g.game_id IS NULL
          AND NOT EXISTS (
              SELECT 1 FROM match_cluster_members mcm
              WHERE mcm.instance_path = g.folder_path
          )
        ORDER BY display_title COLLATE NOCASE
    """)


def downgrade() -> None:
    """Drop library_entry_view."""

    # Drop the view
    op.execute("DROP VIEW IF EXISTS library_entry_view")
