"""
Alembic Environment Configuration

This file is configured by Alembic to run migrations.
It configures the database connection and migration context.

Phase 20.0: Integration with existing Database class
"""

from logging.config import fileConfig
from sqlalchemy import engine_from_config
from sqlalchemy import pool
import sys
from pathlib import Path

# Add app directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from alembic import context
from app.core.database import Database
from app.config import get_config

# Interpret the config file for Python logging.
if context.config.config_file_name is not None:
    fileConfig(context.config.config_file_name)

# Set target_metadata to None for non-SQLAlchemy migrations
# We use raw SQL for SQLite schema
target_metadata = None


def run_migrations_online() -> None:
    """
    Run migrations in 'online' mode.

    In this scenario we need to create an Engine
    and associate a connection with the context.
    """
    # Get database path from config
    config = get_config()
    db_path = config.config_dir / "library.db"

    # Create database instance to initialize schema if needed
    db = Database(db_path)

    # Get connection for Alembic
    # We use SQLite's execute for migrations
    import sqlite3
    connection = sqlite3.connect(db_path)

    context.configure(
        connection=connection,
        target_metadata=target_metadata,
    )

    with context.begin_transaction():
        context.run_migrations()


def run_migrations_offline() -> None:
    """
    Run migrations in 'offline' mode.

    This configures the context with just a URL
    and not an Engine, though an Engine is acceptable
    here as well.  By skipping the Engine creation
    we don't even need a DBAPI to be available.
    """
    url = "sqlite:///"  # Placeholder - not used in offline mode

    context.configure(
        url=url,
        target_metadata=target_metadata,
        literal_binds=True,
        dialect_opts={"paramstyle": "named"},
    )

    with context.begin_transaction():
        context.run_migrations()


if context.is_offline_mode():
    run_migrations_offline()
else:
    run_migrations_online()
