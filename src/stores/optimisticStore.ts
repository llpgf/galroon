// Optimistic Update Store — immediate UI updates with backend reconciliation (R23).

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

interface PendingUpdate {
      field: string;
      value: unknown;
      timestamp: number;
}

interface OptimisticState {
      pendingUpdates: Map<string, PendingUpdate>;
      optimisticUpdate: (workId: string, field: string, value: unknown) => void;
      rollback: (workId: string, field: string) => void;
}

export const useOptimisticStore = create<OptimisticState>((set, get) => ({
      pendingUpdates: new Map(),

      optimisticUpdate: (workId: string, field: string, value: unknown) => {
            const key = `${workId}:${field}`;

            // Record pending update
            set((state) => {
                  const next = new Map(state.pendingUpdates);
                  next.set(key, { field, value, timestamp: Date.now() });
                  return { pendingUpdates: next };
            });

            // Send to backend
            invoke('update_work_field', { id: workId, field, value: String(value) })
                  .then(() => {
                        // Success — remove from pending
                        set((state) => {
                              const next = new Map(state.pendingUpdates);
                              next.delete(key);
                              return { pendingUpdates: next };
                        });
                  })
                  .catch((err) => {
                        console.error('Optimistic update failed, rolling back:', err);
                        get().rollback(workId, field);
                  });
      },

      rollback: (workId: string, field: string) => {
            const key = `${workId}:${field}`;
            set((state) => {
                  const next = new Map(state.pendingUpdates);
                  next.delete(key);
                  return { pendingUpdates: next };
            });
            // The component should re-fetch from backend on rollback event
      },
}));
