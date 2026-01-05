"""
Visual Stats Engine for Galgame Library Manager.

**PHASE 11: Visual Statistics**

Provides aggregated statistics for the Focus dashboard:
- Timeline: Distribution by release year
- Engines: Count by game engine
- Play Time: Distribution by length
- Tags: Weighted tag cloud
"""

import logging
from pathlib import Path
from typing import Dict, Any, List, Optional
from collections import Counter, defaultdict
from datetime import datetime

from ..config import get_config
from ..metadata.manager import get_resource_manager
from ..metadata.models import UnifiedMetadata

logger = logging.getLogger(__name__)


class VisualStatsEngine:
    """
    Aggregates visual statistics across the entire library.

    Uses in-memory aggregation for speed with small-to-medium libraries.
    For large libraries (1000+ games), consider caching.
    """

    # Play time length definitions (in hours)
    LENGTH_BUCKETS = {
        "Very Short": (0, 2),
        "Short": (2, 10),
        "Medium": (10, 30),
        "Long": (30, 50),
        "Epic": (50, 9999)
    }

    def __init__(self):
        """Initialize stats engine."""
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

    def aggregate_timeline(self, metadata_list: List[UnifiedMetadata]) -> Dict[str, int]:
        """
        Aggregate games by release year.

        Args:
            metadata_list: List of metadata objects

        Returns:
            Dict mapping year to count
        """
        timeline = Counter()

        for metadata in metadata_list:
            # Extract release date
            release_date = metadata.release_date.value if metadata.release_date else None

            if release_date:
                try:
                    # Parse ISO date string
                    if isinstance(release_date, str):
                        # Try parsing YYYY-MM-DD
                        if "-" in release_date:
                            year = release_date.split("-")[0]
                        else:
                            year = release_date[:4]
                    else:
                        continue

                    # Validate year
                    if year and year.isdigit() and len(year) == 4:
                        timeline[year] += 1

                except Exception as e:
                    logger.debug(f"Failed to parse release date: {release_date}, {e}")

        # Sort by year
        sorted_timeline = dict(sorted(timeline.items()))
        return sorted_timeline

    def aggregate_engines(self, metadata_list: List[UnifiedMetadata]) -> Dict[str, int]:
        """
        Aggregate games by engine.

        Args:
            metadata_list: List of metadata objects

        Returns:
            Dict mapping engine name to count
        """
        engines = Counter()

        for metadata in metadata_list:
            # Extract engine
            engine = metadata.engine.value if metadata.engine else None

            if engine and engine != "":
                engines[engine] += 1

        # Sort by count (descending)
        sorted_engines = dict(sorted(engines.items(), key=lambda x: x[1], reverse=True))
        return sorted_engines

    def aggregate_play_time(self, metadata_list: List[UnifiedMetadata]) -> Dict[str, int]:
        """
        Aggregate games by play time length.

        Args:
            metadata_list: List of metadata objects

        Returns:
            Dict mapping length bucket to count
        """
        play_time = Counter()

        for metadata in metadata_list:
            # Extract play time
            length = metadata.length.value if metadata.length else None

            if length and length != "":
                # Use the length as-is
                play_time[length] += 1

        # Sort by count (descending)
        sorted_play_time = dict(sorted(play_time.items(), key=lambda x: x[1], reverse=True))
        return sorted_play_time

    def aggregate_tags(
        self,
        metadata_list: List[UnifiedMetadata],
        top_n: int = 50
    ) -> List[Dict[str, Any]]:
        """
        Aggregate tags into weighted tag cloud.

        Args:
            metadata_list: List of metadata objects
            top_n: Number of top tags to return

        Returns:
            List of dicts with tag, count, and weight (0-1)
        """
        tag_counter = Counter()

        for metadata in metadata_list:
            # Extract tags
            tags = metadata.tags.value if metadata.tags else []

            if isinstance(tags, list):
                for tag in tags:
                    if isinstance(tag, str) and tag:
                        tag_counter[tag] += 1
                    elif isinstance(tag, dict):
                        # Handle tag objects with name field
                        tag_name = tag.get("name", "")
                        if tag_name:
                            tag_counter[tag_name] += 1

        # Get top N tags
        top_tags = tag_counter.most_common(top_n)

        if not top_tags:
            return []

        # Calculate weights (0-1 range)
        max_count = top_tags[0][1]
        min_count = top_tags[-1][1]

        tag_cloud = []
        for tag, count in top_tags:
            # Calculate weight (normalized)
            if max_count > min_count:
                weight = (count - min_count) / (max_count - min_count)
            else:
                weight = 1.0

            tag_cloud.append({
                "tag": tag,
                "count": count,
                "weight": round(weight, 3)
            })

        # Sort by count (descending)
        tag_cloud.sort(key=lambda x: x["count"], reverse=True)

        return tag_cloud

    def get_dashboard_stats(self) -> Dict[str, Any]:
        """
        Get all dashboard statistics.

        Returns:
            Dict with timeline, engines, play_time, tags, and summary
        """
        # Load all metadata
        metadata_list = self.load_all_metadata()

        if not metadata_list:
            return {
                "total_games": 0,
                "timeline": {},
                "engines": {},
                "play_time": {},
                "tags": []
            }

        # Aggregate stats
        timeline = self.aggregate_timeline(metadata_list)
        engines = self.aggregate_engines(metadata_list)
        play_time = self.aggregate_play_time(metadata_list)
        tags = self.aggregate_tags(metadata_list, top_n=50)

        return {
            "total_games": len(metadata_list),
            "timeline": timeline,
            "engines": engines,
            "play_time": play_time,
            "tags": tags
        }


# Singleton instance
_stats_engine: Optional[VisualStatsEngine] = None


def get_stats_engine() -> VisualStatsEngine:
    """
    Get or create stats engine singleton.

    Returns:
        VisualStatsEngine instance
    """
    global _stats_engine
    if _stats_engine is None:
        _stats_engine = VisualStatsEngine()
    return _stats_engine
