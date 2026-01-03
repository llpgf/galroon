"""
Data models for the 'Vnite-Killer' Metadata System.

Provides field-level locking, multilingual support, and provider tracking.
"""

from typing import TypeVar, Generic, List, Optional, Dict, Any
from pydantic import BaseModel, Field
from datetime import datetime
from enum import Enum
import logging

logger = logging.getLogger(__name__)

T = TypeVar('T')


class LibraryStatus(str, Enum):
    """
    Library/Collection status for an asset.

    Phase 19.6: Semantic Sanitization - Removed "Game Launcher" language.
    This is an Asset Manager, not a Game Launcher.
    """
    UNSTARTED = "unstarted"  # Asset not yet engaged with
    IN_PROGRESS = "in_progress"  # Currently being experienced
    FINISHED = "finished"  # Completed experience
    ON_HOLD = "on_hold"  # Temporarily paused
    DROPPED = "dropped"  # Abandoned
    PLANNED = "planned"  # Intended to experience in the future


class Character(BaseModel):
    """Character information for visual novel cast."""
    name: str = Field(description="Character name")
    name_ja: Optional[str] = Field(default=None, description="Japanese name")
    role: str = Field(default="", description="Role (protagonist, heroine, supporting, etc.)")
    cv: Optional[str] = Field(default=None, description="Voice actor/CV")
    image_url: Optional[str] = Field(default=None, description="Character image URL")
    description: Optional[str] = Field(default=None, description="Character description")
    spoiler: bool = Field(default=False, description="Contains spoilers")


class Staff(BaseModel):
    """Staff information for visual novel production."""
    name: str = Field(description="Staff member name")
    role: str = Field(default="", description="Staff role (director, writer, artist, music)")
    note: Optional[str] = Field(default=None, description="Additional notes")


class MetadataField(BaseModel, Generic[T]):
    """
    A generic metadata field with locking and source tracking.

    Attributes:
        value: The actual field value
        source: Provider that supplied this value (e.g., "vndb", "manual", "steam")
        locked: If True, never overwrite this field (manual edit protection)
        last_updated: Timestamp of last update
    """

    value: T
    source: str = Field(default="unknown", description="Provider that supplied this value")
    locked: bool = Field(default=False, description="If True, field cannot be overwritten")
    last_updated: str = Field(default_factory=lambda: datetime.now().isoformat())

    def can_update(self, new_source: str) -> bool:
        """
        Check if this field can be updated.

        Args:
            new_source: Provider attempting to update

        Returns:
            True if field can be updated, False otherwise
        """
        if self.locked:
            return False
        return True


class MultilingualTitle(BaseModel):
    """
    Multilingual title support with Babel-like capabilities.

    Provides titles in multiple languages with traditional Chinese preference.
    """

    ja: str = Field(default="", description="Japanese title (original)")
    en: str = Field(default="", description="English title")
    zh_hans: str = Field(default="", description="Simplified Chinese title")
    zh_hant: str = Field(default="", description="Traditional Chinese title (preferred)")
    original: str = Field(default="", description="Original script (typically Japanese)")

    def get_preferred(self, prefer_traditional: bool = True) -> str:
        """
        Get the preferred title based on user preference.

        Args:
            prefer_traditional: If True, prefer Traditional Chinese

        Returns:
            Best available title
        """
        if prefer_traditional and self.zh_hant:
            return self.zh_hant
        if not prefer_traditional and self.zh_hans:
            return self.zh_hans
        if self.en:
            return self.en
        if self.ja:
            return self.ja
        return self.original

    def get_original(self) -> str:
        """Get the original title (typically Japanese)."""
        return self.original or self.ja


class Rating(BaseModel):
    """
    Normalized rating value.

    All ratings normalized to 0-10 scale for consistency.
    """

    score: float = Field(ge=0.0, le=10.0, description="Rating score (0-10)")
    count: int = Field(default=0, ge=0, description="Number of ratings")
    source: str = Field(default="unknown", description="Rating source (e.g., vndb, bangumi)")

    def __str__(self) -> str:
        return f"{self.score:.1f}/10 ({self.count} votes)"


class ExternalIDs(BaseModel):
    """
    External service IDs for cross-provider mapping.

    Provides type-safe access to external service identifiers.
    """
    steam: Optional[str] = Field(default=None, description="Steam App ID")
    bangumi: Optional[str] = Field(default=None, description="Bangumi subject ID")
    erogamescape: Optional[str] = Field(default=None, description="ErogameScape ID")
    vndb: Optional[str] = Field(default=None, description="VNDB ID (duplicate of vndb_id field)")


