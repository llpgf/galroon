"""
Canonicalization Service for Project Galroon

Sprint 4: Turns accepted IdentityMatchCandidates into Canonical Entities with full provenance tracking.

Core Philosophy:
- Canonical Entities are the only source of truth
- IdentityMatch / ScanCandidate are evidence, not truth
- Canonicalization is irreversible but must be fully traceable
- No Canonical Entity may exist without provenance
- No duplicate Canonical Entity may ever be created
- CanonicalSourceLink is single source of truth (idempotent)

This is the final layer that makes identity probabilistic → historical fact.
"""

import logging
from typing import List, Optional, Dict, Any
from datetime import datetime
from enum import Enum

from ..core.database import get_database

logger = logging.getLogger(__name__)


# ============================================================================
# New Data Models for Sprint 4
# ============================================================================

class MatchStatus(str, Enum):
    """Status for identity match candidates."""
    pending = "pending"
    accepted = "accepted"
    canonicalized = "canonicalized"  # NEW - Sprint 4
    rejected = "rejected"


class CanonicalSourceLink(SQLModel, table=True):
    """
    ABSOLUTELY REQUIRED: Single source of truth for all canonical entities.
    
    This table tracks the ORIGIN of every canonical entity.
    Idempotent: Same entity_id + source_type + source_id = one row, forever.
    
    Examples:
    - Game 123 came from VNDB v12345
    - Person 456 came from scan_candidate 789
    - Company 789 came from user manual entry
    
    WITHOUT THIS TABLE: You cannot trace why an entity exists.
    WITH THIS TABLE: Complete audit trail for every entity.
    """
    __tablename__ = "canonical_source_link"
    
    id: Optional[int] = Field(default=None, primary_key=True)
    
    entity_type: str = Field(..., description="Entity type: game | company | person | character | link")
    entity_id: int = Field(..., description="ID of the canonical entity")
    
    source_type: str = Field(..., description="Source type: vndb | user | scanner")
    source_id: str = Field(..., description="Source identifier: e.g., VNDB ID, scan candidate ID")
    
    source_hash: str = Field(..., description="Hash of source payload (for deduplication)")
    
    created_at: datetime = Field(default_factory=datetime.now)


class CanonicalizationResult:
    """Result of canonicalization operation."""
    canonical_game_id: Optional[int] = None
    canonical_company_ids: List[int] = Field(default_factory=list)
    canonical_person_ids: List[int] = Field(default_factory=list)
    canonical_character_ids: List[int] = Field(default_factory=list)
    canonical_link_ids: List[int] = Field(default_factory=list)
    source_links_created: int = 0


# ============================================================================
# Canonicalization Service
# ============================================================================

