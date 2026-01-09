"""Sprint 9: Tags Table Migration

Revision ID: 005_tags_table
Revises: 004_library_entry_view_fix
Create Date: 2026-01-05

Creates:
- tags table: Global tag registry
- game_tags table: Many-to-many relationship
"""
from alembic import op
import sqlalchemy as sa
import logging

# revision identifiers, used by Alembic.
revision = '005_tags_table'
down_revision = '004_library_entry_view_fix'
branch_labels = None
depends_on = None

logger = logging.getLogger(__name__)

def upgrade() -> None:
    """
    Create tags and game_tags tables.
    """
    # Create tags table
    op.execute("""
        CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            color TEXT DEFAULT '#8B5CF6',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    """)
    
    # Create game_tags junction table
    op.execute("""
        CREATE TABLE IF NOT EXISTS game_tags (
            game_id TEXT NOT NULL,
            tag_id TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (game_id, tag_id),
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        )
    """)
    
    # Create indexes for performance
    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_game_tags_game_id ON game_tags(game_id)
    """)
    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_game_tags_tag_id ON game_tags(tag_id)
    """)
    op.execute("""
        CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name)
    """)
    
    logger.info("Sprint 9: Created tags and game_tags tables")


def downgrade() -> None:
    """
    Drop tags and game_tags tables.
    """
    op.execute("DROP TABLE IF EXISTS game_tags")
    op.execute("DROP TABLE IF EXISTS tags")
    logger.info("Sprint 9: Dropped tags and game_tags tables")