class GameVersion(BaseModel):
    """Represents a single version/installation of a game."""
    path: str = Field(description="Absolute path to this version")
    label: str = Field(default="", description="Human-readable version label (e.g., 'Steam Ver', 'CD Ver')")
    is_primary: bool = Field(default=False, description="Is this the primary version?")
    assets: List[str] = Field(default_factory=list, description="Asset tags present in this version")


class UnifiedMetadata(BaseModel):
    """
    Unified metadata model for visual novels/games.

    **PHASE 9 UPDATE - Work-Centric Model:**
    - VNDB ID is now the anchor (primary key)
    - Supports multiple versions (installations) under one work
    - Asset inventory detection for each version
    - Local cached visuals for offline access

    Supports multiple providers with field-level locking and multilingual titles.
    """

    # Primary identifier (ANCHOR - Required for new records)
    vndb_id: Optional[str] = Field(default=None, description="VNDB identifier (e.g., v12345) - PRIMARY KEY")

    # External IDs (Cross-provider mapping)
    external_ids: ExternalIDs = Field(
        default_factory=ExternalIDs,
        description="External service IDs (Steam, Bangumi, ErogameScape, etc.)"
    )

    # DEPRECATED: Kept for backward compatibility, migrate to external_ids['steam']
    steam_id: Optional[str] = Field(default=None, description="Steam AppID (DEPRECATED - use external_ids)")

    # Title (multilingual)
    title: MetadataField[MultilingualTitle] = Field(
        default_factory=lambda: MetadataField(value=MultilingualTitle()),
        description="Multilingual title with locking"
    )

    # Description
    description: MetadataField[str] = Field(
        default_factory=lambda: MetadataField(value=""),
        description="Game description (HTML stripped, plain text)"
    )

    # Rating
    rating: MetadataField[Rating] = Field(
        default_factory=lambda: MetadataField(value=Rating(score=0.0, count=0)),
        description="Normalized rating (0-10 scale)"
    )

    # Cover image
    cover_url: MetadataField[str] = Field(
        default_factory=lambda: MetadataField(value=""),
        description="URL to cover image"
    )
    cover_path: Optional[str] = Field(
        default=None,
        description="Local path to downloaded cover image"
    )

    # Screenshots
    screenshot_urls: MetadataField[List[str]] = Field(
        default_factory=lambda: MetadataField(value=[]),
        description="List of screenshot URLs"
    )
    screenshot_paths: List[str] = Field(
        default_factory=list,
        description="Local paths to downloaded screenshots"
    )

    # Background image (for immersive UI)
    background_url: MetadataField[str] = Field(
        default_factory=lambda: MetadataField(value=""),
        description="URL to background image (highest rated screenshot)"
    )
    background_path: Optional[str] = Field(
        default=None,
        description="Local path to downloaded background image"
    )

    # Characters (Cast list)
    characters: MetadataField[List[Character]] = Field(
        default_factory=lambda: MetadataField(value=[]),
        description="List of main characters with CVs"
    )

    # Staff (Production staff)
    staff: MetadataField[List[Staff]] = Field(
        default_factory=lambda: MetadataField(value=[]),
        description="List of staff members (writer, artist, etc.)"
    )

    # Phase 19.6: User's library status (Semantic Sanitization)
    # Renamed from play_status to remove "Game Launcher" language
    library_status: MetadataField[LibraryStatus] = Field(
        default_factory=lambda: MetadataField(value=LibraryStatus.UNSTARTED),
        description="User's library/progress status for this asset"
    )

    # Tags/Genres (Read-only from providers like VNDB)
    tags: MetadataField[List[str]] = Field(
        default_factory=lambda: MetadataField(value=[]),
        description="Tags and genres (from VNDB, Bangumi, etc.)"
    )

    # ========== PHASE 18.5: CUSTOM USER TAGS ==========
    # User-defined tags (editable, personal organization)
    user_tags: List[str] = Field(
        default_factory=list,
        description="User-defined custom tags (e.g., 'Tier 0', 'Favorites', 'To Play')"
    )
    # ========== END PHASE 18.5 ==========

    # Release information
    release_date: MetadataField[str] = Field(
        default_factory=lambda: MetadataField(value=""),
        description="Release date in YYYY-MM-DD format"
    )

    # Developer
    developer: MetadataField[str] = Field(
        default_factory=lambda: MetadataField(value=""),
        description="Developer/publisher name"
    )

    # Language support
    languages: List[str] = Field(
        default_factory=list,
        description="Supported languages (ISO 639-1 codes)"
    )

    # ========== PHASE 9: WORK-CENTRIC MODEL ==========

    # Versions (Multi-version aggregation)
    versions: List[GameVersion] = Field(
        default_factory=list,
        description="All installed versions of this work (e.g., CD Ver, Steam Ver, HDD Ver)"
    )

    # Asset Inventory (Detected tags)
    assets_detected: List[str] = Field(
        default_factory=list,
        description="Detected asset tags across all versions: ['ISO', 'DLC', 'OST', 'Crack', 'Chinese']"
    )

    # Visuals (Local cached paths)
    visuals: Dict[str, str] = Field(
        default_factory=dict,
        description="Local cached image paths: {poster: '/path/cover.jpg', background: '/path/bg.jpg', logo: '/path/logo.png'}"
    )

    # Credits (Structured staff data for knowledge graph)
    credits: List[Dict[str, str]] = Field(
        default_factory=list,
        description="Structured credits: [{name: 'Gen Urobuchi', role: 'Scenario', vndb_id: '...'}]"
    )

    # ========== END PHASE 9 ==========

    # ========== PHASE 10: FIELD LOCKING ==========
    # List of field names that are locked (manual edits, should not be overwritten)
    locked_fields: List[str] = Field(
        default_factory=list,
        description="List of locked field names that should not be overwritten by scrapers"
    )
    # ========== END PHASE 10 ==========

    # Metadata
    metadata_version: str = Field(
        default="2.1",  # Bump version for Phase 10
        description="Metadata schema version"
    )
    last_sync: str = Field(
        default_factory=lambda: datetime.now().isoformat(),
        description="Last sync timestamp"
    )

    # Provider sources
    providers: List[str] = Field(
        default_factory=list,
        description="List of providers that contributed data"
    )

    class Config:
        """Pydantic configuration."""
        json_encoders = {
            datetime: lambda v: v.isoformat()
        }

    def get_preferred_title(self, prefer_traditional: bool = True) -> str:
        """
        Get the preferred display title.

        Args:
            prefer_traditional: If True, prefer Traditional Chinese

        Returns:
            Best available title
        """
        return self.title.value.get_preferred(prefer_traditional)

    def get_original_title(self) -> str:
        """Get the original title (typically Japanese)."""
        return self.title.value.get_original()

    def lock_field(self, field_name: str) -> bool:
        """
        Lock a specific field to prevent overwrites.

        Args:
            field_name: Name of field to lock

        Returns:
            True if locked successfully
        """
        if hasattr(self, field_name):
            field = getattr(self, field_name)
            if isinstance(field, MetadataField):
                field.locked = True
                return True
        return False

    def unlock_field(self, field_name: str) -> bool:
        """
        Unlock a specific field to allow updates.

        Args:
            field_name: Name of field to unlock

        Returns:
            True if unlocked successfully
        """
        if hasattr(self, field_name):
            field = getattr(self, field_name)
            if isinstance(field, MetadataField):
                field.locked = False
                return True
        return False

    def is_locked(self, field_name: str) -> bool:
        """
        Check if a field is locked.

        Args:
            field_name: Name of field to check

        Returns:
            True if field is locked
        """
        # PHASE 10: Check both the MetadataField.locked AND the locked_fields list
        if field_name in self.locked_fields:
            return True

        if hasattr(self, field_name):
            field = getattr(self, field_name)
            if isinstance(field, MetadataField):
                return field.locked
        return False

    # ========== PHASE 10: BATCH FIELD LOCKING METHODS ==========

    def lock_fields(self, field_names: List[str]) -> int:
        """
        Lock multiple fields at once.

        Args:
            field_names: List of field names to lock

        Returns:
            Number of fields locked
        """
        locked_count = 0

        for field_name in field_names:
            # Add to locked_fields list if not already there
            if field_name not in self.locked_fields:
                self.locked_fields.append(field_name)
                locked_count += 1

            # Also lock the MetadataField if it exists
            if hasattr(self, field_name):
                field = getattr(self, field_name)
                if isinstance(field, MetadataField) and not field.locked:
                    field.locked = True

        logger.debug(f"Locked {locked_count} fields: {field_names}")
        return locked_count

    def unlock_fields(self, field_names: List[str]) -> int:
        """
        Unlock multiple fields at once.

        Args:
            field_names: List of field names to unlock

        Returns:
            Number of fields unlocked
        """
        unlocked_count = 0

        for field_name in field_names:
            # Remove from locked_fields list
            if field_name in self.locked_fields:
                self.locked_fields.remove(field_name)
                unlocked_count += 1

            # Also unlock the MetadataField if it exists
            if hasattr(self, field_name):
                field = getattr(self, field_name)
                if isinstance(field, MetadataField) and field.locked:
                    field.locked = False

        logger.debug(f"Unlocked {unlocked_count} fields: {field_names}")
        return unlocked_count

    def get_locked_fields(self) -> List[str]:
        """
        Get list of all locked field names.

        Returns:
            List of locked field names
        """
        return list(self.locked_fields)

    def is_field_locked(self, field_name: str) -> bool:
        """
        Check if a specific field is in the locked_fields list.

        This is the PHASE 10 method that checks the locked_fields list.
        The is_locked() method checks both this AND the MetadataField.locked.

        Args:
            field_name: Field name to check

        Returns:
            True if field is in locked_fields list
        """
        return field_name in self.locked_fields

    # ========== END PHASE 10 ==========

    # ========== PHASE 9: VERSION MANAGEMENT METHODS ==========

    def add_version(self, path: str, label: str = "", is_primary: bool = False, assets: List[str] = None) -> None:
        """
        Add a new version to this work.

        Args:
            path: Absolute path to the version directory
            label: Human-readable version label
            is_primary: Whether this is the primary version
            assets: List of asset tags present in this version
        """
        # Check if version already exists
        for version in self.versions:
            if version.path == path:
                # Update existing version
                if label:
                    version.label = label
                if is_primary:
                    version.is_primary = True
                if assets is not None:
                    version.assets = assets
                return

        # Add new version
        version = GameVersion(
            path=path,
            label=label,
            is_primary=is_primary or len(self.versions) == 0,  # First version is primary by default
            assets=assets or []
        )
        self.versions.append(version)

    def get_primary_version(self) -> Optional[GameVersion]:
        """
        Get the primary version.

        Returns:
            Primary GameVersion or None
        """
        for version in self.versions:
            if version.is_primary:
                return version
        # Fallback to first version
        return self.versions[0] if self.versions else None

    def get_version_by_path(self, path: str) -> Optional[GameVersion]:
        """
        Get version by path.

        Args:
            path: Path to search for

        Returns:
            GameVersion or None
        """
        for version in self.versions:
            if version.path == path:
                return version
        return None

    def migrate_legacy_data(self, legacy_path: str) -> None:
        """
        Migrate legacy single-path data to versioned model.

        Args:
            legacy_path: The old single game directory path
        """
        if not self.versions:
            # Add legacy path as first (primary) version
            self.add_version(
                path=legacy_path,
                label="Legacy",
                is_primary=True,
                assets=self.assets_detected
            )

    # ========== END PHASE 9 ==========

    def to_dict(self) -> Dict[str, Any]:
        """
        Convert to dictionary for JSON serialization.

        Returns:
            Dictionary representation of metadata
        """
        return self.model_dump()


