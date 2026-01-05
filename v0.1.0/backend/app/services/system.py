"""
System Bridge Service for Galgame Library Manager.

**THE UTILITY BELT: Manual Helper Tools**

Provides manual file operations that help users manage their library:
- Reveal file in Explorer (and SELECT it)
- Copy path to clipboard
- Open file with default application

Core Philosophy: Helper, not Manager.
User decides, App executes.
"""

import logging
import subprocess
import platform
from pathlib import Path
from typing import Dict, Any

logger = logging.getLogger(__name__)


class SystemBridge:
    """
    Manual file operation helpers.

    All operations are explicit - no automation, no smart logic.
    User asks → App does.
    """

    @staticmethod
    def reveal_in_explorer(file_path: str) -> Dict[str, Any]:
        """
        Reveal file in Windows Explorer and SELECT it.

        Windows: Uses 'explorer /select,' command
        macOS: Uses 'open -R' command
        Linux: Uses 'dbus' to show file in file manager

        Args:
            file_path: Path to file or folder to reveal

        Returns:
            Dict with success status and message
        """
        try:
            path = Path(file_path)

            if not path.exists():
                return {
                    "success": False,
                    "operation": "reveal",
                    "message": f"Path does not exist: {file_path}"
                }

            system = platform.system()

            if system == "Windows":
                # Windows: explorer /select,"path" - SELECTS the file
                subprocess.run(
                    ["explorer", "/select,", str(path.resolve())],
                    check=True
                )
                logger.info(f"Revealed in Explorer: {path}")

            elif system == "Darwin":  # macOS
                # macOS: open -R path - Reveals in Finder and selects
                subprocess.run(["open", "-R", str(path.resolve())], check=True)
                logger.info(f"Revealed in Finder: {path}")

            else:  # Linux
                # Linux: Try dbus call to show file in default file manager
                try:
                    subprocess.run([
                        "dbus-send",
                        "--session",
                        "--dest=org.freedesktop.FileManager1",
                        "--type=method_call",
                        "/org/freedesktop/FileManager1",
                        "org.freedesktop.FileManager1.ShowItems",
                        f"array:string:file://{path.resolve()}",
                        "string:"
                    ], check=True)
                    logger.info(f"Revealed in file manager: {path}")

                except (subprocess.CalledProcessError, FileNotFoundError):
                    # Fallback: xdg-open the parent directory
                    subprocess.run(["xdg-open", str(path.parent.resolve())])
                    logger.info(f"Opened parent directory: {path.parent}")

            return {
                "success": True,
                "operation": "reveal",
                "message": f"Revealed in Explorer: {path.name}"
            }

        except Exception as e:
            logger.error(f"Failed to reveal in Explorer: {e}")
            return {
                "success": False,
                "operation": "reveal",
                "message": f"Failed to reveal: {str(e)}"
            }

    @staticmethod
    def copy_to_clipboard(text: str) -> Dict[str, Any]:
        """
        Copy text to clipboard.

        Windows: Uses 'clip' command
        macOS: Uses 'pbcopy' command
        Linux: Uses 'xclip' or 'xsel' command

        Args:
            text: Text to copy to clipboard

        Returns:
            Dict with success status and message
        """
        try:
            system = platform.system()

            if system == "Windows":
                # Windows: echo "text" | clip
                subprocess.run(
                    ["clip"],
                    input=text.encode('utf-8'),
                    check=True
                )
                logger.info(f"Copied to clipboard: {text[:50]}...")

            elif system == "Darwin":  # macOS
                # macOS: pbcopy
                subprocess.run(
                    ["pbcopy"],
                    input=text.encode('utf-8'),
                    check=True
                )
                logger.info(f"Copied to clipboard: {text[:50]}...")

            else:  # Linux
                # Linux: Try xclip first, then xsel
                try:
                    subprocess.run(
                        ["xclip", "-selection", "clipboard"],
                        input=text.encode('utf-8'),
                        check=True
                    )
                    logger.info(f"Copied to clipboard (xclip): {text[:50]}...")

                except (subprocess.CalledProcessError, FileNotFoundError):
                    try:
                        subprocess.run(
                            ["xsel", "--clipboard", "--input"],
                            input=text.encode('utf-8'),
                            check=True
                        )
                        logger.info(f"Copied to clipboard (xsel): {text[:50]}...")

                    except (subprocess.CalledProcessError, FileNotFoundError):
                        return {
                            "success": False,
                            "operation": "copy",
                            "message": "Clipboard tool not found. Install xclip or xsel."
                        }

            return {
                "success": True,
                "operation": "copy",
                "message": "Copied to clipboard"
            }

        except Exception as e:
            logger.error(f"Failed to copy to clipboard: {e}")
            return {
                "success": False,
                "operation": "copy",
                "message": f"Failed to copy: {str(e)}"
            }

    @staticmethod
    def open_file(file_path: str) -> Dict[str, Any]:
        """
        Open file with default associated application.

        Windows: os.startfile() or 'start' command
        macOS: 'open' command
        Linux: 'xdg-open' command

        Args:
            file_path: Path to file to open

        Returns:
            Dict with success status and message
        """
        try:
            path = Path(file_path)

            if not path.exists():
                return {
                    "success": False,
                    "operation": "open",
                    "message": f"File does not exist: {file_path}"
                }

            system = platform.system()

            if system == "Windows":
                # Windows: os.startfile() - opens with default app
                os_startfile = getattr(__import__('os'), 'startfile', None)
                if os_startfile:
                    os_startfile(str(path.resolve()))
                else:
                    # Fallback to start command
                    subprocess.run(["start", "", str(path.resolve())], shell=True)
                logger.info(f"Opened file: {path.name}")

            elif system == "Darwin":  # macOS
                # macOS: open
                subprocess.run(["open", str(path.resolve())], check=True)
                logger.info(f"Opened file: {path.name}")

            else:  # Linux
                # Linux: xdg-open
                subprocess.run(["xdg-open", str(path.resolve())], check=True)
                logger.info(f"Opened file: {path.name}")

            return {
                "success": True,
                "operation": "open",
                "message": f"Opened: {path.name}"
            }

        except Exception as e:
            logger.error(f"Failed to open file: {e}")
            return {
                "success": False,
                "operation": "open",
                "message": f"Failed to open: {str(e)}"
            }

    @staticmethod
    def open_directory(directory_path: str) -> Dict[str, Any]:
        """
        Open directory in file manager.

        Args:
            directory_path: Path to directory to open

        Returns:
            Dict with success status and message
        """
        try:
            path = Path(directory_path)

            if not path.exists():
                return {
                    "success": False,
                    "operation": "open_directory",
                    "message": f"Directory does not exist: {directory_path}"
                }

            if not path.is_dir():
                return {
                    "success": False,
                    "operation": "open_directory",
                    "message": f"Path is not a directory: {directory_path}"
                }

            system = platform.system()

            if system == "Windows":
                subprocess.run(["explorer", str(path.resolve())])
            elif system == "Darwin":
                subprocess.run(["open", str(path.resolve())])
            else:
                subprocess.run(["xdg-open", str(path.resolve())])

            logger.info(f"Opened directory: {path.name}")

            return {
                "success": True,
                "operation": "open_directory",
                "message": f"Opened folder: {path.name}"
            }

        except Exception as e:
            logger.error(f"Failed to open directory: {e}")
            return {
                "success": False,
                "operation": "open_directory",
                "message": f"Failed to open folder: {str(e)}"
            }


# Singleton instance
_system_bridge: SystemBridge = None


def get_system_bridge() -> SystemBridge:
    """
    Get or create SystemBridge singleton.

    Returns:
        SystemBridge instance
    """
    global _system_bridge
    if _system_bridge is None:
        _system_bridge = SystemBridge()
    return _system_bridge
