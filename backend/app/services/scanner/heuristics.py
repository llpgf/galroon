"""
Heuristic Engine for Galgame Detection

Returns ScanCandidates for confirmation workflow (not final Games).
"""

import re
import os
from pathlib import Path
from typing import Optional, List, Tuple
from datetime import datetime
import logging

logger = logging.getLogger(__name__)


class ScanCandidate:
    """
    Scan candidate pending library confirmation.
    
    This is NOT a Game yet - it's a detection result that needs
    confirmation before becoming a Game entity.
    """
    def __init__(
        self,
        path: str,
        detected_title: str,
        detected_engine: Optional[str] = None,
        confidence_score: float = 0.5,
        game_indicators: List[str] = None
    ):
        self.path = path
        self.detected_title = detected_title
        self.detected_engine = detected_engine
        self.confidence_score = confidence_score
        self.game_indicators = game_indicators or []
        
    def to_metadata_dict(self) -> dict:
        """Convert to dictionary for JSON response."""
        return {
            "path": self.path,
            "detected_title": self.detected_title,
            "detected_engine": self.detected_engine,
            "confidence_score": self.confidence_score,
            "game_indicators": self.game_indicators,
            "status": "pending",
            "detected_at": datetime.now().isoformat()
        }


class HeuristicEngine:
    """
    Detects Galgame folders using file signatures and pattern matching.
    
    Strategy:
    - Signature Detection: Look for known engine files (data.xp3, Scene.pck, etc.)
    - Folder Pattern Detection: Common Galgame folder structures
    - Title Cleaning: Remove version brackets and special characters
    - Confidence Scoring: Multi-factor scoring for reliability
    """
    
    # Engine signatures (file names and patterns that indicate specific engines)
    ENGINE_SIGNATURES = {
        "kirikiri": {
            "files": ["data.xp3", "*.xp3"],
            "folders": [],
            "description": "Kirikiri2 engine",
            "confidence_weight": 0.8
        },
        "siglus": {
            "files": ["SiglusEngine.exe", "Scene.pck"],
            "folders": [],
            "description": "Siglus engine",
            "confidence_weight": 0.9
        },
        "willplus": {
            "files": ["Rio.arc", "*.arc"],
            "folders": [],
            "description": "WillPlus engine",
            "confidence_weight": 0.75
        },
        "unity": {
            "files": ["UnityPlayer.dll", "Assembly-CSharp.dll"],
            "folders": [],
            "description": "Unity engine",
            "confidence_weight": 0.6
        },
        "renpy": {
            "files": ["renpy", "*.rpyc"],
            "folders": ["renpy", "game"],
            "description": "Ren'Py engine",
            "confidence_weight": 0.85
        }
    }
    
    # Pattern: Files that commonly exist in Galgame folders (raise confidence)
    GAME_INDICATORS = [
        "*.exe",
        "*.lnk",
        "unins000.exe",
        "*.url",
        "*.desktop"
    ]
    
    # Patterns to ignore (definitely NOT games)
    IGNORE_PATTERNS = [
        # Image-only folders
        re.compile(r'^[Pp]hotos?$', re.IGNORECASE),
        re.compile(r'^[Ii]mages?$', re.IGNORECASE),
        re.compile(r'^[Ss]creenshots?$', re.IGNORECASE),
        re.compile(r'^[Ww]allpapers?$', re.IGNORECASE),
        
        # System folders
        re.compile(r'^[\.\_]'),
        
        # Archive/backup folders
        re.compile(r'^(backup|old|archive|temp)', re.IGNORECASE),
    ]
    
    @staticmethod
    def clean_title(folder_name: str) -> str:
        """
        Clean folder name to extract game title.
        
        Removes:
        - Version brackets: [2021-05-28][v1.0] GameTitle
        - Special characters: [], {}, etc.
        - Redundant spaces
        
        Args:
            folder_name: Original folder name
            
        Returns:
            Cleaned game title
        """
        # Remove version brackets: [2021-05-28][v1.0] GameTitle
        # Pattern matches: [date][version] Title
        cleaned = re.sub(r'\[[\d\-]+\][\[.*?\]]', '', folder_name)
        
        # Remove remaining brackets: GameTitle[Remastered] -> GameTitle
        cleaned = re.sub(r'\[.*?\]', '', cleaned)
        
        # Remove common prefixes/suffixes
        cleaned = re.sub(r'^(~\$|~|\d+[\.\-_]*\s+)', '', cleaned, flags=re.IGNORECASE)
        
        # Normalize whitespace
        cleaned = re.sub(r'\s+', ' ', cleaned).strip()
        
        return cleaned if cleaned else folder_name
    
    @classmethod
    def detect_engine(cls, folder_path: Path) -> Tuple[Optional[str], float]:
        """
        Detect game engine with confidence score.
        
        Returns:
            Tuple of (engine_name, confidence_score)
        """
        if not folder_path.is_dir():
            return (None, 0.0)
        
        engine = None
        confidence = 0.5  # Default confidence
        
        try:
            for engine_name, signatures in cls.ENGINE_SIGNATURES.items():
                # Check for signature files
                for file_pattern in signatures["files"]:
                    if '*' in file_pattern:
                        # List files and check pattern
                        files = list(folder_path.glob(file_pattern))
                        if files:
                            return (engine_name, signatures["confidence_weight"])
                    else:
                        # Direct file check
                        if (folder_path / file_pattern).exists():
                            return (engine_name, signatures["confidence_weight"])
                
                # Check for signature folders
                if "folders" in signatures:
                    for folder_pattern in signatures["folders"]:
                        if (folder_path / folder_pattern).exists():
                            return (engine_name, signatures["confidence_weight"])
            
            return (engine, confidence)
            
        except Exception as e:
            logger.warning(f"Error detecting engine for {folder_path}: {e}")
            return (None, 0.0)
    
    @classmethod
    def has_game_indicators(cls, folder_path: Path) -> bool:
        """
        Check if folder has common Galgame indicator files.
        
        Args:
            folder_path: Path to game folder
            
        Returns:
            True if game indicators found
        """
        if not folder_path.is_dir():
            return False
        
        try:
            for pattern in cls.GAME_INDICATORS:
                files = list(folder_path.glob(pattern))
                if files:
                    return True
            return False
        except Exception as e:
            logger.warning(f"Error checking game indicators for {folder_path}: {e}")
            return False
    
    @classmethod
    def should_ignore(cls, folder_name: str) -> bool:
        """
        Check if folder should be ignored based on patterns.
        
        Args:
            folder_name: Name of folder
            
        Returns:
            True if should be ignored
        """
        for pattern in cls.IGNORE_PATTERNS:
            if pattern.search(folder_name):
                return True
        return False
    
    @classmethod
    def analyze_folder(cls, folder_path: Path) -> Optional[ScanCandidate]:
        """
        Analyze a folder to determine if it's a potential Galgame.
        
        Returns ScanCandidate (not Game) for confirmation workflow.
        
        Heuristics:
        1. Check ignore patterns (photos, system folders, etc.)
        2. Detect engine from file signatures
        3. Check for game indicator files (.exe, .lnk, etc.)
        4. Clean folder name to extract title
        5. Calculate multi-factor confidence score
        
        Args:
            folder_path: Path to folder to analyze
            
        Returns:
            ScanCandidate if detected as potential game, None otherwise
        """
        if not folder_path.exists() or not folder_path.is_dir():
            return None
        
        folder_name = folder_path.name
        
        # Step 1: Check ignore patterns
        if cls.should_ignore(folder_name):
            logger.debug(f"Ignoring folder: {folder_name}")
            return None
        
        # Step 2: Detect engine with confidence
        engine, engine_confidence = cls.detect_engine(folder_path)
        
        # Step 3: Check for game indicators
        has_indicators = cls.has_game_indicators(folder_path)
        indicators = []
        if has_indicators:
            indicators = ["has_executable", "has_game_files"]
        
        # Step 4: If no engine and no indicators, likely not a game
        if not engine and not has_indicators:
            logger.debug(f"Folder {folder_name} has no engine or indicators, skipping")
            return None
        
        # Step 5: Calculate multi-factor confidence
        # Base confidence from engine detection
        base_confidence = engine_confidence if engine else 0.3
        
        # Boost for game indicators
        if has_indicators:
            base_confidence = min(base_confidence + 0.2, 0.95)
        
        # Normalize title
        title = cls.clean_title(folder_name)
        
        logger.info(f"Detected candidate: {title} (engine: {engine}, confidence: {base_confidence:.2f})")
        
        return ScanCandidate(
            path=str(folder_path),
            detected_title=title,
            detected_engine=engine,
            confidence_score=base_confidence,
            game_indicators=indicators
        )
    
    @classmethod
    def analyze_directory(cls, root_path: Path) -> List[ScanCandidate]:
        """
        Analyze a directory and return all detected candidates.
        
        Args:
            root_path: Root directory to scan
            
        Returns:
            List of detected candidates (not Games)
        """
        candidates = []
        
        try:
            # List all subdirectories
            for entry in root_path.iterdir():
                if not entry.is_dir():
                    continue
                
                # Analyze each subdirectory
                candidate = cls.analyze_folder(entry)
                if candidate:
                    candidates.append(candidate)
        
        except PermissionError as e:
            logger.error(f"Permission denied scanning {root_path}: {e}")
        except Exception as e:
            logger.error(f"Error scanning {root_path}: {e}")
        
        return candidates
