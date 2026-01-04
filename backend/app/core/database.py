"""
SQLite Database Core for Galgame Library Manager.

Phase 20.0: The Instant Index - Achieve "Everything-like" instant search performance.

Sprint 2: Added scan_candidates table for candidate confirmation workflow.

Architecture:
- SQLite with FTS5 (Full-Text Search) for instant text search
- WAL mode for high concurrency (readers don't block writers)
- Flattened metadata schema for fast sorting/filtering
- Zero-latency reads: All queries hit DB, not filesystem
- ScanCandidate workflow: Scan → Candidates → Library Confirmation
"""

import logging
import sqlite3
import threading
from pathlib import Path
from typing import Dict, Any, List, Optional, Tuple
from contextlib import contextmanager
from datetime import datetime
from enum import Enum

logger = logging.getLogger(__name__)


class ScanStatus(str, Enum):
    """Status for scan candidates."""
    PENDING = "pending"
    CONFIRMED = "confirmed"
    IGNORED = "ignored"
    REJECTED = "rejected"
    MERGED = "merged"


class ScanCandidate(SQLModel, table=True):
    """
    Scan candidate pending library confirmation.
    
    This is NOT a Game yet - it's a detection result that needs
    confirmation before becoming a Game entity.
    """
    __tablename__ = "scan_candidates"
    id: Optional[int] = Field(default=None, primary_key=True)
    path: str = Field(index=True, unique=True)
    
    # Detection metadata
    detected_title: str
    detected_engine: Optional[str] = None
    confidence_score: float = Field(default=0.5, ge=0.0, le=1.0)
    
    # Game indicators found
    game_indicators: str = Field(default="[]")  # JSON array: ["has_executable", "has_game_files"]
    
    # Status lifecycle
    status: ScanStatus = Field(default=ScanStatus.PENDING)
    
    # Timestamps
    detected_at: datetime = Field(default_factory=datetime.now)
    confirmed_at: Optional[datetime] = None
    
    # User feedback
    manual_correction: Optional[str] = None


class MatchStatus(str, Enum):
    """Status for identity match candidates."""
    pending = "pending"
    accepted = "accepted"
    canonicalized = "canonicalized"  # NEW - Sprint 4
    rejected = "rejected"


class IdentityMatchCandidate(SQLModel, table=True):
    """
    Identity match candidate pending canonicalization.
    
    External identity hypotheses for a detected folder.
    Examples:
    - VNDB ID matches (high confidence)
    - User-provided title match
    - File signature match
    """
    __tablename__ = "identity_match_candidate"
    id: Optional[int] = Field(default=None, primary_key=True)
    
    # Detection result
    path: str = Field(index=True, unique=True)
    detected_title: str
    detected_engine: Optional[str] = None
    confidence_score: float = Field(default=0.5, ge=0.0, le=1.0)
    
    # External hypothesis
    external_source_type: Optional[str] = Field(None, description="Source: vndb | user | scanner")
    external_source_id: Optional[str] = Field(None, description="Source ID: VNDB v12345 | scan_candidate_789")
    
    # Status lifecycle
    status: MatchStatus = Field(default=MatchStatus.pending)
    
    # Timestamps
    detected_at: datetime = Field(default_factory=datetime.now)
    canonicalized_at: Optional[datetime] = None


class ScanStatus(str, Enum):
    """Status for scan candidates."""
    pending = "pending"
    confirmed = "confirmed"
    ignored = "ignored"
    rejected = "rejected"
    merged = "merged"
    locked = "locked"


