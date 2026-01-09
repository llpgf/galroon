"""create_library_entry_view

Revision ID: 004_library_entry_view_fix
Revises: 003_library_view
Create Date: 2026-01-05

Sprint 5: CQRS Lite - Read View (Fixed)

This migration creates library_entry_view SQL View that aggregates:
1. Canonical entries: Games linked to canonical entities
2. Suggested clusters: Pending cluster suggestions
3. Orphan entries: Games without canonical link or cluster

Schema Reference:
- games table: folder_path (PK), game_id (FK to canonical_games), title, cover_image
- canonical_games table: id (PK), display_title, cover_image_url
- match_clusters table: id (PK), status, suggested_title, confidence_score
- match_cluster_members table: id (PK), cluster_id (FK), instance_path (games.folder_path)
"""
from alembic import op

# revision identifiers, used by Alembic.
revision = '004_library_entry_view_fix'
down_revision = '003_library_view'
branch_labels = None
depends_on = None


def upgrade() -> None:
    """Create library_entry_view SQL View."""

    # Drop view if it exists (for idempotent migration)
    op.execute("DROP VIEW IF EXISTS library_entry_view")

    # Create library_entry_view
    # This view provides a unified read model for the Library UI
    op.execute("""
        CREATE VIEW library_entry_view AS

        -- PART 1: Canonical entries (games linked to canonical_games)
        SELECT
            'canonical:' || cg.id AS entry_id,
            'canonical' AS entry_type,
            cg.display_title AS display_title,
            cg.cover_image_url AS cover_image_url,
            cg.metadata_snapshot AS metadata,
            NULL AS cluster_id,
            cg.id AS canonical_id,
            COUNT(g.folder_path) AS instance_count,
            NULL AS confidence_score,
            cg.created_at AS created_at
        FROM canonical_games cg
        LEFT JOIN games g ON g.game_id = cg.id
        GROUP BY cg.id

        UNION ALL

        -- PART 2: Suggested cluster entries (pending user decision)
        SELECT
            'cluster:' || mc.id AS entry_id,
            'suggested' AS entry_type,
            mc.suggested_title AS display_title,
            (SELECT g2.cover_image FROM games g2 WHERE g2.folder_path = (
                SELECT mcm2.instance_path FROM match_cluster_members mcm2
                WHERE mcm2.cluster_id = mc.id AND mcm2.is_primary = 1 LIMIT 1
            )) AS cover_image_url,
            mc.metadata_snapshot AS metadata,
            mc.id AS cluster_id,
            mc.suggested_canonical_id AS canonical_id,
            COUNT(mcm.id) AS instance_count,
            mc.confidence_score AS confidence_score,
            mc.created_at AS created_at
        FROM match_clusters mc
        INNER JOIN match_cluster_members mcm ON mcm.cluster_id = mc.id
        WHERE mc.status = 'suggested'
        GROUP BY mc.id

        UNION ALL

        -- PART 3: Orphan entries (no canonical link, not in any cluster)
        SELECT
            'orphan:' || g.folder_path AS entry_id,
            'orphan' AS entry_type,
            g.title AS display_title,
            g.cover_image AS cover_image_url,
            json_object(
                'developer', g.developer,
                'library_status', g.library_status,
                'rating', g.rating,
                'release_date', g.release_date,
                'badges', g.badges,
                'tags', g.tags,
                'user_tags', g.user_tags,
                'vndb_id', g.vndb_id
            ) AS metadata,
            NULL AS cluster_id,
            NULL AS canonical_id,
            1 AS instance_count,
            NULL AS confidence_score,
            g.created_at AS created_at
        FROM games g
        WHERE g.game_id IS NULL
          AND NOT EXISTS (
              SELECT 1 FROM match_cluster_members mcm
              WHERE mcm.instance_path = g.folder_path
          )

        ORDER BY created_at DESC
    """)


def downgrade() -> None:
    """Drop library_entry_view."""

    # Drop view
    op.execute("DROP VIEW IF EXISTS library_entry_view")
