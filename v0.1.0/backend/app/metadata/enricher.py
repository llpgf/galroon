"""
Metadata Enrichment Engine for Galgame Library Manager.

**PHASE 12: The Connector - Enrichment Engine**

Coordinates external connectors to enrich metadata:
- Steam integration (high-res assets, Steam ID)
- Bangumi integration (Chinese metadata, rating)

Implements waterfall enrichment logic with locked field respect.
"""

import logging
from pathlib import Path
from typing import Dict, Any, Optional, List
from datetime import datetime

from .models import UnifiedMetadata, MetadataField, ExternalIDs
from .manager import get_resource_manager
from ..connectors.steam import get_steam_connector
from ..connectors.bangumi import get_bangumi_connector

logger = logging.getLogger(__name__)


class EnrichmentResult:
    """Result of enrichment operation."""
    def __init__(
        self,
        success: bool,
        steam_id: Optional[str] = None,
        bangumi_id: Optional[str] = None,
        assets_added: List[str] = None,
        fields_updated: List[str] = None,
        message: str = ""
    ):
        self.success = success
        self.steam_id = steam_id
        self.bangumi_id = bangumi_id
        self.assets_added = assets_added or []
        self.fields_updated = fields_updated or []
        self.message = message


class MetadataEnricher:
    """
    Enriches metadata from external sources.

    Waterfall Logic:
    1. If external_ids.steam is missing -> Search Steam -> Save ID
    2. If visuals.background is missing -> Fetch from Steam -> Download
    3. If description is not Chinese -> Search Bangumi -> Update

    Respects locked fields (Phase 10).
    """

    def __init__(
        self,
        library_root: Path,
        rate_limit_delay: float = 1.0,
        download_assets: bool = True
    ):
        """
        Initialize enricher.

        Args:
            library_root: Library root directory
            rate_limit_delay: Delay between API requests
            download_assets: Whether to download assets or just URLs
        """
        self.library_root = library_root
        self.rate_limit_delay = rate_limit_delay
        self.download_assets = download_assets

        # Get connectors
        self.steam_connector = get_steam_connector(rate_limit_delay=rate_limit_delay)
        self.bangumi_connector = get_bangumi_connector(rate_limit_delay=rate_limit_delay)

        # Get resource manager
        self.resource_manager = get_resource_manager(library_root, quota_gb=2.0)

    def _is_chinese_text(self, text: str) -> bool:
        """
        Check if text contains Chinese characters.

        Args:
            text: Text to check

        Returns:
            True if text contains Chinese
        """
        if not text:
            return False

        # Check for CJK Unified Ideographs block
        for char in text:
            if '\u4e00' <= char <= '\u9fff':
                return True

        return False

    def _extract_titles(self, metadata: UnifiedMetadata) -> tuple:
        """
        Extract all title variants from metadata.

        Args:
            metadata: UnifiedMetadata object

        Returns:
            Tuple of (title, title_cn, title_ja)
        """
        if not metadata.title:
            return None, None, None

        title_obj = metadata.title.value

        title = title_obj.original or ""
        title_cn = title_obj.zh_hans or title_obj.zh_hant or ""
        title_ja = title_obj.ja or ""

        return title, title_cn, title_ja

    def enrich_steam_id(
        self,
        metadata: UnifiedMetadata,
        force: bool = False
    ) -> Optional[str]:
        """
        Enrich Steam ID if missing.

        Args:
            metadata: Metadata object to enrich
            force: Force re-search even if ID exists

        Returns:
            Steam ID if found, else None
        """
        # Check if already exists (unless force)
        if not force and metadata.external_ids and metadata.external_ids.steam:
            logger.debug("Steam ID already exists, skipping")
            return metadata.external_ids.steam

        # Extract titles
        title, title_cn, title_ja = self._extract_titles(metadata)

        if not title:
            logger.warning("No title available for Steam search")
            return None

        # Search Steam
        result = self.steam_connector.search_by_title(title)

        if not result:
            logger.info(f"Steam not found: {title}")
            return None

        steam_id = result["steam_id"]
        logger.info(f"Found Steam ID: {steam_id} for {title}")

        return steam_id

    def enrich_steam_assets(
        self,
        metadata: UnifiedMetadata,
        metadata_dir: Path,
        force: bool = False
    ) -> List[str]:
        """
        Fetch Steam assets if missing.

        Args:
            metadata: Metadata object
            metadata_dir: .metadata/ directory
            force: Force re-download

        Returns:
            List of asset paths added
        """
        assets_added = []

        # Check if background exists (unless force)
        if not force and metadata.visuals and metadata.visuals.background:
            logger.debug("Background already exists, skipping Steam assets")
            return assets_added

        # Get Steam ID
        steam_id = metadata.external_ids.steam if metadata.external_ids else None

        if not steam_id:
            # Try to find Steam ID first
            steam_id = self.enrich_steam_id(metadata)
            if not steam_id:
                return assets_added

        # Fetch assets
        assets = self.steam_connector.fetch_assets(
            steam_id=steam_id,
            metadata_dir=metadata_dir,
            download=self.download_assets
        )

        # Record assets added
        if assets.get("header"):
            assets_added.append("header")

        if assets.get("background"):
            assets_added.append("background")

        if assets.get("screenshots"):
            assets_added.append(f"screenshots ({len(assets['screenshots'])})")

        logger.info(f"Steam assets added: {assets_added}")
        return assets_added

    def enrich_bangumi_metadata(
        self,
        metadata: UnifiedMetadata,
        force: bool = False
    ) -> Dict[str, Any]:
        """
        Enrich Chinese metadata from Bangumi.

        Args:
            metadata: Metadata object
            force: Force update even if Chinese exists

        Returns:
            Dict with bangumi_id, summary_cn, rating_score
        """
        # Check if description is already Chinese (unless force)
        if not force and metadata.description and metadata.description.value:
            current_desc = metadata.description.value
            if self._is_chinese_text(current_desc):
                logger.debug("Chinese description already exists, skipping Bangumi")
                return {}

        # Check if already has Bangumi ID
        if not force and metadata.external_ids and metadata.external_ids.bangumi:
            bangumi_id = metadata.external_ids.bangumi
            logger.info(f"Already has Bangumi ID: {bangumi_id}")
            # Could fetch fresh data here if needed
            return {
                "bangumi_id": bangumi_id,
                "summary_cn": None,  # Already have it
                "rating_score": None
            }

        # Extract titles
        title, title_cn, title_ja = self._extract_titles(metadata)

        if not title:
            logger.warning("No title available for Bangumi search")
            return {}

        # Search Bangumi
        result = self.bangumi_connector.get_chinese_metadata(
            title=title,
            title_cn=title_cn,
            title_ja=title_ja
        )

        if not result.get("success"):
            logger.info(f"Bangumi not found: {title}")
            return {}

        logger.info(f"Found Bangumi ID: {result['bangumi_id']} for {title}")

        return {
            "bangumi_id": result.get("bangumi_id"),
            "name_cn": result.get("name_cn"),
            "summary_cn": result.get("summary_cn"),
            "rating_score": result.get("rating_score"),
            "rating_count": result.get("rating_count")
        }

    def enrich_game(
        self,
        game_folder: Path,
        force_steam: bool = False,
        force_bangumi: bool = False,
        skip_locked: bool = True
    ) -> EnrichmentResult:
        """
        Complete enrichment workflow for a game.

        Waterfall:
        1. Enrich Steam ID (if missing)
        2. Enrich Steam assets (if missing)
        3. Enrich Bangumi Chinese metadata (if not Chinese)

        Args:
            game_folder: Path to game folder
            force_steam: Force Steam enrichment
            force_bangumi: Force Bangumi enrichment
            skip_locked: Skip locked fields (Phase 10)

        Returns:
            EnrichmentResult with changes made
        """
        try:
            # Load metadata
            metadata_dict = self.resource_manager.load_metadata(game_folder)

            if not metadata_dict:
                return EnrichmentResult(
                    success=False,
                    message=f"No metadata found for: {game_folder}"
                )

            metadata = UnifiedMetadata(**metadata_dict)

            # Track changes
            steam_id = metadata.external_ids.steam if metadata.external_ids else None
            bangumi_id = metadata.external_ids.bangumi if metadata.external_ids else None
            assets_added = []
            fields_updated = []

            # Check if description is locked
            desc_locked = skip_locked and metadata.is_field_locked("description")

            # Step 1: Enrich Steam ID
            if steam_id is None or force_steam:
                new_steam_id = self.enrich_steam_id(metadata, force=force_steam)
                if new_steam_id and new_steam_id != steam_id:
                    # Update external_ids
                    if not metadata.external_ids:
                        metadata.external_ids = ExternalIDs()

                    metadata.external_ids.steam = new_steam_id
                    steam_id = new_steam_id
                    fields_updated.append("external_ids.steam")

            # Step 2: Enrich Steam Assets
            metadata_dir = game_folder / ".metadata" / "visuals"
            new_assets = self.enrich_steam_assets(
                metadata,
                metadata_dir,
                force=force_steam
            )
            assets_added.extend(new_assets)

            # Step 3: Enrich Bangumi (if description not locked and not Chinese)
            if not desc_locked:
                bangumi_data = self.enrich_bangumi_metadata(
                    metadata,
                    force=force_bangumi
                )

                if bangumi_data and bangumi_data.get("bangumi_id"):
                    # Update external_ids
                    if not metadata.external_ids:
                        metadata.external_ids = ExternalIDs()

                    metadata.external_ids.bangumi = bangumi_data["bangumi_id"]
                    bangumi_id = bangumi_data["bangumi_id"]
                    fields_updated.append("external_ids.bangumi")

                    # Update Chinese description if available
                    if bangumi_data.get("summary_cn"):
                        # Preserve source and lock status
                        old_source = metadata.description.source if metadata.description else "bangumi"
                        old_locked = metadata.description.locked if metadata.description else False

                        metadata.description = MetadataField(
                            value=bangumi_data["summary_cn"],
                            source="bangumi",
                            locked=old_locked,
                            last_updated=datetime.now().isoformat()
                        )
                        fields_updated.append("description")

                    # Update rating from Bangumi if higher
                    if bangumi_data.get("rating_score"):
                        # Could merge with existing rating
                        pass

            # Save updated metadata
            if fields_updated or assets_added:
                self.resource_manager.save_metadata(metadata.model_dump(), game_folder)
                logger.info(f"Enriched {game_folder.name}: {fields_updated}, {assets_added}")

            return EnrichmentResult(
                success=True,
                steam_id=steam_id,
                bangumi_id=bangumi_id,
                assets_added=assets_added,
                fields_updated=fields_updated,
                message=f"Enriched: {', '.join(fields_updated)} | Assets: {', '.join(assets_added)}"
            )

        except Exception as e:
            logger.error(f"Error enriching {game_folder}: {e}")
            return EnrichmentResult(
                success=False,
                message=f"Error: {str(e)}"
            )

    def enrich_library(
        self,
        game_folders: List[Path] = None,
        force_steam: bool = False,
        force_bangumi: bool = False
    ) -> Dict[str, Any]:
        """
        Enrich multiple games in the library.

        Args:
            game_folders: List of game folders (None = scan entire library)
            force_steam: Force Steam enrichment
            force_bangumi: Force Bangumi enrichment

        Returns:
            Dict with enrichment statistics
        """
        # If no folders provided, scan library
        if not game_folders:
            game_folders = []
            for metadata_file in self.library_root.rglob("metadata.json"):
                if metadata_file.parent != self.library_root:
                    game_folders.append(metadata_file.parent)

        logger.info(f"Starting enrichment for {len(game_folders)} games")

        results = {
            "total": len(game_folders),
            "processed": 0,
            "success": 0,
            "failed": 0,
            "steam_added": 0,
            "bangumi_added": 0,
            "assets_downloaded": 0,
            "errors": []
        }

        for game_folder in game_folders:
            result = self.enrich_game(
                game_folder,
                force_steam=force_steam,
                force_bangumi=force_bangumi
            )

            results["processed"] += 1

            if result.success:
                results["success"] += 1
                if result.steam_id:
                    results["steam_added"] += 1
                if result.bangumi_id:
                    results["bangumi_added"] += 1
                if result.assets_added:
                    results["assets_downloaded"] += len(result.assets_added)
            else:
                results["failed"] += 1
                results["errors"].append({
                    "folder": str(game_folder),
                    "message": result.message
                })

        logger.info(f"Enrichment complete: {results['success']}/{results['total']} succeeded")
        return results


# Singleton instance
_enricher: Optional[MetadataEnricher] = None


def get_enricher(
    library_root: Path,
    rate_limit_delay: float = 1.0,
    download_assets: bool = True
) -> MetadataEnricher:
    """
    Get or create enricher singleton.

    Args:
        library_root: Library root directory
        rate_limit_delay: Delay between API requests
        download_assets: Whether to download assets

    Returns:
        MetadataEnricher instance
    """
    global _enricher
    # Create new instance if library root changes
    if _enricher is None or _enricher.library_root != library_root:
        _enricher = MetadataEnricher(
            library_root=library_root,
            rate_limit_delay=rate_limit_delay,
            download_assets=download_assets
        )
    return _enricher
