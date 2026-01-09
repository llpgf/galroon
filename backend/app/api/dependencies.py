from fastapi import HTTPException, Request, status


def verify_not_read_only():
    """
    Dependency function to check if system is in read-only mode.

    Use this in endpoints that modify data:
        @router.post("/api/modify")
        async def modify_data(request, _ok: None = Depends(verify_not_read_only())):

    Raises:
        HTTPException: 503 if system is read-only
    """

    async def _check(request: Request):
        if getattr(request.app.state, 'is_read_only', False):
            raise HTTPException(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                detail=(
                    "System is in READ-ONLY mode due to recovery failure. "
                    "No write operations are allowed. "
                    "Please contact administrator to resolve journal corruption."
                )
            )
        return None

    return _check
