"""
Organizer API - Sprint 10.5
Exposes endpoints for "The Crown Engine" physical reorganization.
"""

from fastapi import APIRouter, HTTPException, Depends
from pydantic import BaseModel
from typing import List, Optional
import os

from ...core.database import get_db, Database
from ...organizer.engine import ReorganizationEngine

router = APIRouter(prefix="/organizer", tags=["organizer"])

class PreviewRequest(BaseModel):
    canonical_id: str
    mode: str = "A" # A (Virtual) or B (Museum)
    root_override: Optional[str] = None

class PreviewResponse(BaseModel):
    mode: str
    original_path: str
    new_path: str
    actions: List[str]
    warnings: List[str]

class ExecuteRequest(BaseModel):
    canonical_id: str
    mode: str
    root_override: Optional[str] = None

class ExecuteResponse(BaseModel):
    success: bool
    final_path: str
    log: List[str]

@router.post("/preview", response_model=PreviewResponse)
async def preview_reorg(req: PreviewRequest, db: Database = Depends(get_db)):
    """
    Dry-run the reorganization logic to show consequences.
    """
    engine = ReorganizationEngine(db)
    try:
        result = engine.dry_run(req.canonical_id, req.mode, req.root_override)
        return PreviewResponse(
            mode=result.mode,
            original_path=result.original_path,
            new_path=result.new_path,
            actions=result.actions,
            warnings=result.warnings
        )
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@router.post("/execute", response_model=ExecuteResponse)
async def execute_reorg(req: ExecuteRequest, db: Database = Depends(get_db)):
    """
    Execute the physical reorganization.
    """
    engine = ReorganizationEngine(db)
    try:
        # Re-run sanity check implicitly via engine logic
        result = engine.execute(req.canonical_id, req.mode, req.root_override)
        return ExecuteResponse(**result)
    except Exception as e:
        # In production, we'd want detailed error codes here
        raise HTTPException(status_code=500, detail=str(e))


# ============================================================================
# DIAGNOSTIC ENDPOINTS (Self-Audit Engine)
# ============================================================================

from ...organizer.diagnostic import AppDiagnosticProvider, generate_audit_report
from pathlib import Path

class DiagnosticResponse(BaseModel):
    total_tests: int
    passed: int
    failed: int
    results: List[dict]

@router.get("/diagnostic/run", response_model=DiagnosticResponse)
async def run_diagnostics(db: Database = Depends(get_db)):
    """
    Run all self-audit diagnostics.
    """
    provider = AppDiagnosticProvider(db)
    report = provider.run_all_checks()
    return DiagnosticResponse(
        total_tests=report.total_tests,
        passed=report.passed,
        failed=report.failed,
        results=report.results
    )

@router.post("/diagnostic/safety")
async def check_physical_safety(db: Database = Depends(get_db)):
    """
    Run physical safety checks only.
    """
    provider = AppDiagnosticProvider(db)
    results = provider.check_physical_safety()
    return {"results": [{"test_name": r.test_name, "passed": r.passed, "details": r.details} for r in results]}

@router.post("/diagnostic/report")
async def generate_report(output_dir: str = "C:/Users/Ben/Desktop/galroon/record", db: Database = Depends(get_db)):
    """
    Generate full audit report to file.
    """
    try:
        report_path = generate_audit_report(db, Path(output_dir))
        return {"success": True, "report_path": str(report_path)}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))
