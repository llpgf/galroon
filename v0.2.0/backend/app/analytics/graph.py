"""
Knowledge Graph API for Galgame Library Manager.

**PHASE 11: Knowledge Graph**

Provides relationship exploration through:
- Staff search (scenario, art, music, etc.)
- Cast search (voice actors)
- Series/franchise linking
"""

import logging
from pathlib import Path
from typing import Dict, Any, List, Optional
from collections import defaultdict

from ..config import get_config
from ..metadata.manager import get_resource_manager
from ..metadata.models import UnifiedMetadata

logger = logging.getLogger(__name__)


class KnowledgeGraphEngine:
    """
    Builds and queries the knowledge graph.

    Relationships:
    - Staff: Works grouped by role (scenario, art, music, etc.)
    - Cast: Voice actors and their characters
    - Series: Games in the same franchise
    """

    def __init__(self):
        """Initialize graph engine."""
        self.config = get_config()
        self.library_roots = self.config.library_roots

    def load_all_metadata(self) -> List[UnifiedMetadata]:
        """
        Load all metadata from library roots.

        Returns:
            List of all metadata objects
        """
        all_metadata = []

        for library_root in self.library_roots:
            if not library_root.exists():
                logger.warning(f"Library root does not exist: {library_root}")
                continue

            resource_manager = get_resource_manager(library_root, quota_gb=2.0)

            # Find all metadata.json files
            for metadata_file in library_root.rglob("metadata.json"):
                try:
                    metadata_dict = resource_manager.load_metadata(metadata_file.parent)
                    if metadata_dict:
                        metadata = UnifiedMetadata(**metadata_dict)
                        all_metadata.append(metadata)
                except Exception as e:
                    logger.warning(f"Failed to load metadata from {metadata_file}: {e}")

        logger.info(f"Loaded {len(all_metadata)} metadata objects")
        return all_metadata

    def search_staff(self, name: str) -> Dict[str, Any]:
        """
        Search for staff member across all games.

        Returns games grouped by role (scenario, art, music, etc.).

        Args:
            name: Staff name to search (case-insensitive, partial match)

        Returns:
            Dict with staff name, total_games, and games_by_role
        """
        metadata_list = self.load_all_metadata()

        name_lower = name.lower()
        games_by_role = defaultdict(list)
        total_games = 0
        matched_names = set()

        for metadata in metadata_list:
            # Extract credits
            credits = metadata.credits.value if metadata.credits else []

            if not isinstance(credits, list):
                continue

            found_in_this_game = False

            for credit in credits:
                if not isinstance(credit, dict):
                    continue

                # Extract role and staff list
                role = credit.get("role", "")
                staff_list = credit.get("staff", [])

                if not isinstance(staff_list, list):
                    continue

                # Search for matching staff
                for staff in staff_list:
                    if not isinstance(staff, str):
                        continue

                    # Check if name matches (case-insensitive, partial)
                    if name_lower in staff.lower():
                        matched_names.add(staff)
                        found_in_this_game = True

                        # Add game to this role
                        game_info = {
                            "vndb_id": metadata.external_ids.vndb if metadata.external_ids else None,
                            "title": metadata.title.value.original if metadata.title else "Unknown",
                            "folder_path": str(metadata.folder_path) if hasattr(metadata, 'folder_path') else None,
                            "year": metadata.release_date.value[:4] if metadata.release_date and metadata.release_date.value else None
                        }

                        games_by_role[role].append(game_info)
                        break

            if found_in_this_game:
                total_games += 1

        # Sort games in each role by title
        for role in games_by_role:
            games_by_role[role].sort(key=lambda x: x["title"])

        # Use the most common matched name as the canonical name
        canonical_name = name if not matched_names else sorted(matched_names, key=len, reverse=True)[0]

        return {
            "name": canonical_name,
            "matched_names": list(matched_names),
            "total_games": total_games,
            "games_by_role": dict(games_by_role)
        }

    def search_cast(self, name: str) -> Dict[str, Any]:
        """
        Search for voice actor across all games.

        Returns games with characters voiced.

        Args:
            name: Voice actor name to search (case-insensitive, partial match)

        Returns:
            Dict with actor name, total_characters, and games
        """
        metadata_list = self.load_all_metadata()

        name_lower = name.lower()
        games = []
        total_characters = 0
        matched_names = set()

        for metadata in metadata_list:
            # Extract credits
            credits = metadata.credits.value if metadata.credits else []

            if not isinstance(credits, list):
                continue

            # Find cast section
            cast_credit = None
            for credit in credits:
                if isinstance(credit, dict) and credit.get("role") == "cast":
                    cast_credit = credit
                    break

            if not cast_credit:
                continue

            cast_list = cast_credit.get("staff", [])

            if not isinstance(cast_list, list):
                continue

            # Find matching characters for this actor
            characters = []

            for cast_entry in cast_list:
                if not isinstance(cast_entry, dict):
                    continue

                # Extract actor and character
                actor = cast_entry.get("actor", "")
                character = cast_entry.get("character", "Unknown")

                if not isinstance(actor, str):
                    continue

                # Check if actor matches
                if name_lower in actor.lower():
                    matched_names.add(actor)
                    characters.append(character)

            if characters:
                total_characters += len(characters)
                games.append({
                    "vndb_id": metadata.external_ids.vndb if metadata.external_ids else None,
                    "title": metadata.title.value.original if metadata.title else "Unknown",
                    "folder_path": str(metadata.folder_path) if hasattr(metadata, 'folder_path') else None,
                    "year": metadata.release_date.value[:4] if metadata.release_date and metadata.release_date.value else None,
                    "characters": characters
                })

        # Sort games by title
        games.sort(key=lambda x: x["title"])

        # Use the most common matched name as the canonical name
        canonical_name = name if not matched_names else sorted(matched_names, key=len, reverse=True)[0]

        return {
            "name": canonical_name,
            "matched_names": list(matched_names),
            "total_characters": total_characters,
            "games": games
        }

    def search_series(self, series_name: str) -> Dict[str, Any]:
        """
        Search for games in a series/franchise.

        Uses fuzzy matching on title and aliases.

        Args:
            series_name: Series name to search (case-insensitive, partial match)

        Returns:
            Dict with series name, total_games, and games sorted by year
        """
        metadata_list = self.load_all_metadata()

        series_lower = series_name.lower()
        games = []
        matched_aliases = set()

        for metadata in metadata_list:
            # Check title
            title = metadata.title.value.original if metadata.title else ""
            aliases = metadata.aliases.value if metadata.aliases else []

            # Check if title or any alias matches series name
            matched = False

            # Check title
            if series_lower in title.lower():
                matched = True

            # Check aliases
            if not matched and isinstance(aliases, list):
                for alias in aliases:
                    if isinstance(alias, str) and series_lower in alias.lower():
                        matched = True
                        matched_aliases.add(alias)
                        break

            if matched:
                # Extract year
                year = None
                if metadata.release_date and metadata.release_date.value:
                    try:
                        year = int(metadata.release_date.value[:4])
                    except (ValueError, TypeError):
                        pass

                games.append({
                    "vndb_id": metadata.external_ids.vndb if metadata.external_ids else None,
                    "title": title,
                    "folder_path": str(metadata.folder_path) if hasattr(metadata, 'folder_path') else None,
                    "year": year,
                    "developers": metadata.developer.value if metadata.developer else [],
                    "engine": metadata.engine.value if metadata.engine else None
                })

        # Sort games by year
        games.sort(key=lambda x: x["year"] or 0)

        return {
            "name": series_name,
            "matched_aliases": list(matched_aliases),
            "total_games": len(games),
            "games": games
        }


# Singleton instance
_graph_engine: Optional[KnowledgeGraphEngine] = None


def get_graph_engine() -> KnowledgeGraphEngine:
    """
    Get or create graph engine singleton.

    Returns:
        KnowledgeGraphEngine instance
    """
    global _graph_engine
    if _graph_engine is None:
        _graph_engine = KnowledgeGraphEngine()
    return _graph_engine
