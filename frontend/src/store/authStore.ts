import { create } from 'zustand';
import { api } from '../api/client';

interface AuthState {
      isGDriveAuthenticated: boolean;
      lastBackup: string | null;
      checkAuthStatus: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
      isGDriveAuthenticated: false,
      lastBackup: null,

      checkAuthStatus: async () => {
            try {
                  const response = await api.getBackupStatus();
                  set({
                        isGDriveAuthenticated: response.data?.authenticated || false,
                        lastBackup: response.data?.last_backup || null,
                  });
            } catch (error) {
                  console.error('Failed to check auth status:', error);
                  set({ isGDriveAuthenticated: false });
            }
      },
}));
