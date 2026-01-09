"""${message}

Revision ID: ${up_revision}
Revises:
Create Date: ${create_date}

Phase 20.0: Initial schema migration - mark current database schema as base version.

This migration creates the initial schema for the Galroon library database:
- games table with flattened metadata
- Indexes for fast sorting
- FTS5 virtual table for full-text search
- Triggers to keep FTS5 in sync

The actual schema creation is handled by the Database class in core/database.py.
This migration script marks that state as the base version for future migrations.

"""
from alembic import op
import sqlalchemy as sa
import sqlite3

# revision identifiers, used by Alembic.
revision = '001_initial_schema'
down_revision = None
branch_labels = None
depends_on = None


def upgrade() -> None:
    """
    Mark current database schema as base version.

    The Database class in core/database.py already creates all tables with:
    - CREATE TABLE IF NOT EXISTS
    - CREATE INDEX IF NOT EXISTS
    - CREATE VIRTUAL TABLE IF NOT EXISTS

    This migration simply records the fact that this is the initial schema state.
    Future migrations will use this as their base.
    """
    # Note: We don't create tables here because Database._init_db()
    # uses CREATE TABLE IF NOT EXISTS, which is idempotent.
    # This migration marks the current schema state as version '001_initial_schema'.

    # Create alembic_version table if it doesn't exist
    # (Alembic normally does this, but we're being explicit)
    conn = op.get_bind()
    inspector = sa.inspect(conn)

    # Check if alembic_version table exists
    existing_tables = inspector.get_table_names()
    if 'alembic_version' not in existing_tables:
        op.execute("""
            CREATE TABLE alembic_version (
                version_num VARCHAR(32) NOT NULL
            )
        """)

        # Insert our revision
        op.execute("INSERT INTO alembic_version (version_num) VALUES ('001_initial_schema')")


def downgrade() -> None:
    """
    Revert to pre-migration state.

    In practice, this would drop all tables, but we want to preserve data.
    Instead, we just remove the alembic_version tracking.
    """
    # Remove alembic_version table to stop tracking
    # (This essentially reverts to a non-Alembic managed state)
    op.execute("DROP TABLE IF EXISTS alembic_version")
