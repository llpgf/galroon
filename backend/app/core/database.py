"""
SQLite Database Core for Galgame Library Manager.

Phase 20.0: The Instant Index - Achieve "Everything-like" instant search performance.

Architecture:
- SQLite with FTS5 (Full-Text Search) for instant text search
- WAL mode for high concurrency (readers don't block writers)
- Flattened metadata schema for fast sorting/filtering
- Zero-latency reads: All queries hit DB, not filesystem
"""

import logging
import sqlite3
import threading
from pathlib import Path
from typing import Dict, Any, List, Optional, Tuple
from contextlib import contextmanager

logger = logging.getLogger(__name__)


class Database:
    """
    SQLite database manager with FTS5 full-text search.

    Thread-safe singleton pattern with connection pooling.
    """

    def __init__(self, db_path: Path):
        """
        Initialize database.

        Args:
            db_path: Path to SQLite database file
        """
        self.db_path = db_path
        self.local = threading.local()
        self._init_db()

    def _get_connection(self) -> sqlite3.Connection:
        """
        Get thread-local database connection.

        Returns:
            SQLite connection for current thread
        """
        if not hasattr(self.local, 'conn') or self.local.conn is None:
            self.local.conn = sqlite3.connect(
                self.db_path,
                check_same_thread=False,
                timeout=30.0
            )
            # Enable WAL mode for better concurrency
            self.local.conn.execute("PRAGMA journal_mode=WAL;")
            # Performance optimizations
            self.local.conn.execute("PRAGMA synchronous=NORMAL;")
            self.local.conn.execute("PRAGMA cache_size=-64000;")  # 64MB cache
            self.local.conn.row_factory = sqlite3.Row  # Return dict-like rows

        return self.local.conn

    @contextmanager
    def get_cursor(self):
        """
        Context manager for database cursor.

        Yields:
            SQLite cursor
        """
        conn = self._get_connection()
        cursor = conn.cursor()
        try:
            yield cursor
            conn.commit()
        except Exception as e:
            conn.rollback()
            logger.error(f"Database error: {e}")
            raise

    def _init_db(self):
        """
        Initialize database schema.

        Creates:
        - games table: Flattened metadata for fast queries
        - games_fts table: FTS5 virtual table for full-text search
        """
        # Ensure parent directory exists
        self.db_path.parent.mkdir(parents=True, exist_ok=True)

        logger.info(f"Initializing database at: {self.db_path}")

        with self.get_cursor() as cursor:
            # Main games table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS games (
                    folder_path TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    developer TEXT,
                    cover_image TEXT,
                    library_status TEXT DEFAULT 'unstarted',
                    rating REAL,
                    release_date TEXT,
                    badges TEXT,  -- JSON array: ['ISO', 'DLC', 'Patch']
                    tags TEXT,    -- JSON array: provider tags
                    user_tags TEXT,  -- JSON array: user-defined tags
                    folder_mtime REAL,  -- For fast diff
                    json_mtime REAL,    -- For fast diff
                    vndb_id TEXT,
                    external_ids TEXT,  -- JSON object
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)

            # Indexes for fast sorting
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_title
                ON games(title COLLATE NOCASE)
            """)

            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_library_status
                ON games(library_status)
            """)

            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_rating
                ON games(rating DESC)
            """)

            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_release_date
                ON games(release_date)
            """)

            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_folder_mtime
                ON games(folder_mtime)
            """)

            # FTS5 virtual table for full-text search
            # Indexed on title, developer, and tags
            # NOTE: Using contentless FTS5 table with manual sync via triggers
            cursor.execute("""
                CREATE VIRTUAL TABLE IF NOT EXISTS games_fts
                USING fts5(
                    folder_path UNINDEXED,
                    title,
                    developer,
                    tags,
                    tokenize='porter unicode61'
                )
            """)

            # FTS5 triggers to keep search index in sync
            cursor.execute("""
                CREATE TRIGGER IF NOT EXISTS games_ai
                AFTER INSERT ON games BEGIN
                    INSERT INTO games_fts(folder_path, title, developer, tags)
                    VALUES (new.folder_path, new.title, new.developer, new.tags);
                END
            """)

            cursor.execute("""
                CREATE TRIGGER IF NOT EXISTS games_ad
                AFTER DELETE ON games BEGIN
                    DELETE FROM games_fts WHERE folder_path = old.folder_path;
                END
            """)

            cursor.execute("""
                CREATE TRIGGER IF NOT EXISTS games_au
                AFTER UPDATE ON games BEGIN
                    DELETE FROM games_fts WHERE folder_path = old.folder_path;
                    INSERT INTO games_fts(folder_path, title, developer, tags)
                    VALUES (new.folder_path, new.title, new.developer, new.tags);
                END
            """)

            # Sprint 9: Tags table for global tag registry
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS tags (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    color TEXT DEFAULT '#8B5CF6',
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)

            # Sprint 9: Game-Tag junction table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS game_tags (
                    game_id TEXT NOT NULL,
                    tag_id TEXT NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (game_id, tag_id),
                    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
                )
            """)

            # Sprint 9: Indexes for game_tags
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_game_tags_game_id ON game_tags(game_id)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_game_tags_tag_id ON game_tags(tag_id)
            """)

            logger.info("Database schema initialized successfully")

    def upsert_game(self, metadata: Dict[str, Any], folder_path: Path, folder_mtime: float, json_mtime: float):
        """
        Insert or update a game in the database.

        Args:
            metadata: Metadata dictionary from metadata.json
            folder_path: Path to game folder
            folder_mtime: Folder modification time (for fast diff)
            json_mtime: metadata.json modification time (for fast diff)
        """
        import json

        # Extract fields from metadata
        title_obj = metadata.get('title', {})
        if isinstance(title_obj, dict) and 'value' in title_obj:
            title_value = title_obj['value']
            if isinstance(title_value, dict):
                title = (title_value.get('zh_hant') or title_value.get('zh_hans') or
                        title_value.get('en') or title_value.get('ja') or
                        title_value.get('original') or folder_path.name)
            else:
                title = str(title_value) if title_value else folder_path.name
        else:
            title = folder_path.name

        # Developer
        developer_obj = metadata.get('developer')
        if isinstance(developer_obj, dict) and 'value' in developer_obj:
            developer = developer_obj['value']
        else:
            developer = None

        # Cover image
        cover_image = metadata.get('cover_path') or metadata.get('cover_url', {})
        if isinstance(cover_image, dict) and 'value' in cover_image:
            cover_image = cover_image['value']
        # Ensure cover_image is a string (or None)
        if cover_image and not isinstance(cover_image, str):
            cover_image = str(cover_image)
        elif not cover_image:
            cover_image = None

        # Library status
        library_status_obj = metadata.get('library_status')
        if isinstance(library_status_obj, dict) and 'value' in library_status_obj:
            library_status = library_status_obj['value']
        elif isinstance(library_status_obj, str):
            library_status = library_status_obj
        else:
            library_status = 'unstarted'

        # Rating
        rating_obj = metadata.get('rating')
        if isinstance(rating_obj, dict) and 'value' in rating_obj:
            value = rating_obj['value']
            if isinstance(value, dict) and 'score' in value:
                rating = value['score']
            else:
                rating = None
        else:
            rating = None

        # Release date
        release_date_obj = metadata.get('release_date')
        if isinstance(release_date_obj, dict) and 'value' in release_date_obj:
            release_date = release_date_obj['value']
        else:
            release_date = None

        # Badges
        badges = self._extract_badges(metadata)

        # Tags
        tags_obj = metadata.get('tags')
        if isinstance(tags_obj, dict) and 'value' in tags_obj:
            tags = tags_obj['value']
        elif isinstance(tags_obj, list):
            tags = tags_obj
        else:
            tags = []

        # User tags
        user_tags = metadata.get('user_tags', [])
        if not isinstance(user_tags, list):
            user_tags = []

        # VNDB ID
        vndb_id = metadata.get('vndb_id')

        # External IDs
        external_ids = metadata.get('external_ids', {})

        with self.get_cursor() as cursor:
            cursor.execute("""
                INSERT OR REPLACE INTO games (
                    folder_path, title, developer, cover_image, library_status,
                    rating, release_date, badges, tags, user_tags,
                    folder_mtime, json_mtime, vndb_id, external_ids,
                    updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            """, (
                str(folder_path),
                title,
                developer,
                cover_image,
                library_status,
                rating,
                release_date,
                json.dumps(badges),
                json.dumps(tags),
                json.dumps(user_tags),
                folder_mtime,
                json_mtime,
                vndb_id,
                json.dumps(external_ids)
            ))

    def _extract_badges(self, metadata: Dict[str, Any]) -> List[str]:
        """
        Extract asset badges from metadata.

        Args:
            metadata: Metadata dictionary

        Returns:
            List of badge strings
        """
        badges = set()

        assets_detected = metadata.get('assets_detected', [])
        if isinstance(assets_detected, list):
            for asset in assets_detected:
                asset_lower = asset.lower()
                if 'patch' in asset_lower:
                    badges.add('Patch')
                elif 'dlc' in asset_lower or 'expansion' in asset_lower:
                    badges.add('DLC')
                elif asset_lower.endswith('.iso') or asset_lower.endswith('.mdf'):
                    badges.add('ISO')

        versions = metadata.get('versions', [])
        if isinstance(versions, list):
            for version in versions:
                if isinstance(version, dict):
                    assets = version.get('assets', [])
                    for asset in assets:
                        asset_lower = asset.lower()
                        if 'patch' in asset_lower:
                            badges.add('Patch')
                        elif 'dlc' in asset_lower or 'expansion' in asset_lower:
                            badges.add('DLC')
                        elif asset_lower.endswith('.iso'):
                            badges.add('ISO')

        return list(badges)

    def get_games(
        self,
        skip: int = 0,
        limit: int = 50,
        sort_by: str = "recently_added",
        descending: bool = True,
        search: Optional[str] = None,
        filter_tag: Optional[str] = None
    ) -> Tuple[List[Dict[str, Any]], int]:
        """
        Get games from database with sorting, filtering, and pagination.

        This is the ZERO-LATENCY read path - no filesystem I/O!

        Args:
            skip: Number of games to skip (pagination)
            limit: Number of games to return
            sort_by: Sort field ("recently_added", "name", "release_date", "rating")
            descending: Sort order
            search: Full-text search query (uses FTS5)
            filter_tag: Filter by tag

        Returns:
            Tuple of (games list, total count)
        """
        import json

        # Build SQL query
        if search:
            # Use FTS5 full-text search
            query = """
                SELECT g.* FROM games g
                INNER JOIN games_fts fts ON g.folder_path = fts.folder_path
                WHERE games_fts MATCH ?
            """
            params = [search]
        else:
            query = "SELECT * FROM games WHERE 1=1"
            params = []

        # Tag filtering
        if filter_tag and filter_tag != "all":
            query += " AND (tags LIKE ? OR user_tags LIKE ?)"
            params.extend([f'%{filter_tag}%', f'%{filter_tag}%'])

        # Sorting
        sort_column = "created_at"  # Default: recently_added (insertion order)
        if sort_by == "name":
            sort_column = "title COLLATE NOCASE"
        elif sort_by == "release_date":
            sort_column = "release_date"
        elif sort_by == "rating":
            sort_column = "rating"

        query += f" ORDER BY {sort_column} {'DESC' if descending else 'ASC'}"

        # Get total count (before pagination)
        with self.get_cursor() as cursor:
            if search or (filter_tag and filter_tag != "all"):
                # Count with filters
                count_query = f"SELECT COUNT(*) FROM ({query})"
                cursor.execute(count_query, params)
                total = cursor.fetchone()[0]
            else:
                # Fast path: no filters, get total from table count
                cursor.execute("SELECT COUNT(*) FROM games")
                total = cursor.fetchone()[0]

            # Apply pagination
            query += " LIMIT ? OFFSET ?"
            params.extend([limit, skip])

            cursor.execute(query, params)
            rows = cursor.fetchall()

        # Convert rows to dicts
        games = []
        for row in rows:
            game = dict(row)
            # Parse JSON fields
            game['badges'] = json.loads(game.get('badges') or '[]')
            game['tags'] = json.loads(game.get('tags') or '[]')
            game['user_tags'] = json.loads(game.get('user_tags') or '[]')
            game['external_ids'] = json.loads(game.get('external_ids') or '{}')
            games.append(game)

        return games, total

    def get_game_by_path(self, folder_path: str) -> Optional[Dict[str, Any]]:
        """
        Get a game by folder path.

        Args:
            folder_path: Path to game folder

        Returns:
            Game dict or None if not found
        """
        import json

        with self.get_cursor() as cursor:
            cursor.execute("SELECT * FROM games WHERE folder_path = ?", (folder_path,))
            row = cursor.fetchone()

            if row is None:
                return None

            game = dict(row)
            game['badges'] = json.loads(game.get('badges') or '[]')
            game['tags'] = json.loads(game.get('tags') or '[]')
            game['user_tags'] = json.loads(game.get('user_tags') or '[]')
            game['external_ids'] = json.loads(game.get('external_ids') or '{}')
            return game

    def update_library_status(self, folder_path: str, library_status: str):
        """
        Update library status for a game.

        Args:
            folder_path: Path to game folder
            library_status: New library status
        """
        with self.get_cursor() as cursor:
            cursor.execute("""
                UPDATE games
                SET library_status = ?, updated_at = CURRENT_TIMESTAMP
                WHERE folder_path = ?
            """, (library_status, folder_path))

    def delete_game(self, folder_path: str):
        """
        Delete a game from database.

        Args:
            folder_path: Path to game folder
        """
        with self.get_cursor() as cursor:
            cursor.execute("DELETE FROM games WHERE folder_path = ?", (folder_path,))

    def get_all_folder_mtimes(self) -> Dict[str, float]:
        """
        Get all folder paths and their modification times.

        Used for fast diff during scanning.

        Returns:
            Dict mapping folder_path -> folder_mtime
        """
        with self.get_cursor() as cursor:
            cursor.execute("SELECT folder_path, folder_mtime FROM games")
            return {row[0]: row[1] for row in cursor.fetchall()}

    def get_all_json_mtimes(self) -> Dict[str, float]:
        """
        Get all folder paths and their metadata.json modification times.

        Used for fast diff during scanning.

        Returns:
            Dict mapping folder_path -> json_mtime
        """
        with self.get_cursor() as cursor:
            cursor.execute("SELECT folder_path, json_mtime FROM games")
            return {row[0]: row[1] for row in cursor.fetchall()}

    def close(self):
        """Close database connection."""
        if hasattr(self.local, 'conn') and self.local.conn:
            self.local.conn.close()
            self.local.conn = None


# Global database instance
_db: Database = None


def get_database() -> Database:
    """
    Get or create global database instance.

    Returns:
        Database singleton
    """
    global _db
    if _db is None:
        from .config import settings
        db_path = Path(settings.DATABASE_PATH)
        _db = Database(db_path)
        logger.info(f"Database initialized at: {db_path}")
    return _db


def get_db():
    """FastAPI dependency for database access."""
    yield get_database()


def close_database():
    """Close database connection (mainly for testing)."""
    global _db
    if _db:
        _db.close()
        _db = None
