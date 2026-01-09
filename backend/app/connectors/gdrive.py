"""
Google Drive Connector - Sprint 10
Handles OAuth2 authentication and file operations for Cloud Backup.
"""

import os
import logging
from pathlib import Path
from typing import Optional
from google.auth.transport.requests import Request
from google.oauth2.credentials import Credentials
from google_auth_oauthlib.flow import InstalledAppFlow
from googleapiclient.discovery import build
from googleapiclient.http import MediaFileUpload
from app.config import get_config

logger = logging.getLogger(__name__)

# If modifying these scopes, delete the file token.json.
SCOPES = ['https://www.googleapis.com/auth/drive.appdata']

class GoogleDriveService:
    def __init__(self):
        # Use app config_dir so portable/sandbox modes stay consistent.
        self.config_dir = Path(get_config().config_dir)
        self.credentials_path = self.config_dir / "client_secrets.json"
        self.token_path = self.config_dir / "token.json"
        self.creds: Optional[Credentials] = None
        self.service = None

    def _ensure_config_dir(self):
        if not self.config_dir.exists():
            self.config_dir.mkdir(parents=True, exist_ok=True)

    def is_authenticated(self) -> bool:
        """Check if we have valid credentials."""
        if self.token_path.exists():
            try:
                creds = Credentials.from_authorized_user_file(str(self.token_path), SCOPES)
                return creds and (creds.valid or creds.expired and creds.refresh_token)
            except Exception:
                return False
        return False

    def get_auth_url(self, redirect_uri: str) -> str:
        """Generate the authorization URL for the user."""
        if not self.credentials_path.exists():
            raise FileNotFoundError("client_secrets.json not found in config directory")

        flow = InstalledAppFlow.from_client_secrets_file(
            str(self.credentials_path), SCOPES, redirect_uri=redirect_uri)
        
        auth_url, _ = flow.authorization_url(prompt='consent')
        return auth_url

    def fetch_token(self, code: str, redirect_uri: str):
        """Exchange the authorization code for credentials and save them."""
        flow = InstalledAppFlow.from_client_secrets_file(
            str(self.credentials_path), SCOPES, redirect_uri=redirect_uri)
        
        flow.fetch_token(code=code)
        self.creds = flow.credentials
        
        # Save credentials
        self._ensure_config_dir()
        with open(self.token_path, 'w') as token:
            token.write(self.creds.to_json())
        
        self.service = build('drive', 'v3', credentials=self.creds)
        logger.info("Successfully authenticated with Google Drive")

    def get_service(self):
        """Get or create the Drive service instance."""
        if self.service:
            return self.service

        if self.token_path.exists():
            self.creds = Credentials.from_authorized_user_file(str(self.token_path), SCOPES)
        
        if not self.creds or not self.creds.valid:
            if self.creds and self.creds.expired and self.creds.refresh_token:
                try:
                    self.creds.refresh(Request())
                    # Save refreshed token
                    with open(self.token_path, 'w') as token:
                        token.write(self.creds.to_json())
                except Exception as e:
                    logger.error(f"Failed to refresh token: {e}")
                    raise Exception("Authentication expired. Please sign in again.")
            else:
                raise Exception("Not authenticated. Please sign in.")

        self.service = build('drive', 'v3', credentials=self.creds)
        return self.service

    def upload_file(self, file_path: str, filename: str) -> str:
        """Upload a file to the App Data folder."""
        service = self.get_service()
        
        # Check if file exists to update or create new
        # Search for file in appDataFolder
        results = service.files().list(
            q=f"name='{filename}' and 'appDataFolder' in parents",
            spaces='appDataFolder',
            fields="nextPageToken, files(id, name)"
        ).execute()
        items = results.get('files', [])

        file_metadata = {
            'name': filename,
            'parents': ['appDataFolder']
        }
        media = MediaFileUpload(file_path, resumable=True)

        if not items:
            # Create new
            file = service.files().create(
                body=file_metadata,
                media_body=media,
                fields='id'
            ).execute()
            logger.info(f"Created backup file ID: {file.get('id')}")
            return file.get('id')
        else:
            # Update existing
            file_id = items[0]['id']
            # For update we don't send parents
            update_metadata = {'name': filename}
            file = service.files().update(
                fileId=file_id,
                body=update_metadata,
                media_body=media,
                fields='id'
            ).execute()
            logger.info(f"Updated backup file ID: {file.get('id')}")
            return file.get('id')

    def list_backups(self):
        """List files in the App Data folder."""
        service = self.get_service()
        results = service.files().list(
            spaces='appDataFolder',
            fields="nextPageToken, files(id, name, size, modifiedTime)",
            orderBy="modifiedTime desc"
        ).execute()
        return results.get('files', [])

