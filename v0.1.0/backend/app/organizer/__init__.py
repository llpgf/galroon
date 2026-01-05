"""
Organization Module for Galgame Library Manager.

**PHASE 9.5: The Curator Workbench**

Provides safe, interactive file organization with:
- Scene Standard folder structure
- Read-only proposal generation
- Undo-supported execution

Usage:
    from app.organizer import generate_proposal, execute_plan

    # Generate proposal
    proposal = generate_proposal(
        source_path=Path("H:/Messy/Fate"),
        target_root=Path("D:/Games"),
        vndb_metadata={...}
    )

    # Review proposal.moves
    # User edits as needed

    # Execute
    result = execute_plan(proposal)
"""

from .standards import (
    StandardDir,
    DirRule,
    categorize_file,
    generate_standard_path,
    sanitize_path_component,
    OrganizationStandard
)

from .proposal import (
    MoveStatus,
    FileMove,
    ArchiveGroup,
    OrganizationProposal,
    generate_proposal,
    save_proposal,
    load_proposal
)

from .executor import (
    ExecutionResult,
    UndoRecord,
    execute_plan,
    rollback,
    pre_flight_check
)

__all__ = [
    # Standards
    "StandardDir",
    "DirRule",
    "categorize_file",
    "generate_standard_path",
    "sanitize_path_component",
    "OrganizationStandard",

    # Proposal
    "MoveStatus",
    "FileMove",
    "ArchiveGroup",
    "OrganizationProposal",
    "generate_proposal",
    "save_proposal",
    "load_proposal",

    # Executor
    "ExecutionResult",
    "UndoRecord",
    "execute_plan",
    "rollback",
    "pre_flight_check",
]
