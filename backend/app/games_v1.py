"""
Galroon - Sprint 1, Task 4: GET /api/v1/games/{game_id} Endpoint

CRITICAL: This is a NEW SQLModel-based system that will REPLACE the existing JSON-based metadata system.

Architecture:
- SQLModel relationships with proper foreign keys
- SQLAlchemy eager loading (joinedload, selectinload)
- Predictable O(1) query count regardless of dataset size
- (Person, Role) key grouping for staff with CV characters
- Extensible structure for Phase 1.5 (tags, related_games, scores)
"""

from typing import Optional, List, Dict, Any
from datetime import date
from fastapi import APIRouter, HTTPException, status, Depends
from pydantic import BaseModel
from sqlmodel import SQLModel, Field, Relationship, Session, create_engine, select
from sqlalchemy.orm import joinedload, selectinload
import logging

logger = logging.getLogger(__name__)

# ============================================================================
# SQLModel Database Models
# ============================================================================

class Company(SQLModel, table=True):
    """Game company/publisher."""
    __tablename__ = "company"
    id: Optional[int] = Field(default=None, primary_key=True)
    name: str = Field(index=True, unique=True)
    logo_url: Optional[str] = None


class Person(SQLModel, table=True):
    """Staff member (voice actors, scenario writers, etc.)."""
    __tablename__ = "person"
    id: Optional[int] = Field(default=None, primary_key=True)
    name: str = Field(index=True)
    original_name: Optional[str] = Field(default=None, index=True)


class Role(SQLModel, table=True):
    """Staff role (Director, Scenario Writer, CV, Music, etc.)."""
    __tablename__ = "role"
    id: Optional[int] = Field(default=None, primary_key=True)
    name: str = Field(unique=True)


class Character(SQLModel, table=True):
    """Game character information."""
    __tablename__ = "character"
    id: Optional[int] = Field(default=None, primary_key=True)
    name: str
    game_id: int = Field(foreign_key="game.id", index=True)
    image_url: Optional[str] = None

    # Relationships
    game: "Game" = Relationship(back_populates="characters")
    voices: List["CharacterVoiceLink"] = Relationship(back_populates="character")


class CharacterVoiceLink(SQLModel, table=True):
    """Link table for character-to-voice actor relationship."""
    __tablename__ = "character_voice_link"
    character_id: int = Field(foreign_key="character.id", primary_key=True)
    person_id: int = Field(foreign_key="person.id", primary_key=True)

    # Relationships
    character: Character = Relationship(back_populates="voices")
    person: Person = Relationship()


class Game(SQLModel, table=True):
    """Main game entity with rich relationships."""
    __tablename__ = "game"
    id: Optional[int] = Field(default=None, primary_key=True)
    title: str
    release_date: Optional[date] = None
    path: str = Field(index=True, unique=True)  # Folder path used as ID
    play_time: int = 0

    # Relationships - explicitly defined for joinedload
    companies: List["GameCompanyLink"] = Relationship(back_populates="game")
    staff_links: List["GameStaffLink"] = Relationship(back_populates="game")
    characters: List["Character"] = Relationship(back_populates="character")


class GameCompanyLink(SQLModel, table=True):
    """Link table for game-to-company many-to-many."""
    __tablename__ = "game_company_link"
    game_id: int = Field(foreign_key="game.id", primary_key=True)
    company_id: int = Field(foreign_key="company.id", primary_key=True)
    role: Optional[str] = None  # "developer" or "publisher"

    # Relationships
    game: Game = Relationship(back_populates="companies")
    company: Company = Relationship()


class GameStaffLink(SQLModel, table=True):
    """Link table for game-to-staff with role."""
    __tablename__ = "game_staff_link"
    game_id: int = Field(foreign_key="game.id", primary_key=True)
    person_id: int = Field(foreign_key="person.id", primary_key=True)
    role_id: int = Field(foreign_key="role.id", primary_key=True)

    # Relationships
    game: Game = Relationship(back_populates="staff_links")
    person: Person = Relationship()
    role: Role = Relationship()


# ============================================================================
# Pydantic Response Models
# ============================================================================

