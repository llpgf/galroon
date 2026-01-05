"""
Metadata management module.

Provides batch metadata scanning and downloading capabilities with:
- Multilingual title support (Babel)
- Field-level locking (Curator)
- Data normalization (OpenCC, HTML cleaning)
- VNDB provider integration
- Resource management (LocalFirst, Quota)
"""

from .models import (
    UnifiedMetadata,
    MetadataField,
    MultilingualTitle,
    Rating,
    LibraryStatus,  # Phase 19.6: Renamed from PlayStatus
    Character,
    Staff,
    create_empty_metadata,
    create_metadata_from_vndb,
)
from .normalizer import (
    TextNormalizer,
    clean_html,
    normalize_rating,
    to_traditional_chinese,
    to_simplified_chinese,
    sanitize_description,
)
from .merger import (
    MetadataMerger,
    merge_metadata,
    can_update_field,
)
from .manager import (
    ResourceManager,
    get_resource_manager,
)
from .providers import (
    VNDBProvider,
    get_vndb_provider,
)
from .batch import (
    BatchManager,
    BatchStatus,
    get_batch_manager,
)
from .waterfall import (
    WaterfallMatcher,
    MatchCandidate,
    get_waterfall_matcher,
)
from .enricher import (
    MetadataEnricher,
    EnrichmentResult,
    get_enricher,
)

__all__ = [
    # Models
    "UnifiedMetadata",
    "MetadataField",
    "MultilingualTitle",
    "Rating",
    "LibraryStatus",  # Phase 19.6: Renamed from PlayStatus
    "Character",
    "Staff",
    "create_empty_metadata",
    "create_metadata_from_vndb",
    # Normalizer
    "TextNormalizer",
    "clean_html",
    "normalize_rating",
    "to_traditional_chinese",
    "to_simplified_chinese",
    "sanitize_description",
    # Merger
    "MetadataMerger",
    "merge_metadata",
    "can_update_field",
    # Manager
    "ResourceManager",
    "get_resource_manager",
    # Providers
    "VNDBProvider",
    "get_vndb_provider",
    # Batch
    "BatchManager",
    "BatchStatus",
    "get_batch_manager",
    # Waterfall
    "WaterfallMatcher",
    "MatchCandidate",
    "get_waterfall_matcher",
    # Enricher
    "MetadataEnricher",
    "EnrichmentResult",
    "get_enricher",
]
