"""
Intelligent Scanner Service for Galgame Library

Non-blocking file scanner with candidate confirmation workflow.
"""

import asyncio
import os
import logging
from pathlib import Path
from typing import List, Optional, Dict, Any
from datetime import datetime
from concurrent.futures import ThreadPoolExecutor

from .heuristics import HeuristicEngine, ScanCandidate, ScanStatus
from ..core.database import get_database

logger = logging.getLogger(__name__)


class ScanPhase(Enum):
    """Scan phase for semantic progress tracking."""
    IDENTITY_RESOLUTION = "identity_resolution"
    CANDIDATE_ANALYSIS = "candidate_analysis"
    CONFIRMATION = "confirmation"
    MERGE = "merge"
    COMPLETED = "completed"


class ScanProgressEvent:
    """
    Scan progress event with semantic phase and confidence tracking.
    
    Roon-inspired: Not just "technical progress" but "what system is doing".
    """
    def __init__(
        self,
        current: int = 0,
        total: int = 0,
        phase: ScanPhase = ScanPhase.IDENTITY_RESOLUTION,
        message: str = "",
        current_item: str = "",
        confidence: Optional[float] = None,
        candidates_found: int = 0
    ):
        self.current = current
        self.total = total
        self.phase = phase
        self.message = message
        self.current_item = current_item
        self.confidence = confidence
        self.candidates_found = candidates_found
        self.percent_complete = (current / total * 100) if total > 0 else 0
        self.timestamp = datetime.now().isoformat()

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "current": self.current,
            "total": self.total,
            "percent_complete": self.percent_complete,
            "phase": self.phase.value,
            "message": self.message,
            "current_item": self.current_item,
            "timestamp": self.timestamp,
            "confidence": self.confidence,
            "candidates_found": self.candidates_found
        }


