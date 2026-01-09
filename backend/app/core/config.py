"""
Unified Configuration - Sprint 10.5 Final
Centralizes all environment-based configuration using Pydantic BaseSettings.

Features:
- Environment variable override with safe defaults
- .env file support
- Type validation
"""

import os
from typing import Optional
from pydantic import Field, ConfigDict
from pydantic_settings import BaseSettings


class Settings(BaseSettings):
    """
    Application settings with environment variable support.
    
    Usage:
        from app.core.config import settings
        print(settings.GDRIVE_REDIRECT_URI)
    """
    
    # =====================================
    # OAuth / Authentication
    # =====================================
    GDRIVE_REDIRECT_URI: str = Field(
        default="http://localhost:8000/api/v1/auth/gdrive/callback",
        description="Google Drive OAuth callback URL"
    )
    
    GDRIVE_CLIENT_ID: Optional[str] = Field(
        default=None,
        description="Google Cloud OAuth Client ID"
    )
    
    GDRIVE_CLIENT_SECRET: Optional[str] = Field(
        default=None,
        description="Google Cloud OAuth Client Secret"
    )
    
    # =====================================
    # Frontend URLs
    # =====================================
    FRONTEND_BASE_URL: str = Field(
        default="http://localhost:5173",
        description="Frontend development server URL"
    )
    
    FRONTEND_SETTINGS_URL: str = Field(
        default="http://localhost:5173/settings",
        description="Settings page callback after OAuth"
    )
    
    # =====================================
    # API Configuration
    # =====================================
    API_HOST: str = Field(default="127.0.0.1")
    API_PORT: int = Field(default=8000)
    API_DEBUG: bool = Field(default=True)
    
    # =====================================
    # Environment & Security
    # =====================================
    GALGAME_ENV: str = Field(
        default="production",
        description="Environment mode (production/sandbox)"
    )

    # =====================================
    # Path Configuration (shared with Config)
    # =====================================
    VNITE_DATA_PATH: Optional[str] = Field(
        default=None,
        description="Portable mode data path (set by launcher)"
    )

    VNITE_LOG_PATH: Optional[str] = Field(
        default=None,
        description="Portable mode log path (set by launcher)"
    )

    GALGAME_LIBRARY_ROOTS: Optional[str] = Field(
        default=None,
        description="JSON list of library root paths"
    )

    GALGAME_LIBRARY_ROOT: Optional[str] = Field(
        default=None,
        description="Single library root path (legacy)"
    )

    GALGAME_CONFIG_DIR: Optional[str] = Field(
        default=None,
        description="Configuration directory override"
    )

    GALGAME_RATE_LIMITS: Optional[str] = Field(
        default=None,
        description="Comma-separated rate limits for slowapi (e.g. '100/minute,1000/hour')"
    )

    ALLOW_EXTERNAL_PATHS: bool = Field(
        default=False,
        description="Allow paths outside library root for system/utilities endpoints"
    )
    
    SESSION_TOKEN: Optional[str] = Field(
        default=None,
        description="Security token for API access"
    )
    
    # =====================================
    # Database
    # =====================================
    DATABASE_PATH: str = Field(
        default="data/library.db",
        description="Path to SQLite database"
    )
    
    BACKUP_DIR: str = Field(
        default="data/backups",
        description="Local backup directory"
    )
    
    # =====================================
    # Diagnostic / Self-Audit
    # =====================================
    DIAGNOSTIC_OUTPUT_DIR: str = Field(
        default="record",
        description="Default output directory for diagnostic reports"
    )
    
    SAFETY_SCORE_THRESHOLD: int = Field(
        default=100,
        description="Minimum safety score required for Mode B"
    )
    
    # =====================================
    # VNDB / External APIs
    # =====================================
    VNDB_API_URL: str = Field(
        default="https://api.vndb.org/kana",
        description="VNDB API base URL"
    )
    
    model_config = ConfigDict(
        env_file='.env',
        env_file_encoding='utf-8',
        case_sensitive=True
    )


# Global settings instance
settings = Settings()