class Database:
    """
    SQLite database manager with FTS5 full-text search.
    
    Sprint 4: Added Canonicalization (Identity Match → Canonical Entities).
    
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
        Sprint 2:
            - scan_candidates table: For candidate confirmation workflow
        
        Sprint 4:
            - identity_match_candidate table: For identity match tracking
            - canonical_source_link table: Trace origin of every canonical entity
            - companies, persons, roles, characters tables: Canonical entities
            - game_staff_link, character_voice_link: Graph links
        Games table: Flattened metadata for fast queries
        - games_fts table: FTS5 virtual table for full-text search
        """
        # Ensure parent directory exists
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        
        logger.info(f"Initializing database at: {self.db_path}")
        
        with self.get_cursor() as cursor:
            # Sprint 2: ScanCandidates table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS scan_candidates (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path TEXT UNIQUE NOT NULL,
                    detected_title TEXT NOT NULL,
                    detected_engine TEXT,
                    confidence_score REAL DEFAULT 0.5,
                    game_indicators TEXT,  -- JSON array: ["has_executable", "has_game_files"]
                    status TEXT DEFAULT 'pending',  -- pending, confirmed, ignored, rejected, merged
                    detected_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    confirmed_at TIMESTAMP,
                    manual_correction TEXT
                )
            """)
            
            # Indexes for ScanCandidates
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_candidates_status
                ON scan_candidates(status)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_candidates_confidence
                ON scan_candidates(confidence_score DESC)
            """)
            
            # Sprint 4: Identity Match Candidates table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS identity_match_candidate (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path TEXT UNIQUE NOT NULL,
                    detected_title TEXT NOT NULL,
                    detected_engine TEXT,
                    confidence_score REAL DEFAULT 0.5,
                    external_source_type TEXT,
                    external_source_id TEXT,
                    status TEXT DEFAULT 'pending',
                    detected_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    canonicalized_at TIMESTAMP
                )
            """)
            
            # Sprint 4: Canonical Source Links table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS canonical_source_link (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    entity_type TEXT NOT NULL,
                    entity_id INTEGER NOT NULL,
                    source_type TEXT NOT NULL,
                    source_id TEXT NOT NULL,
                    source_hash TEXT NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)
            
            # Sprint 4: Companies table (canonical)
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS companies (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT UNIQUE NOT NULL,
                    logo_url TEXT
                )
            """)
            
            # Sprint 4: Persons table (canonical)
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS persons (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    original_name TEXT,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)
            
            # Sprint 4: Roles table (canonical)
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS roles (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT UNIQUE NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)
            
            # Sprint 4: Characters table (canonical, game-scoped)
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS characters (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    game_id INTEGER NOT NULL,
                    image_url TEXT,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)
            
            # Sprint 4: Game Staff Links table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS game_staff_link (
                    game_id INTEGER NOT NULL,
                    person_id INTEGER NOT NULL,
                    role_id INTEGER NOT NULL,
                    PRIMARY KEY (game_id, person_id, role_id),
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)
            
            # Sprint 4: Character Voice Links table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS character_voice_link (
                    character_id INTEGER NOT NULL,
                    person_id INTEGER NOT NULL,
                    PRIMARY KEY (character_id, person_id),
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)
            
            # Indexes for scan_candidates
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_candidates_status
                ON scan_candidates(status)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_candidates_confidence
                ON scan_candidates(confidence_score DESC)
            """)
            
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
            
            # Indexes for scan_candidates
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_candidates_status
                ON scan_candidates(status)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_candidates_confidence
                ON scan_candidates(confidence_score DESC)
            """)
            
            # Indexes for canonical entities
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_companies_name
                ON companies(name)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_persons_name
                ON persons(name)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_persons_original_name
                ON persons(original_name)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_roles_name
                ON roles(name)
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_characters_game_name
                ON characters(game_id)
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
        sort_by: str = "最近添加",
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
            sort_by: Sort field ("最近添加", "名称", "发行日期", "评分")
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
        if filter_tag and filter_tag != "所有游戏":
            query += " AND (tags LIKE ? OR user_tags LIKE ?)"
            params.extend([f'%{filter_tag}%', f'%{filter_tag}%'])

        # Sorting
        sort_column = "created_at"  # Default: 最近添加 (insertion order)
        if sort_by == "名称":
            sort_column = "title COLLATE NOCASE"
        elif sort_by == "发行日期":
            sort_column = "release_date"
        elif sort_by == "评分":
            sort_column = "rating"

        query += f" ORDER BY {sort_column} {'DESC' if descending else 'ASC'}"

        # Get total count (before pagination)
        with self.get_cursor() as cursor:
            if search or (filter_tag and filter_tag != "所有游戏"):
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

    # ========================================================================
    # Sprint 4: CRUD Methods for Canonicalization
    # ========================================================================

    # Identity Match Candidates
    def insert_identity_match_candidate(
        self,
        path: str,
        detected_title: str,
        detected_engine: Optional[str] = None,
        confidence_score: float = 0.5,
        game_indicators: List[str] = None
    ) -> bool:
        """
        Insert an identity match candidate.
        
        Args:
            path: Folder path
            detected_title: Detected title
            detected_engine: Engine name
            confidence_score: Confidence score
            game_indicators: List of game indicators
            
        Returns:
            True if successful
        """
        try:
            with self.get_cursor() as cursor:
                cursor.execute("""
                    INSERT INTO identity_match_candidate (
                        path, detected_title, detected_engine, confidence_score, 
                        game_indicators, external_source_type, external_source_id, status
                    ) VALUES (?, ?, ?, ?, ?, ?, 'scanner', NULL, ?)
                """, (
                    path, detected_title, detected_engine, confidence_score,
                    json.dumps(game_indicators) if game_indicators else "[]",
                    MatchStatus.pending.value
                ))
                logger.info(f"Inserted identity match candidate: {detected_title}")
                return True
        except Exception as e:
            logger.error(f"Failed to insert identity match candidate: {e}")
            return False
    
    def get_identity_match_candidates(
        self,
        limit: int = 100,
        status: Optional[str] = None
    ) -> List:
        """
        Get identity match candidates.
        
        Args:
            limit: Maximum candidates to return
            status: Optional filter by status
            
        Returns:
            List of IdentityMatchCandidate objects
        """
        try:
            with self.get_cursor() as cursor:
                if status:
                    cursor.execute("""
                        SELECT * FROM identity_match_candidate
                        WHERE status = ?
                        ORDER BY confidence_score DESC
                        LIMIT ?
                    """, (status.value, limit))
                else:
                    cursor.execute("""
                        SELECT * FROM identity_match_candidate
                        ORDER BY confidence_score DESC
                        LIMIT ?
                    """, (limit,))
                
                rows = cursor.fetchall()
                
                candidates = []
                for row in rows:
                    # Parse game_indicators
                    game_indicators = json.loads(row['game_indicators']) if row['game_indicators'] else []
                    
                    candidates.append(IdentityMatchCandidate(
                        id=row['id'],
                        path=row['path'],
                        detected_title=row['detected_title'],
                        detected_engine=row['detected_engine'],
                        confidence_score=row['confidence_score'],
                        game_indicators=game_indicators,
                        external_source_type=row.get('external_source_type'),
                        external_source_id=row.get('external_source_id'),
                        status=MatchStatus(row['status']),
                        detected_at=datetime.fromisoformat(row['detected_at']),
                        canonicalized_at=datetime.fromisoformat(row.get('canonicalized_at')) if row.get('canonicalized_at') else None
                    ))
                
                logger.debug(f"Retrieved {len(candidates)} identity match candidates (status: {status})")
                return candidates
                
        except Exception as e:
            logger.error(f"Failed to get identity match candidates: {e}")
            return []
    
    def get_identity_match_candidate_by_id(
        self,
        candidate_id: int
    ):
        """
        Get a specific IdentityMatchCandidate by ID.
        """
        try:
            with self.get_cursor() as cursor:
                cursor.execute("SELECT * FROM identity_match_candidate WHERE id = ?", (candidate_id,))
                row = cursor.fetchone()
                
                if not row:
                    return None
                
                # Parse game_indicators
                game_indicators = json.loads(row['game_indicators']) if row['game_indicators'] else []
                
                return IdentityMatchCandidate(
                    id=row['id'],
                    path=row['path'],
                    detected_title=row['detected_title'],
                        detected_engine=row['detected_engine'],
                        confidence_score=row['confidence_score'],
                        game_indicators=game_indicators,
                        external_source_type=row.get('external_source_type'),
                        external_source_id=row.get('external_source_id'),
                        status=MatchStatus(row['status']),
                        detected_at=datetime.fromisoformat(row['detected_at']),
                        canonicalized_at=datetime.fromisoformat(row.get('canonicalized_at')) if row.get('canonicalized_at') else None
                )
                
        except Exception as e:
            logger.error(f"Failed to get identity match candidate by ID: {e}")
            return None
    
    # Scan Candidates
    def insert_scan_candidate(
        self,
        candidate
    ) -> bool:
        """
        Insert a ScanCandidate into database.
        
        Args:
            candidate: ScanCandidate object
            
        Returns:
            True if successful
        """
        try:
            with self.get_cursor() as cursor:
                cursor.execute("""
                    INSERT INTO scan_candidates (
                        path, detected_title, detected_engine, confidence_score, 
                        game_indicators, status, detected_at, manual_correction
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """, (
                    candidate.path, candidate.detected_title, candidate.detected_engine, candidate.confidence_score,
                    json.dumps(candidate.game_indicators),
                    candidate.status.value,
                    candidate.detected_at.isoformat(),
                    candidate.manual_correction
                ))
                logger.info(f"Inserted scan candidate: {candidate.detected_title} ({candidate.status.value})")
                return True
        except Exception as e:
            logger.error(f"Failed to insert scan candidate: {e}")
            return False

    def get_scan_candidate_by_id(
        self,
        candidate_id: int
    ):
        """
        Get a specific ScanCandidate by ID.
        """
        try:
            with self.get_cursor() as cursor:
                cursor.execute("SELECT * FROM scan_candidates WHERE id = ?", (candidate_id,))
                row = cursor.fetchone()
                
                if not row:
                    return None
                
                # Parse game_indicators
                game_indicators = json.loads(row['game_indicators']) if row['game_indicators'] else []
                
                return ScanCandidate(
                    id=row['id'],
                        path=row['path'],
                        detected_title=row['detected_title'],
                        detected_engine=row['detected_engine'],
                        confidence_score=row['confidence_score'],
                        game_indicators=game_indicators,
                        status=MatchStatus(row['status']),
                        detected_at=datetime.fromisoformat(row['detected_at']),
                        confirmed_at=datetime.fromisoformat(row['confirmed_at']) if row['confirmed_at'] else None,
                        manual_correction=row['manual_correction']
                    )
                
        except Exception as e:
            logger.error(f"Failed to get scan candidate by ID: {e}")
            return None
    
    def get_scan_candidates(
        self,
        status: Optional[str] = None,
        limit: int = 100
    ) -> List:
        """
        Get scanCandidates from database with optional filtering.
        """
        try:
            with self.get_cursor() as cursor:
                if status:
                    cursor.execute("""
                        SELECT * FROM scan_candidates
                        WHERE status = ?
                        ORDER BY confidence_score DESC
                        LIMIT ?
                    """, (status.value, limit))
                else:
                    cursor.execute("""
                        SELECT * FROM scan_candidates
                        ORDER BY confidence_score DESC
                        LIMIT ?
                    """, (limit,))
                
                rows = cursor.fetchall()
                
                candidates = []
                for row in rows:
                    # Parse game_indicators
                    game_indicators = json.loads(row['game_indicators']) if row['game_indicators'] else []
                    
                    candidates.append(ScanCandidate(
                        id=row['id'],
                        path=row['path'],
                        detected_title=row['detected_title'],
                        detected_engine=row['detected_engine'],
                        confidence_score=row['confidence_score'],
                        game_indicators=game_indicators,
                        status=MatchStatus(row['status']),
                        detected_at=datetime.fromisoformat(row['detected_at']),
                        confirmed_at=datetime.fromisoformat(row['confirmed_at']) if row['confirmed_at'] else None,
                        manual_correction=row['manual_correction']
                    ))
                
                logger.debug(f"Retrieved {len(candidates)} scan candidates (status: {status})")
                return candidates
                
        except Exception as e:
            logger.error(f"Failed to get scan candidates: {e}")
            return []
    
    # Scan Candidate Status Updates
    def update_candidate_status(
        self,
        candidate_id: int,
        status: MatchStatus
        manual_correction: Optional[str] = None
    ) -> bool:
        """
        Update status of an IdentityMatchCandidate.
        """
        try:
            with self.get_cursor() as cursor:
                if manual_correction:
                    cursor.execute("""
                        UPDATE identity_match_candidate
                        SET status = ?, manual_correction = ?
                        WHERE id = ?
                    """, (status.value, manual_correction, candidate_id))
                else:
                    cursor.execute("""
                        UPDATE identity_match_candidate
                        SET status = ?
                        WHERE id = ?
                    """, (status.value, candidate_id))
                
                logger.info(f"Updated identity match candidate {candidate_id} to {status.value}")
                return True
                
        except Exception as e:
            logger.error(f"Failed to update candidate status: {e}")
            return False
    
    # Scan Candidate Status Updates
    def update_scan_candidate_status(
        self,
        candidate_id: int,
        status: MatchStatus
        manual_correction: Optional[str] = None
    ) -> bool:
        """
        Update status of a ScanCandidate.
        """
        try:
            with self.get_cursor() as cursor:
                if manual_correction:
                    cursor.execute("""
                        UPDATE scan_candidates
                        SET status = ?, manual_correction = ?
                        WHERE id = ?
                    """, (status.value, manual_correction, candidate_id))
                else:
                    cursor.execute("""
                        UPDATE scan_candidates
                        SET status = ?
                        WHERE id = ?
                    """, (status.value, candidate_id))
                
                logger.info(f"Updated scan candidate {candidate_id} to {status.value}")
                return True
                
        except Exception as e:
            logger.error(f"Failed to update candidate status: {e}")
            return False
    
    # ========================================================================
    # Sprint 4: Canonicalization CRUD Methods
    # ========================================================================

    def create_canonical_source_link(
        self,
        entity_type: str,
        entity_id: int,
        source_type: str,
        source_id: str,
        source_hash: str
    ) -> int:
        """
        Create a CanonicalSourceLink (provenance tracking).
        
        ABSOLUTELY REQUIRED: Every canonical entity creation MUST write one of these.
        """
        try:
            with self.get_cursor() as cursor:
                cursor.execute("""
                    INSERT INTO canonical_source_link (
                        entity_type, entity_id, source_type, source_id, source_hash
                    ) VALUES (?, ?, ?, ?, ?)
                """, (
                    entity_type, entity_id, source_type, source_id, source_hash
                ))
                
                logger.info(f"Created canonical_source_link: {entity_type}:{entity_id} <- {source_type}:{source_id}")
                return cursor.lastrowid
                
        except Exception as e:
            logger.error(f"Failed to create canonical source link: {e}")
            raise
    
    def close(self):
        """Close database connection (mainly for testing)."""
        if hasattr(self.local, 'conn') and self.local.conn:
            self.local.conn.close()
            self.local.conn = None

    # ========================================================================
    # Sprint 2: ScanCandidate Management
    # ========================================================================

    def insert_scan_candidate(
        self,
        candidate: ScanCandidate
    ) -> bool:
        """
        Insert a scanCandidate into database.
        
        Args:
            candidate: ScanCandidate object
            
        Returns:
            True if successful
        """
        try:
            with self.get_cursor() as cursor:
                cursor.execute("""
                    INSERT INTO scan_candidates (
                        path,
                        detected_title,
                        detected_engine,
                        confidence_score,
                        game_indicators,
                        status,
                        detected_at,
                        manual_correction
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """, (
                    candidate.path,
                    candidate.detected_title,
                    candidate.detected_engine,
                    candidate.confidence_score,
                    json.dumps(candidate.game_indicators),
                    candidate.status.value,
                    candidate.detected_at.isoformat(),
                    candidate.manual_correction
                ))
                logger.info(f"Inserted scan candidate: {candidate.detected_title} ({candidate.status.value})")
                return True
        except Exception as e:
            logger.error(f"Failed to insert scan candidate: {e}")
            return False

    def get_scan_candidates(
        self,
        status: Optional[ScanStatus] = None,
        limit: int = 100
    ) -> List[ScanCandidate]:
        """
        Get scanCandidates from database with optional filtering.
        
        Args:
            status: Filter by status (optional)
            limit: Maximum number of candidates to return
            
        Returns:
            List of ScanCandidate objects
        """
        try:
            with self.get_cursor() as cursor:
                if status:
                    cursor.execute("""
                        SELECT * FROM scan_candidates
                        WHERE status = ?
                        ORDER BY confidence_score DESC
                        LIMIT ?
                    """, (status.value, limit))
                else:
                    cursor.execute("""
                        SELECT * FROM scan_candidates
                        ORDER BY confidence_score DESC
                        LIMIT ?
                    """, (limit,))
                
                rows = cursor.fetchall()
                
                candidates = []
                for row in rows:
                    # Parse JSON fields
                    game_indicators = json.loads(row['game_indicators']) if row['game_indicators'] else []
                    
                    candidates.append(ScanCandidate(
                        id=row['id'],
                        path=row['path'],
                        detected_title=row['detected_title'],
                        detected_engine=row['detected_engine'],
                        confidence_score=row['confidence_score'],
                        game_indicators=game_indicators,
                        status=ScanStatus(row['status']),
                        detected_at=datetime.fromisoformat(row['detected_at']),
                        confirmed_at=datetime.fromisoformat(row['confirmed_at']) if row['confirmed_at'] else None,
                        manual_correction=row['manual_correction']
                    ))
                
                logger.debug(f"Retrieved {len(candidates)} scan candidates (status: {status})")
                return candidates
                
        except Exception as e:
            logger.error(f"Failed to get scan candidates: {e}")
            return []

    def get_scan_candidate_by_id(
        self,
        candidate_id: int
    ) -> Optional[ScanCandidate]:
        """
        Get a specific ScanCandidate by ID.
        
        Args:
            candidate_id: ID of candidate
            
        Returns:
            ScanCandidate object or None
        """
        try:
            with self.get_cursor() as cursor:
                cursor.execute("SELECT * FROM scan_candidates WHERE id = ?", (candidate_id,))
                row = cursor.fetchone()
                
                if not row:
                    return None
                
                # Parse JSON fields
                game_indicators = json.loads(row['game_indicators']) if row['game_indicators'] else []
                
                return ScanCandidate(
                    id=row['id'],
                    path=row['path'],
                    detected_title=row['detected_title'],
                    detected_engine=row['detected_engine'],
                    confidence_score=row['confidence_score'],
                    game_indicators=game_indicators,
                    status=ScanStatus(row['status']),
                    detected_at=datetime.fromisoformat(row['detected_at']),
                    confirmed_at=datetime.fromisoformat(row['confirmed_at']) if row['confirmed_at'] else None,
                    manual_correction=row['manual_correction']
                )
                
        except Exception as e:
            logger.error(f"Failed to get scan candidate by ID: {e}")
            return None

    def update_candidate_status(
        self,
        candidate_id: int,
        status: ScanStatus,
        manual_correction: Optional[str] = None
    ) -> bool:
        """
        Update status of a ScanCandidate.
        
        Args:
            candidate_id: ID of candidate
            status: New status
            manual_correction: Optional manual correction text
            
        Returns:
            True if successful
        """
        try:
            with self.get_cursor() as cursor:
                if manual_correction:
                    cursor.execute("""
                        UPDATE scan_candidates
                        SET status = ?, manual_correction = ?, confirmed_at = ?
                        WHERE id = ?
                    """, (
                        status.value,
                        manual_correction,
                        datetime.now().isoformat(),
                        candidate_id
                    ))
                else:
                    cursor.execute("""
                        UPDATE scan_candidates
                        SET status = ?, confirmed_at = ?
                        WHERE id = ?
                    """, (
                        status.value,
                        datetime.now().isoformat(),
                        candidate_id
                    ))
                
                logger.info(f"Updated candidate {candidate_id} to {status.value}")
                return True
                
        except Exception as e:
            logger.error(f"Failed to update candidate status: {e}")
            return False

    def delete_scan_candidate(
        self,
        candidate_id: int
    ) -> bool:
        """
        Delete a ScanCandidate from database.
        
        Args:
            candidate_id: ID of candidate to delete
            
        Returns:
            True if successful
        """
        try:
            with self.get_cursor() as cursor:
                cursor.execute("DELETE FROM scan_candidates WHERE id = ?", (candidate_id,))
                logger.info(f"Deleted scan candidate: {candidate_id}")
                return True
                
        except Exception as e:
            logger.error(f"Failed to delete scan candidate: {e}")
            return False


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
        from ..config import get_config
        config = get_config()
        db_path = config.config_dir / "library.db"
        _db = Database(db_path)
        logger.info(f"Database initialized at: {db_path}")
    return _db


def close_database():
    """Close database connection (mainly for testing)."""
    global _db
    if _db:
        _db.close()
        _db = None