class ScannerService:
    """
    High-performance, non-blocking file scanner with candidate workflow.
    
    Architecture:
    - Scanner: Generates ScanCandidates (NOT Games)
    - Library: Confirms/Splits/Merges Candidates → Games
    - Scanner can NEVER silently insert to Games table
    
    Lifecycle:
    1. Identity Resolution (Scan)
    2. Candidate Analysis (Review)
    3. Confirmation (User action)
    4. Merge (Confirmed candidates → Games)
    
    Features:
    - Runs disk I/O in thread pool (non-blocking)
    - Streams semantic progress with phases
    - Uses HeuristicEngine for intelligent detection
    - Supports: Auto-confirm, Manual-override, Split, Merge
    """
    
    def __init__(self):
        self._executor = ThreadPoolExecutor(max_workers=4)
        self._is_scanning = False
        self._current_total = 0
        self._current_count = 0
        self._progress_callbacks = []
        
    def add_progress_callback(self, callback):
        """Register a callback for progress updates."""
        self._progress_callbacks.append(callback)
        
    def remove_progress_callback(self, callback):
        """Unregister a progress callback."""
        if callback in self._progress_callbacks:
            self._progress_callbacks.remove(callback)
    
    async def _emit_progress(self, event: ScanProgressEvent):
        """Emit progress to all registered callbacks."""
        for callback in self._progress_callbacks:
            try:
                await callback(event) if asyncio.iscoroutinefunction(callback) else callback(event)
            except Exception as e:
                logger.error(f"Error in progress callback: {e}")
    
    def _update_progress(
        self,
        phase: ScanPhase,
        message: str,
        current_item: str = "",
        confidence: Optional[float] = None,
        candidates_found: int = 0
    ):
        """Internal method to update and emit progress."""
        event = ScanProgressEvent(
            current=self._current_count,
            total=self._current_total,
            phase=phase,
            message=message,
            current_item=current_item,
            confidence=confidence,
            candidates_found=candidates_found
        )
        
        # Log with semantic context
        logger.info(f"[{phase.value}] {self._current_count}/{self._current_total} ({event.percent_complete:.1f}%) - {message}")
        
        # Emit to all callbacks
        loop = asyncio.get_event_loop()
        loop.run_until_complete(self._emit_progress(event))
    
    async def scan_directory(
        self,
        root_path: str,
        session: Any = None,
        auto_confirm: bool = False
    ) -> Dict[str, Any]:
        """
        Scan a directory and generate candidates for confirmation.
        
        IMPORTANT: This ONLY creates ScanCandidates.
        Does NOT insert into Games table.
        
        Args:
            root_path: Directory to scan
            session: Database session (optional, used if provided)
            auto_confirm: If True, auto-confirm all candidates
            
        Returns:
            Scan results with candidate list
        """
        root = Path(root_path)
        
        if not root.exists() or not root.is_dir():
            raise ValueError(f"Invalid directory: {root_path}")
        
        if self._is_scanning:
            logger.warning("Scan already in progress")
            return {
                "success": False,
                "message": "Scan already in progress",
                "status": "already_scanning"
            }
        
        self._is_scanning = True
        self._current_count = 0
        self._current_total = 0
        
        logger.info(f"Starting scan of: {root_path}")
        
        try:
            # PHASE 1: Identity Resolution
            self._update_progress(
                ScanPhase.IDENTITY_RESOLUTION,
                "Analyzing directory structure...",
                str(root)
            )
            
            loop = asyncio.get_event_loop()
            candidates: List[ScanCandidate] = await loop.run_in_executor(
                self._executor,
                HeuristicEngine.analyze_directory,
                root
            )
            
            self._current_total = len(candidates)
            
            self._update_progress(
                ScanPhase.CANDIDATE_ANALYSIS,
                f"Found {len(candidates)} potential games",
                "",
                candidates_found=len(candidates)
            )
            
            # Calculate average confidence
            if candidates:
                avg_confidence = sum(c.confidence_score for c in candidates) / len(candidates)
                self._update_progress(
                    ScanPhase.CANDIDATE_ANALYSIS,
                    f"Average detection confidence: {avg_confidence:.2%}",
                    "",
                    confidence=avg_confidence,
                    candidates_found=len(candidates)
                )
            
            # Phase 2: Candidate Analysis (just found, no DB check yet)
            self._update_progress(
                ScanPhase.CANDIDATE_ANALYSIS,
                "Candidates ready for review",
                ""
            )
            
            # Prepare candidate metadata
            candidates_data = [c.to_metadata_dict() for c in candidates]
            
            # Auto-confirm if requested (for testing)
            if auto_confirm:
                confirmed_count = 0
                for i, candidate in enumerate(candidates, 1):
                    self._update_progress(
                        ScanPhase.CONFIRMATION,
                        f"Auto-confirming: {candidate.detected_title}",
                        candidate.detected_title
                    )
                    confirmed_count += 1
                
                result = {
                    "success": True,
                    "message": f"Scan completed: {confirmed_count} candidates auto-confirmed",
                    "status": "completed",
                    "phase": "auto_confirmed",
                    "candidates": candidates_data
                }
                
                self._update_progress(
                    ScanPhase.COMPLETED,
                    "All candidates processed",
                    "",
                    candidates_found=confirmed_count
                )
                
                return result
            else:
                # Manual confirmation workflow - return candidates for library
                result = {
                    "success": True,
                    "message": f"Scan completed: {len(candidates)} candidates found",
                    "status": "pending_confirmation",
                    "phase": ScanPhase.CONFIRMATION.value,
                    "candidates": candidates_data
                }
                
                self._update_progress(
                    ScanPhase.COMPLETED,
                    "Candidates ready for confirmation",
                    "",
                    candidates_found=len(candidates)
                )
                
                return result
            
        except Exception as e:
            logger.error(f"Scan failed: {e}")
            result = {
                "success": False,
                "message": f"Scan failed: {str(e)}",
                "status": "error"
            }
            
            self._update_progress(
                ScanPhase.COMPLETED,
                f"Error: {str(e)}",
                ""
            )
            
            return result
            
        finally:
            self._is_scanning = False
    
    def is_scanning(self) -> bool:
        """Check if scan is currently running."""
        return self._is_scanning
    
    async def cancel(self):
        """Cancel current scan (placeholder for future implementation)."""
        if self._is_scanning:
            logger.warning("Scan cancellation requested (not yet implemented)")
            self._is_scanning = False


# Global scanner instance
_scanner: Optional[ScannerService] = None


def get_scanner() -> ScannerService:
    """Get or create global scanner instance."""
    global _scanner
    if _scanner is None:
        _scanner = ScannerService()
        logger.info("ScannerService initialized")
    return _scanner
