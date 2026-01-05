"""
Analytics package for Galgame Library Manager.

**PHASE 11: The Explorer Backend**

Provides:
- Visual statistics (timeline, engines, play time, tags)
- Knowledge graph (staff, cast, series)
- Advanced search
"""

from .stats import VisualStatsEngine, get_stats_engine
from .graph import KnowledgeGraphEngine, get_graph_engine

__all__ = [
    "VisualStatsEngine",
    "get_stats_engine",
    "KnowledgeGraphEngine",
    "get_graph_engine",
]
