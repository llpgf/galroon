"""
Waterfall Matcher for multi-source metadata matching.

Implements priority-based source matching with confidence thresholds.
"""

import logging
from typing import List, Dict, Any, Optional, Tuple
from pathlib import Path
from dataclasses import dataclass

logger = logging.getLogger(__name__)


@dataclass
class MatchCandidate:
    """A match candidate from a source."""
    source: str
    metadata: Dict[str, Any]
    confidence: float
    match_id: str  # ID from source (e.g., VNDB ID)


class WaterfallMatcher:
    """
    Multi-source metadata matcher with priority waterfall.

    Priority order:
    1. Manual (user-set metadata, highest priority)
    2. Local (existing metadata.json)
    3. VNDB (API matching with fuzzy search)
    4. Steam (future)
    5. BGM (future)

    Logic:
    - Iterate sources in priority order
    - IF confidence > 95% -> Return immediately
    - ELSE -> Collect as candidate
    - Return best candidate after waterfall
    """

    DEFAULT_CONFIDENCE_THRESHOLD = 95.0
    DEFAULT_SOURCE_PRIORITY = ['manual', 'local', 'vndb']

    def __init__(
        self,
        source_priority: Optional[List[str]] = None,
        confidence_threshold: float = DEFAULT_CONFIDENCE_THRESHOLD
    ):
        """
        Initialize Waterfall Matcher.

        Args:
            source_priority: Order of sources to try (highest first)
            confidence_threshold: Min confidence to accept immediately
        """
        self.source_priority = source_priority or self.DEFAULT_SOURCE_PRIORITY.copy()
        self.confidence_threshold = confidence_threshold

    def match(
        self,
        game_name: str,
        game_dir: Path,
        vndb_provider,
        resource_manager,
        prefer_traditional: bool = True
    ) -> Tuple[Optional[Dict[str, Any]], List[MatchCandidate]]:
        """
        Find best metadata match using waterfall strategy.

        Args:
            game_name: Game title to match
            game_dir: Game directory path
            vndb_provider: VNDB provider instance
            resource_manager: Resource manager instance
            prefer_traditional: Prefer Traditional Chinese

        Returns:
            Tuple of (best_match, all_candidates)
        """
        candidates: List[MatchCandidate] = []

        # Priority 1: Check for existing local metadata
        local_metadata = resource_manager.load_metadata(game_dir)
        if local_metadata:
            # Local metadata is always 100% confidence (user already approved)
            local_candidate = MatchCandidate(
                source='local',
                metadata=local_metadata,
                confidence=100.0,
                match_id=local_metadata.get('vndb_id', 'local')
            )
            candidates.append(local_candidate)

            # If local is locked/high-quality, return immediately
            if local_metadata.get('title', {}).get('locked', False):
                logger.info(f"Waterfall: Using locked local metadata for {game_name}")
                return local_metadata, candidates

        # Priority 2: VNDB API matching
        try:
            vndb_result = vndb_provider.fetch_and_parse(game_name, prefer_traditional)
            if vndb_result:
                # Get confidence from fuzzy match
                fuzzy_score = getattr(vndb_result, '_fuzzy_score', None)
                confidence = fuzzy_score if fuzzy_score else 85.0

                vndb_dict = vndb_result.model_dump()
                vndb_candidate = MatchCandidate(
                    source='vndb',
                    metadata=vndb_dict,
                    confidence=confidence,
                    match_id=vndb_dict.get('vndb_id', 'unknown')
                )
                candidates.append(vndb_candidate)

                # If high confidence match, return immediately
                if confidence >= self.confidence_threshold:
                    logger.info(f"Waterfall: High confidence VNDB match ({confidence:.0f}%) for {game_name}")
                    return vndb_dict, candidates

        except Exception as e:
            logger.warning(f"Waterfall: VNDB matching failed for {game_name}: {e}")

        # Sort candidates by confidence
        candidates.sort(key=lambda c: c.confidence, reverse=True)

        # Return best candidate (if any)
        if candidates:
            best = candidates[0]
            logger.info(f"Waterfall: Best match is {best.source} ({best.confidence:.0f}%) for {game_name}")
            return best.metadata, candidates

        logger.warning(f"Waterfall: No matches found for {game_name}")
        return None, candidates

    def get_best_candidate(
        self,
        candidates: List[MatchCandidate],
        min_confidence: float = 70.0
    ) -> Optional[MatchCandidate]:
        """
        Get best candidate from list.

        Args:
            candidates: List of candidates
            min_confidence: Minimum confidence threshold

        Returns:
            Best candidate or None
        """
        if not candidates:
            return None

        # Sort by confidence
        sorted_candidates = sorted(candidates, key=lambda c: c.confidence, reverse=True)

        best = sorted_candidates[0]
        if best.confidence >= min_confidence:
            return best

        return None

    def format_candidates_for_ui(
        self,
        candidates: List[MatchCandidate]
    ) -> List[Dict[str, Any]]:
        """
        Format candidates for frontend UI.

        Args:
            candidates: List of match candidates

        Returns:
            List of candidate dicts for UI
        """
        formatted = []

        for candidate in candidates:
            metadata = candidate.metadata
            title_data = metadata.get('title', {})
            title_value = title_data if isinstance(title_data, str) else title_data.get('value', {})

            # Get preferred title
            if isinstance(title_value, dict):
                title = title_value.get('zh_hant') or title_value.get('en') or title_value.get('ja', 'Unknown')
            else:
                title = str(title_value)

            formatted.append({
                'source': candidate.source,
                'confidence': candidate.confidence,
                'match_id': candidate.match_id,
                'title': title,
                'description': metadata.get('description', {}).get('value', '').get('value', '')[:200] if isinstance(metadata.get('description'), dict) else '',
                'rating': metadata.get('rating', {}).get('value', {}).get('score', 0) if isinstance(metadata.get('rating'), dict) else 0,
            })

        return formatted


# Singleton instance
_waterfall_matcher: Optional[WaterfallMatcher] = None


def get_waterfall_matcher(
    source_priority: Optional[List[str]] = None,
    confidence_threshold: float = 95.0
) -> WaterfallMatcher:
    """
    Get or create WaterfallMatcher singleton.

    Args:
        source_priority: Source priority list
        confidence_threshold: Confidence threshold

    Returns:
        WaterfallMatcher instance
    """
    global _waterfall_matcher
    if _waterfall_matcher is None:
        _waterfall_matcher = WaterfallMatcher(source_priority, confidence_threshold)
    return _waterfall_matcher