# Factory functions for common operations

def create_empty_metadata() -> UnifiedMetadata:
    """Create an empty UnifiedMetadata object with default values."""
    return UnifiedMetadata()


def create_metadata_from_vndb(
    vndb_id: str,
    titles: Dict[str, str],
    description: str,
    rating: float,
    tags: List[str],
    image_url: str,
    release_date: str = ""
) -> UnifiedMetadata:
    """
    Create UnifiedMetadata from VNDB API response.

    Args:
        vndb_id: VNDB identifier
        titles: Dictionary of titles in different languages
        description: Game description
        rating: Rating score (0-10)
        tags: List of tags
        image_url: Cover image URL
        release_date: Release date

    Returns:
        UnifiedMetadata object
    """
    # Build multilingual title
    multi_title = MultilingualTitle(
        ja=titles.get("ja", ""),
        en=titles.get("en", ""),
        zh_hans=titles.get("zh-Hans", ""),
        zh_hant=titles.get("zh-Hant", ""),
        original=titles.get("original", titles.get("ja", ""))
    )

    # Build rating
    rating_obj = Rating(score=rating, count=0, source="vndb")

    # Create metadata
    return UnifiedMetadata(
        vndb_id=vndb_id,
        title=MetadataField(value=multi_title, source="vndb"),
        description=MetadataField(value=description, source="vndb"),
        rating=MetadataField(value=rating_obj, source="vndb"),
        cover_url=MetadataField(value=image_url, source="vndb"),
        tags=MetadataField(value=tags, source="vndb"),
        release_date=MetadataField(value=release_date, source="vndb"),
        providers=["vndb"]
    )