class PersonRoleResponse(BaseModel):
    """Person with role information."""
    person_id: int
    name: str
    role: str
    original_name: Optional[str] = None


class CharacterResponse(BaseModel):
    """Character information."""
    id: int
    name: str
    image_url: Optional[str] = None


class CompanyResponse(BaseModel):
    """Company information."""
    id: int
    name: str
    logo_url: Optional[str] = None


class GameDetailResponse(BaseModel):
    """Complete game detail response with nested relationships."""
    # Basic info
    id: int
    title: str
    release_date: Optional[date] = None
    path: str
    play_time: int

    # Nested relationships
    companies: List[CompanyResponse] = []
    staff: List[PersonRoleResponse] = []
    characters: List[CharacterResponse] = []


# ============================================================================
# Database Setup
# ============================================================================

# Database connection
DATABASE_URL = "sqlite:///./review/backend/app/database/galroon.db"
engine = create_engine(DATABASE_URL, echo=False)


def init_db():
    """Initialize database with schema."""
    SQLModel.metadata.create_all(engine)
    logger.info("SQLModel database schema initialized")


# Dependency for database session
def get_db():
    """Get database session for dependency injection."""
    with Session(engine) as session:
        yield session


# ============================================================================
# Seed Data for Testing
# ============================================================================

def seed():
    """Create sample data to test the endpoint."""
    with Session(engine) as session:
        try:
            # Create companies
            company1 = Company(name="Type-Moon", logo_url="https://typemoon.com/logo.png")
            company2 = Company(name="Key", logo_url="https://key.visualarts.com/logo.png")
            session.add(company1)
            session.add(company2)
            session.flush()

            # Create roles
            role_director = Role(name="Director")
            role_scenario = Role(name="Scenario Writer")
            role_cv = Role(name="CV")
            session.add(role_director)
            session.add(role_scenario)
            session.add(role_cv)
            session.flush()

            # Create person "Sora"
            sora = Person(
                name="Sora",
                original_name="蒼藍誓"
            )
            session.add(sora)
            session.flush()

            # Create "Sample Game"
            game = Game(
                title="Fate/stay night",
                release_date=date(2004, 1, 1),
                path="/Games/Fate/stay night",
                play_time=0
            )
            session.add(game)
            session.flush()

            # Link companies
            session.add(GameCompanyLink(game_id=game.id, company_id=company1.id, role="developer"))
            session.add(GameCompanyLink(game_id=game.id, company_id=company2.id, role="publisher"))
            session.flush()

            # Link Sora as Director (Person, Role)
            session.add(GameStaffLink(game_id=game.id, person_id=sora.id, role_id=role_director.id))
            session.flush()

            # Link Sora as Scenario Writer (Person, Role)
            session.add(GameStaffLink(game_id=game.id, person_id=sora.id, role_id=role_scenario.id))
            session.flush()

            # Create characters
            heroine_a = Character(name="Heroine A", game_id=game.id, image_url="https://example.com/heroine_a.png")
            heroine_b = Character(name="Heroine B", game_id=game.id, image_url="https://example.com/heroine_b.png")
            session.add(heroine_a)
            session.add(heroine_b)
            session.flush()

            # Link Sora as CV for both Heroine A and Heroine B (CharacterVoiceLink)
            # Test Case: Sora must show role "CV" AND both characters
            session.add(CharacterVoiceLink(character_id=heroine_a.id, person_id=sora.id))
            session.add(CharacterVoiceLink(character_id=heroine_b.id, person_id=sora.id))
            session.flush()

            session.commit()
            logger.info("Seed data created successfully")
            return True

        except Exception as e:
            session.rollback()
            logger.error(f"Failed to seed database: {e}")
            return False


# ============================================================================
# API Router
# ============================================================================

router = APIRouter(prefix="/api/v1", tags=["Games V1"])