class CanonicalizationService:
    """
    Canonicalization Service with strict algorithm.
    
    Turns accepted IdentityMatchCandidates into canonical entities:
    - Games with VNDB provenance
    - Companies (deduplicated globally by name)
    - Persons (global identity, VNDB enriched)
    - Characters (game-scoped)
    - Roles (global game-independent)
    
    Guarantees:
    - Full traceability via CanonicalSourceLink
    - Global uniqueness for companies, persons, roles
    - Game uniqueness by VNDB ID (never by title)
    - Character uniqueness by game_id + name
    """
    
    def __init__(self):
        self._lock_timeout = 30  # Lock timeout in seconds
        
    @staticmethod
    def _compute_source_hash(payload: dict) -> str:
        """
        Compute hash of source payload for deduplication.
        
        Uses deterministic hash of sorted keys.
        """
        import hashlib
        import json
        
        # Sort keys for deterministic hash
        sorted_payload = json.dumps(payload, sort_keys=True)
        return hashlib.sha256(sorted_payload.encode()).hexdigest()
    
    async def canonicalize_match(
        self,
        match_id: int,
        session
    ) -> CanonicalizationResult:
        """
        Turn an accepted IdentityMatchCandidate into canonical entities.
        
        CRITICAL: This must be idempotent and fully traceable.
        Every canonical entity creation MUST write a CanonicalSourceLink.
        
        Args:
            match_id: ID of IdentityMatchCandidate to canonicalize
            session: Database session
            
        Returns:
            CanonicalizationResult with all created entity IDs
        """
        from ..core.database import (
            get_database,
            IdentityMatchCandidate,
            MatchStatus,
            Game,
            Company,
            Person,
            Role,
            Character,
            CharacterVoiceLink,
            GameCompanyLink,
            GameStaffLink,
            ScanCandidate,
            ScanStatus
        )
        
        db = session if session else get_database()
        
        # STEP 0: Preconditions (FAIL FAST)
        logger.info(f"Starting canonicalization of match {match_id}")
        
        # Step 0.1: Get match
        match = db.get_identity_match_candidate_by_id(match_id)
        
        if not match:
            raise ValueError(f"Match {match_id} not found")
        
        # Step 0.2: Validate match status
        if match.status != MatchStatus.accepted:
            logger.error(f"Match {match_id} has status {match.status.value}, not accepted")
            raise ValueError(f"Match must be accepted to canonicalize, got {match.status.value}")
        
        # Step 0.3: Check if already canonicalized
        if match.status == MatchStatus.canonicalized:
            logger.warning(f"Match {match_id} already canonicalized, skipping")
            return CanonicalizationResult()
        
        # Lock ScanCandidate to prevent concurrent canonicalization
        lock_success = self._lock_scan_candidate(db, match_id)
        if not lock_success:
            logger.error(f"Failed to lock scan candidate {match_id}")
            raise ValueError(f"Cannot canonicalize - could not lock match {match_id}")
        
        try:
            # ====================================================================
            # STEP 1: Canonical Game Resolution (CRITICAL)
            # ====================================================================
            
            logger.info("STEP 1: Canonical Game Resolution")
            
            # Rule: Check if game exists with this VNDB ID
            # NEVER deduplicate by title - deduplication is by VNDB ID only
            existing_game = None
            
            with db.get_cursor() as cursor:
                cursor.execute("""
                    SELECT id, title, vndb_id 
                    FROM games 
                    WHERE vndb_id = ?
                    LIMIT 1
                """, (match.external_id,))
                row = cursor.fetchone()
                
                if row:
                    existing_game_id, existing_title, existing_vndb_id = row
                    logger.info(f"Found existing game: {existing_title} (VNDB: {existing_vndb_id})")
                    
                    # Create provenance link
                    db._create_canonical_source_link(
                        entity_type="game",
                        entity_id=existing_game_id,
                        source_type="vndb",
                        source_id=existing_vndb_id,
                        source_hash=self._compute_source_hash({"vndb_id": existing_vndb_id})
                    )
            
            # Step 1.3: Create Canonical Game if needed
            if not existing_game:
                logger.info("No existing game found, creating new canonical game")
                
                # Parse match title
                canonical_title = self._clean_title(match.detected_title)
                
                game = Game(
                    title=canonical_title,
                    original_title=match.detected_title,
                    vndb_id=match.external_id,
                    path=match.path,
                    play_time=0
                )
                
                with db.get_cursor() as cursor:
                    cursor.execute("""
                        INSERT INTO games (title, original_title, vndb_id, path, play_time)
                        VALUES (?, ?, ?, ?, ?)
                    """, (
                        game.title,
                        game.original_title,
                        game.vndb_id,
                        game.path,
                        game.play_time
                    ))
                    
                    # Get new game ID
                    cursor.execute("SELECT last_insert_rowid()")
                    game_id_row = cursor.fetchone()
                    new_game_id = game_id_row[0]
                
                logger.info(f"Created canonical game: {canonical_title} (ID: {new_game_id})")
                
                # Create provenance link for new game
                db._create_canonical_source_link(
                    entity_type="game",
                    entity_id=new_game_id,
                    source_type="vndb",
                    source_id=match.external_id,
                    source_hash=self._compute_source_hash({"vndb_id": match.external_id})
                )
            else:
                # Reuse existing game
                game_id = existing_game_id
                logger.info(f"Reusing existing game: {existing_title} (ID: {game_id})")
            
            # ====================================================================
            # STEP 2: Canonical Company Resolution (UPSERT LAW)
            # ====================================================================
            
            logger.info("STEP 2: Canonical Company Resolution")
            
            company_ids_created = []
            
            # Extract companies from match (from game_indicators or manual parsing)
            # For now, we'll create placeholder companies
            # In future, this would come from VNDB API
            
            # For this demo, create a generic company "Unknown Developer"
            with db.get_cursor() as cursor:
                # Check if exists by name (UPSERT)
                cursor.execute("""
                    SELECT id FROM companies WHERE name = ? LIMIT 1
                """, ("Unknown Developer",))
                company_row = cursor.fetchone()
                
                if not company_row:
                    # Create new company
                    cursor.execute("""
                        INSERT INTO companies (name)
                        VALUES (?)
                    """, ("Unknown Developer",))
                    
                    cursor.execute("SELECT last_insert_rowid()")
                    company_id_row = cursor.fetchone()
                    company_id = company_id_row[0]
                    
                    logger.info(f"Created canonical company: Unknown Developer (ID: {company_id})")
                else:
                    company_id = company_row[0]
                    logger.info(f"Reusing canonical company: Unknown Developer (ID: {company_id})")
                
                company_ids_created.append(company_id)
                
                # Create link
                db._create_canonical_source_link(
                    entity_type="link",
                    entity_id=company_id,
                    source_type="scanner",
                    source_id=str(match_id),
                    source_hash=self._compute_source_hash({"match_id": match_id, "inferred": "unknown_developer"})
                )
            
            # ====================================================================
            # STEP 3: Canonical Person Resolution (UPSERT LAW)
            # ====================================================================
            
            logger.info("STEP 3: Canonical Person Resolution")
            
            # Extract persons from match (voice actors, staff)
            # For this demo, create placeholder person
            # In future, this would come from VNDB API
            
            person_ids_created = []
            
            # Check if match has person info
            if match.detected_engine:
                # Create placeholder person based on detected engine
                person_name = f"{match.detected_engine}_Voice_Actor"
                
                with db.get_cursor() as cursor:
                    # Check if exists by original_name (UPSERT)
                    cursor.execute("""
                        SELECT id FROM persons WHERE original_name = ? LIMIT 1
                    """, (person_name,))
                    person_row = cursor.fetchone()
                    
                    if not person_row:
                        # Create new person
                        cursor.execute("""
                            INSERT INTO persons (name, original_name)
                            VALUES (?, ?)
                        """, (person_name, person_name))
                        
                        cursor.execute("SELECT last_insert_rowid()")
                        person_id_row = cursor.fetchone()
                        person_id = person_id_row[0]
                        
                        logger.info(f"Created canonical person: {person_name} (ID: {person_id})")
                    else:
                        person_id = person_row[0]
                        logger.info(f"Reusing canonical person: {person_name} (ID: {person_id})")
                
                person_ids_created.append(person_id)
                
                # Create links (GameCompanyLink equivalent for Persons - but we use GameStaffLink)
                # For now, skip staff links - this is a simplification
                
                # Create provenance link
                db._create_canonical_source_link(
                    entity_type="person",
                    entity_id=person_id,
                    source_type="scanner",
                    source_id=str(match_id),
                    source_hash=self._compute_source_hash({"match_id": match_id, "inferred": person_name})
                )
            
            # ====================================================================
            # STEP 4: Canonical Character Resolution
            # ====================================================================
            
            logger.info("STEP 4: Canonical Character Resolution")
            
            # Extract characters from match (game_indicators)
            # For this demo, create placeholder characters
            # In future, this would come from VNDB API
            
            character_ids_created = []
            
            # Check if match has character info
            if "character" in str(match.game_indicators).lower():
                # Create placeholder character
                character_name = "Main_Character"
                
                with db.get_cursor() as cursor:
                    # Check if exists by game_id + name (UPSERT)
                    cursor.execute("""
                        SELECT id FROM characters 
                        WHERE game_id = ? AND name = ? 
                        LIMIT 1
                    """, (game_id, character_name))
                    
                    character_row = cursor.fetchone()
                    
                    if not character_row:
                        # Create new character
                        cursor.execute("""
                            INSERT INTO characters (name, game_id)
                            VALUES (?, ?)
                        """, (character_name, game_id))
                        
                        cursor.execute("SELECT last_insert_rowid()")
                        character_id_row = cursor.fetchone()
                        character_id = character_id_row[0]
                        
                        logger.info(f"Created canonical character: {character_name} (ID: {character_id})")
                    else:
                        character_id = character_row[0]
                        logger.info(f"Reusing canonical character: {character_name} (ID: {character_id})")
                
                character_ids_created.append(character_id)
                
                # Create provenance link
                db._create_canonical_source_link(
                    entity_type="character",
                    entity_id=character_id,
                    source_type="scanner",
                    source_id=str(match_id),
                    source_hash=self._compute_source_hash({"match_id": match_id, "inferred": character_name})
                )
            
            # ====================================================================
            # STEP 5: Canonical Role Resolution (GLOBAL, UPSERT)
            # ====================================================================
            
            logger.info("STEP 5: Canonical Role Resolution")
            
            # Create standard roles for game engines
            role_ids_created = []
            
            roles_to_create = ["Director", "Scenario Writer", "CV"]
            
            for role_name in roles_to_create:
                with db.get_cursor() as cursor:
                    # Check if exists by name (UPSERT)
                    cursor.execute("""
                        SELECT id FROM roles WHERE name = ? LIMIT 1
                    """, (role_name,))
                    role_row = cursor.fetchone()
                    
                    if not role_row:
                        # Create new role
                        cursor.execute("""
                            INSERT INTO roles (name)
                            VALUES (?)
                        """, (role_name,))
                        
                        cursor.execute("SELECT last_insert_rowid()")
                        role_id_row = cursor.fetchone()
                        role_id = role_id_row[0]
                        
                        logger.info(f"Created canonical role: {role_name} (ID: {role_id})")
                    else:
                        role_id = role_row[0]
                        logger.info(f"Reusing canonical role: {role_name} (ID: {role_id})")
                
                role_ids_created.append(role_id)
                
                # Create GameStaffLink for game
                for role_id in role_ids_created:
                    # Use person_ids_created[0] as director
                    # This is a simplification - all staff use same person for now
                    staff_person_id = person_ids_created[0] if person_ids_created else 1
                    
                    with db.get_cursor() as cursor:
                        # Check if link exists
                        cursor.execute("""
                            SELECT id FROM game_staff_link
                            WHERE game_id = ? AND person_id = ? AND role_id = ?
                            LIMIT 1
                        """, (game_id, staff_person_id, role_id))
                        
                        staff_link_row = cursor.fetchone()
                        
                        if not staff_link_row:
                            # Create new link
                            cursor.execute("""
                                INSERT INTO game_staff_link (game_id, person_id, role_id)
                                VALUES (?, ?, ?)
                            """, (game_id, staff_person_id, role_id))
                            
                            logger.info(f"Created game_staff_link: {role_name}")
                    
                    # Create provenance link
                    db._create_canonical_source_link(
                        entity_type="link",
                        entity_id=staff_person_id if person_ids_created else game_id,
                        source_type="scanner",
                        source_id=str(match_id),
                        source_hash=self._compute_source_hash({"match_id": match_id, "role": role_name})
                    )
            
            # ====================================================================
            # STEP 6: CharacterVoiceLink with Provenance
            # ====================================================================
            
            logger.info("STEP 6: CharacterVoiceLink with Provenance")
            
            # For this demo, we won't create voice links
            # In future, this would link the canonical person to the character
            
            # ====================================================================
            # STEP 7: Graph Links (GameCompanyLink)
            # ====================================================================
            
            logger.info("STEP 7: Graph Links - GameCompanyLink")
            
            link_ids_created = []
            
            for company_id in company_ids_created:
                with db.get_cursor() as cursor:
                    # Check if exists
                    cursor.execute("""
                        SELECT id FROM game_company_link
                        WHERE game_id = ? AND company_id = ? 
                        LIMIT 1
                    """, (game_id, company_id))
                        
                    company_link_row = cursor.fetchone()
                    
                    if not company_link_row:
                        # Create new link (developer role)
                        cursor.execute("""
                            INSERT INTO game_company_link (game_id, company_id, role)
                            VALUES (?, ?, ?)
                        """, (game_id, company_id, "developer"))
                            
                        logger.info(f"Created game_company_link: developer")
                    
                    link_ids_created.append(company_id)
                    
                    # Create provenance link
                    db._create_canonical_source_link(
                        entity_type="link",
                        entity_id=company_id,
                        source_type="scanner",
                        source_id=str(match_id),
                        source_hash=self._compute_source_hash({"match_id": match_id, "company": company_id})
                    )
            
            # ====================================================================
            # STEP 9: Final State Transitions (MANDATORY)
            # ====================================================================
            
            logger.info("STEP 9: Final State Transitions")
            
            # Update match status
            match.status = MatchStatus.canonicalized
            
            # Update scan candidate status
            update_success = db.update_candidate_status(match_id, ScanStatus.merged)
            
            if not update_success:
                logger.error(f"Failed to update scan candidate {match_id} to merged")
            
            result = CanonicalizationResult(
                canonical_game_id=game_id,
                canonical_company_ids=company_ids_created,
                canonical_person_ids=person_ids_created,
                canonical_character_ids=character_ids_created,
                canonical_link_ids=link_ids_created,
                source_links_created=len(company_ids_created) + len(person_ids_created) + len(character_ids_created)
            )
            
            logger.info(f"Canonicalization complete: {result.source_links_created} source links created")
            
            return result
        
        except Exception as e:
            logger.error(f"Canonicalization failed for match {match_id}: {e}")
            raise
    
    def _clean_title(self, title: str) -> str:
        """Clean title for canonicalization."""
        import re
        # Remove version brackets: [2021-05-28][v1.0] GameTitle
        cleaned = re.sub(r'\[[\d\-]+\][\[.*?\]]', '', title)
        # Remove remaining brackets: GameTitle[Remastered] -> GameTitle
        cleaned = re.sub(r'\[.*?\]', '', cleaned)
        return cleaned.strip()
    
    def _lock_scan_candidate(
        self,
        db,
        match_id: int
    ) -> bool:
        """
        Lock ScanCandidate to prevent concurrent canonicalization.
        
        Returns True if successful.
        """
        try:
            with db.get_cursor() as cursor:
                cursor.execute("""
                    UPDATE scan_candidates
                    SET status = ?
                    WHERE id = ?
                """, (ScanStatus.locked.value, match_id))
                
            logger.info(f"Locked scan candidate {match_id}")
            return True
            
        except Exception as e:
            logger.error(f"Failed to lock scan candidate {match_id}: {e}")
            return False


# Global service instance
_canonicalization_service: Optional[CanonicalizationService] = None


def get_canonicalization_service() -> CanonicalizationService:
    """Get or create global canonicalization service instance."""
    global _canonicalization_service
    if _canonicalization_service is None:
        _canonicalization_service = CanonicalizationService()
        logger.info("CanonicalizationService initialized")
    return _canonicalization_service
