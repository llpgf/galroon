"""
Scanner Service Package

Heuristics and intelligent file scanning for Galgame library.
Sprint 2: Added ScanCandidate workflow for confirmation.
"""

from .heuristics import HeuristicEngine, ScanCandidate, ScanStatus
from .engine import ScannerService, get_scanner, ScanProgressEvent, ScanPhase

__all__ = [
    "HeuristicEngine",
    "ScanCandidate",
    "ScanStatus",
    "ScanPhase",
    "ScannerService",
    "get_scanner",
    "ScanProgressEvent"
]
