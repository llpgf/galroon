"""
Frozen Backend Entry Point

Phase 26.0: Portable Logging - Read environment variables early

This is the entry point used when the backend is frozen with PyInstaller.
It starts the FastAPI server with uvicorn.
"""

import sys
import os
import logging
from pathlib import Path

# Add the app directory to path (for frozen imports)
if getattr(sys, 'frozen', False):
    # Running as frozen executable
    app_dir = Path(sys.executable).parent
    sys.path.insert(0, str(app_dir))

# Phase 26.0: Debug environment variables BEFORE importing anything else
galroon_log_path = os.getenv('VNITE_LOG_PATH')
galroon_data_path = os.getenv('VNITE_DATA_PATH')

print("=" * 70)
print("PHASE 26.0: ENVIRONMENT CHECK")
print("=" * 70)
print(f"VNITE_LOG_PATH: {galroon_log_path}")
print(f"VNITE_DATA_PATH: {galroon_data_path}")
print("=" * 70)

# Configure logging for portable app
# Use UTF-8 encoding to avoid Windows CP950 errors
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler(sys.stdout)
    ]
)

logger = logging.getLogger(__name__)

def main():
    """Main entry point for frozen backend."""
    try:
        import uvicorn
        from app.main import app

        logger.info("=" * 70)
        logger.info("VNITE BACKEND STARTING (Frozen Mode)")
        logger.info("=" * 70)
        logger.info(f"Executable: {sys.executable}")
        logger.info(f"Working Directory: {Path.cwd()}")
        logger.info(f"VNITE_LOG_PATH: {os.getenv('VNITE_LOG_PATH')}")
        logger.info(f"VNITE_DATA_PATH: {os.getenv('VNITE_DATA_PATH')}")
        logger.info("=" * 70)

        # Start uvicorn server
        uvicorn.run(
            app,
            host="127.0.0.1",
            port=8000,
            log_level="info",
            access_log=True
        )

    except Exception as e:
        logger.error(f"Failed to start backend: {e}", exc_info=True)
        sys.exit(1)

if __name__ == "__main__":
    main()
