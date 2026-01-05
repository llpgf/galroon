import { api } from './client';

/**
 * UtilityAPI - Phase 18: The Great Wiring
 *
 * Integration with backend utility endpoints via centralized API client
 *
 * Provides helper tools for file management:
 * - Reveal in Explorer
 * - Copy to clipboard
 * - Open file/folder
 * - Extract archives
 */

/**
 * Response wrapper for utility API calls
 */
interface UtilityResponse {
  success: boolean;
  operation?: string;
  message?: string;
  error?: string;
}

/**
 * Reveal file/folder in Explorer (SELECTS the file)
 *
 * Endpoint: POST /api/utils/reveal
 */
export const revealInExplorer = async (path: string): Promise<UtilityResponse> => {
  try {
    const response = await api.revealFolder(path);
    return {
      success: true,
      operation: 'reveal',
      message: 'Folder revealed in Explorer',
    };
  } catch (error) {
    console.error('[UtilityAPI] Failed to reveal in Explorer:', error);
    return {
      success: false,
      operation: 'reveal',
      error: error instanceof Error ? error.message : 'Failed to reveal',
    };
  }
};

/**
 * Copy text to clipboard
 *
 * Endpoint: POST /api/utils/copy
 */
export const copyToClipboard = async (text: string): Promise<UtilityResponse> => {
  try {
    const response = await api.copyToClipboard(text);
    return {
      success: true,
      operation: 'copy',
      message: 'Text copied to clipboard',
    };
  } catch (error) {
    console.error('[UtilityAPI] Failed to copy to clipboard:', error);
    return {
      success: false,
      operation: 'copy',
      error: error instanceof Error ? error.message : 'Failed to copy',
    };
  }
};

/**
 * Open file/folder with default application
 *
 * Endpoint: POST /api/utils/open
 *
 * Phase 19: Now using centralized api.openFile() from api/client.ts
 */
export const openFile = async (path: string): Promise<UtilityResponse> => {
  try {
    await api.openFile(path);
    return {
      success: true,
      operation: 'open',
      message: 'File opened with default application',
    };
  } catch (error) {
    console.error('[UtilityAPI] Failed to open file:', error);
    return {
      success: false,
      operation: 'open',
      error: error instanceof Error ? error.message : 'Failed to open',
    };
  }
};

/**
 * Get status of utility services
 *
 * Endpoint: GET /api/utils/status
 */
export const getUtilityStatus = async () => {
  try {
    const response = await api.getUtilityStatus();
    return response.data;
  } catch (error) {
    console.error('[UtilityAPI] Failed to get utility status:', error);
    return null;
  }
};

/**
 * UtilityAPI class for convenient access
 *
 * Provides static methods for common utility operations
 */
export class UtilityAPI {
  /**
   * Reveal game folder in Explorer
   *
   * Usage: UtilityAPI.revealGameFolder('C:/Games/MyGame')
   */
  static async revealGameFolder(gamePath: string): Promise<boolean> {
    const result = await revealInExplorer(gamePath);
    return result.success;
  }

  /**
   * Copy game path to clipboard
   *
   * Usage: UtilityAPI.copyGamePath('C:/Games/MyGame')
   */
  static async copyGamePath(gamePath: string): Promise<boolean> {
    const result = await copyToClipboard(gamePath);
    return result.success;
  }

  /**
   * Open PDF manual
   *
   * Usage: UtilityAPI.openManual('C:/Games/MyGame/manual.pdf')
   */
  static async openManual(manualPath: string): Promise<boolean> {
    const result = await openFile(manualPath);
    return result.success;
  }

  /**
   * Mount ISO (open with default app)
   *
   * Usage: UtilityAPI.mountISO('C:/Games/MyGame/game.iso')
   *
   * TODO: Implement proper ISO mounting in backend
   * Currently just opens the ISO file with default application
   */
  static async mountISO(isoPath: string): Promise<boolean> {
    const result = await openFile(isoPath);
    return result.success;
  }

  /**
   * Trigger archive extraction
   *
   * Usage:
   * ```typescript
   * const taskId = await UtilityAPI.extractArchive(
   *   'C:/Games/game.7z',
   *   'C:/Games/Extracted'
   * );
   * if (taskId) {
   *   // Poll task status with api.getTaskStatus(taskId)
   * }
   * ```
   *
   * Endpoint: POST /api/utils/extract
   * Returns: task_id (string) for polling, or null if failed
   */
  static async extractArchive(
    archivePath: string,
    targetDir: string
  ): Promise<string | null> {
    try {
      const response = await api.extractArchive(archivePath, targetDir);

      if (response.data?.success) {
        return response.data.task_id || null;
      }

      return null;
    } catch (error) {
      console.error('[UtilityAPI] Failed to extract archive:', error);
      return null;
    }
  }
}

export default UtilityAPI;
