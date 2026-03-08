// SFW Store — privacy blur toggle for covers. Persisted to localStorage.

import { create } from 'zustand';

interface SfwState {
      sfwMode: boolean;
      toggleSfw: () => void;
}

export const useSfwStore = create<SfwState>((set) => ({
      sfwMode: localStorage.getItem('galroon_sfw') === 'true',

      toggleSfw: () => set((state) => {
            const next = !state.sfwMode;
            localStorage.setItem('galroon_sfw', String(next));

            // Toggle CSS class on document for global blur
            document.documentElement.classList.toggle('sfw-mode', next);

            return { sfwMode: next };
      }),
}));
