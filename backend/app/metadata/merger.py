"""
Metadata merger with field-level locking logic.

Intelligently merges metadata from multiple providers while respecting
manual edits (locked fields).
"""

import logging
from typing import Dict, Any, List, Optional, TypeVar, Generic
from datetime import datetime

from .models import (
    UnifiedMetadata,
    MetadataField,
    MultilingualTitle,
    Rating
)

logger = logging.getLogger(__name__)

T = TypeVar('T')


class MetadataMerger:
    """
    Merges metadata from multiple providers with field-level locking.

    Logic:
    1. If current field is locked -> KEEP current (never overwrite)
    2. If new provider has data -> Update with new data
    3. Track changes for review
    """

    def __init__(self, prefer_traditional: bool = True):
        """
        Initialize merger.

        Args:
            prefer_traditional: If True, prefer Traditional Chinese titles
        """
        self.prefer_traditional = prefer_traditional
        self.changes_made: List[str] = []

    def merge_field(
        self,
        current: MetadataField[T],
        new_value: Optional[T],
        new_source: str,
        field_name: str,
        locked_fields: Optional[List[str]] = None
    ) -> MetadataField[T]:
        """
        Merge a single metadata field with locking logic.

        PHASE 10: Now checks both MetadataField.locked AND locked_fields list.

        Args:
            current: Current field value
            new_value: New value from provider
            new_source: Provider name
            field_name: Field name for logging
            locked_fields: List of field names locked at metadata level

        Returns:
            Updated (or unchanged) MetadataField
        """
        # Check if field is locked (PHASE 10: Check both sources)
        if current.locked:
            logger.debug(f"Field '{field_name}' is locked (MetadataField), skipping update")
            return current

        if locked_fields and field_name in locked_fields:
            logger.debug(f"Field '{field_name}' is locked (locked_fields list), skipping update")
            return current

        # Check if new value is available
        if new_value is None or new_value == "" or (isinstance(new_value, list) and len(new_value) == 0):
            return current

        # Check if value actually changed
        if current.value == new_value:
            return current

        # Update field
        updated = MetadataField(
            value=new_value,
            source=new_source,
            locked=current.locked,  # Preserve lock status
            last_updated=datetime.now().isoformat()
        )

        self.changes_made.append(f"{field_name}: {current.value} -> {new_value} (source: {new_source})")
        logger.info(f"Updated field '{field_name}' from {current.source} to {new_source}")

        return updated

    def merge_multilingual_title(
        self,
        current: MetadataField[MultilingualTitle],
        new_titles: Dict[str, str],
        new_source: str
    ) -> MetadataField[MultilingualTitle]:
        """
        Merge multilingual titles with locking logic.

        Args:
            current: Current title field
            new_titles: New titles dictionary (may have partial data)
            new_source: Provider name

        Returns:
            Updated MetadataField with merged titles
        """
        # Check if field is locked
        if current.locked:
            logger.debug("Title field is locked, skipping update")
            return current

        # Merge titles
        current_titles = current.value
        merged_titles_dict = {
            "ja": current_titles.ja,
            "en": current_titles.en,
            "zh_hans": current_titles.zh_hans,
            "zh_hant": current_titles.zh_hant,
            "original": current_titles.original
        }

        # Update with new titles (only if provided)
        updated_any = False
        for key, value in new_titles.items():
            if value and value != "":
                # Map common key names
                key_mapped = self._normalize_title_key(key)
                if merged_titles_dict.get(key_mapped) != value:
                    merged_titles_dict[key_mapped] = value
                    updated_any = True

        if not updated_any:
            return current

        # Create merged title object
        merged_title = MultilingualTitle(**merged_titles_dict)

        updated = MetadataField(
            value=merged_title,
            source=new_source,
            locked=current.locked,
            last_updated=datetime.now().isoformat()
        )

        self.changes_made.append(f"title: Updated from {new_source}")
        logger.info(f"Updated title from {new_source}")

        return updated

    def _normalize_title_key(self, key: str) -> str:
        """Normalize title key to match MultilingualTitle fields."""
        key_mapping = {
            "ja": "ja",
            "jp": "ja",
            "japanese": "ja",
            "en": "en",
            "english": "en",
            "zh-hans": "zh_hans",
            "zh-hans-cn": "zh_hans",
            "zh-hant": "zh_hant",
            "zh-hant-tw": "zh_hant",
            "original": "original",
            "orig": "original"
        }
        return key_mapping.get(key.lower(), key)

    def merge_rating(
        self,
        current: MetadataField[Rating],
        new_score: float,
        new_count: int = 0,
        new_source: str = "unknown"
    ) -> MetadataField[Rating]:
        """
        Merge rating with locking logic.

        Args:
            current: Current rating field
            new_score: New rating score (0-10)
            new_count: New rating count
            new_source: Provider name

        Returns:
            Updated MetadataField with new rating
        """
        # Check if field is locked
        if current.locked:
            logger.debug("Rating field is locked, skipping update")
            return current

        # Create new rating object
        new_rating = Rating(
            score=new_score,
            count=max(current.value.count, new_count),  # Keep higher count
            source=new_source
        )

        # Check if score actually changed
        if abs(current.value.score - new_score) < 0.1:
            return current

        updated = MetadataField(
            value=new_rating,
            source=new_source,
            locked=current.locked,
            last_updated=datetime.now().isoformat()
        )

        self.changes_made.append(f"rating: {current.value.score} -> {new_score} (source: {new_source})")
        logger.info(f"Updated rating from {current.value.score} to {new_score} (source: {new_source})")

        return updated

    def merge_metadata(
        self,
        current: UnifiedMetadata,
        new_data: Dict[str, Any],
        new_source: str
    ) -> UnifiedMetadata:
        """
        Merge new metadata into existing metadata.

        PHASE 10: Now respects locked_fields list from metadata.

        Args:
            current: Current metadata
            new_data: New data from provider (dict with optional fields)
            new_source: Provider name (e.g., "vndb", "steam")

        Returns:
            Updated UnifiedMetadata with merged data
        """
        self.changes_made = []  # Reset changes tracker

        # PHASE 10: Extract locked_fields list
        locked_fields = getattr(current, 'locked_fields', [])

        # Merge title
        if "title" in new_data:
            current.title = self.merge_multilingual_title(
                current.title,
                new_data["title"],
                new_source
            )

        # Merge description
        if "description" in new_data:
            current.description = self.merge_field(
                current.description,
                new_data["description"],
                new_source,
                "description",
                locked_fields
            )

        # Merge rating
        if "rating" in new_data:
            rating_data = new_data["rating"]
            score = rating_data if isinstance(rating_data, (int, float)) else rating_data.get("score", 0.0)
            count = rating_data.get("count", 0) if isinstance(rating_data, dict) else 0
            current.rating = self.merge_rating(
                current.rating,
                score,
                count,
                new_source
            )

        # Merge cover URL
        if "cover_url" in new_data:
            current.cover_url = self.merge_field(
                current.cover_url,
                new_data["cover_url"],
                new_source,
                "cover_url",
                locked_fields
            )

        # Merge screenshot URLs
        if "screenshot_urls" in new_data:
            current.screenshot_urls = self.merge_field(
                current.screenshot_urls,
                new_data["screenshot_urls"],
                new_source,
                "screenshot_urls",
                locked_fields
            )

        # Merge tags
        if "tags" in new_data:
            current.tags = self.merge_field(
                current.tags,
                new_data["tags"],
                new_source,
                "tags",
                locked_fields
            )

        # Merge release date
        if "release_date" in new_data:
            current.release_date = self.merge_field(
                current.release_date,
                new_data["release_date"],
                new_source,
                "release_date",
                locked_fields
            )

        # Merge developer
        if "developer" in new_data:
            current.developer = self.merge_field(
                current.developer,
                new_data["developer"],
                new_source,
                "developer",
                locked_fields
            )

        # Update providers list
        if new_source not in current.providers:
            current.providers.append(new_source)

        # Update sync timestamp
        current.last_sync = datetime.now().isoformat()

        return current

    def get_changes(self) -> List[str]:
        """
        Get list of changes made during last merge.

        Returns:
            List of change descriptions
        """
        return self.changes_made.copy()

    def has_changes(self) -> bool:
        """
        Check if any changes were made during last merge.

        Returns:
            True if changes were made
        """
        return len(self.changes_made) > 0


# Convenience functions

def merge_metadata(
    current: UnifiedMetadata,
    new_data: Dict[str, Any],
    new_source: str,
    prefer_traditional: bool = True
) -> tuple[UnifiedMetadata, List[str]]:
    """
    Merge metadata from new provider.

    Args:
        current: Current metadata
        new_data: New data from provider
        new_source: Provider name
        prefer_traditional: Prefer Traditional Chinese

    Returns:
        Tuple of (updated_metadata, list_of_changes)
    """
    merger = MetadataMerger(prefer_traditional=prefer_traditional)
    updated = merger.merge_metadata(current, new_data, new_source)
    changes = merger.get_changes()

    return updated, changes


def can_update_field(field: MetadataField[T]) -> bool:
    """
    Check if a field can be updated (not locked).

    Args:
        field: MetadataField to check

    Returns:
        True if field can be updated
    """
    return not field.locked