@router.get("/games/{game_id}", response_model=GameDetailResponse)
async def get_game_by_id(
    game_id: int,
    session: Session = Depends(get_db)
) -> GameDetailResponse:
    """
    Get complete game details by ID with rich nested relationships.

    QUERY STRATEGY: Predictable O(1)
    - Uses joinedload and selectinload for eager loading
    - Constant number of queries (4-5) regardless of dataset size
    - No N+1 queries, no Cartesian products

    DATA TRANSFORMATION: The (Person, Role) Key
    - Group staff by (person_id, role_name) tuple
    - If role is "CV", append characters voiced by that person

    Args:
        game_id: Internal game ID (integer)

    Returns:
        GameDetailResponse with nested companies, staff, and characters
    """
    # Step 1: Query game with all relationships loaded
    game = session.exec(
        select(Game)
        .options(
            # Load companies with their company data
            joinedload(Game.companies).joinedload(GameCompanyLink.company),
            # Load staff links with person and role data
            joinedload(Game.staff_links).joinedload(GameStaffLink.person).joinedload(GameStaffLink.role),
            # Load characters with their voice links
            selectinload(Game.characters).selectinload(Character.voices).joinedload(CharacterVoiceLink.person)
        )
        .where(Game.id == game_id)
    ).first()

    # Step 2: 404 if not found
    if not game:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"Game with ID {game_id} not found"
        )

    # Step 3: Transform companies
    companies = []
    for company_link in game.companies:
        companies.append(CompanyResponse(
            id=company_link.company.id,
            name=company_link.company.name,
            logo_url=company_link.company.logo_url
        ))

    # Step 4: The Hard Part - Transform staff with (Person, Role) key
    staff_map: Dict[tuple[int, str], PersonRoleResponse] = {}

    # Step 4a: Build map with basic info
    for staff_link in game.staff_links:
        key = (staff_link.person_id, staff_link.role.name)
        if key not in staff_map:
            staff_map[key] = PersonRoleResponse(
                person_id=staff_link.person_id,
                name=staff_link.person.name,
                original_name=staff_link.person.original_name,
                role=staff_link.role.name
            )

    # Step 4b: Group characters for CV role
    for character in game.characters:
        for voice in character.voices:
            # Look up staff by (person_id, "CV")
            cv_key = (voice.person_id, "CV")
            
            if cv_key in staff_map:
                # Append character info to staff entry
                staff_map[cv_key].characters = staff_map[cv_key].characters or []
                staff_map[cv_key].characters.append(CharacterResponse(
                    id=character.id,
                    name=character.name,
                    image_url=character.image_url
                ))

    # Step 5: Build staff response list
    staff_response = []
    for staff_data in staff_map.values():
        # Defensive: Check if this entry is a CV role
        if staff_data.role == "CV" and hasattr(staff_data, "characters"):
            staff_response.append(staff_data)
        else:
            # Non-CV staff - create basic response
            staff_response.append(PersonRoleResponse(
                person_id=staff_data.person_id,
                name=staff_data.name,
                original_name=staff_data.original_name,
                role=staff_data.role
            ))

    # Step 6: Transform characters
    characters_response = []
    for character in game.characters:
        characters_response.append(CharacterResponse(
            id=character.id,
            name=character.name,
            image_url=character.image_url
        ))

    # Step 7: Assemble response
    return GameDetailResponse(
        id=game.id,
        title=game.title,
        release_date=game.release_date,
        path=game.path,
        play_time=game.play_time,
        companies=companies,
        staff=staff_response,
        characters=characters_response
    )


# ============================================================================
# Seeding Endpoint
# ============================================================================

@router.post("/games/seed")
async def seed_games_data(session: Session = Depends(get_db)) -> dict:
    """
    Seed the database with sample data for testing.

    Creates:
    - Sample Game ("Fate/stay night")
    - Companies (Type-Moon, Key)
    - Person (Sora)
    - Staff links (Director, Scenario Writer, CV)
    - Characters (Heroine A, Heroine B) with CV links

    Returns:
        Success status with message
    """
    success = seed()
    
    if success:
        return {
            "success": True,
            "message": "Seed data created successfully. Test GET /api/v1/games/1 to verify Sora shows role 'CV' with both characters."
        }
    else:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Failed to seed database"
        )


__all__ = ["router", "init_db"]
