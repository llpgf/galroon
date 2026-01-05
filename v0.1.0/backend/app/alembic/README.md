# Alembic Migration Scripts

This directory contains the database migration scripts.

## Migration Files

Migrations are timestamped and ordered chronologically. Each migration file contains:
- `upgrade()` function: Apply the migration
- `downgrade()` function: Revert the migration

## Usage

```bash
# Create a new migration
alembic revision -m "description"

# Apply all pending migrations
alembic upgrade head

# Revert the last migration
alembic downgrade -1

# Show current version
alembic current

# Show migration history
alembic history
```

## Versioning

Migrations use SQLAlchemy revision IDs for version tracking.
The `alembic_version` table in the database tracks the current state.
