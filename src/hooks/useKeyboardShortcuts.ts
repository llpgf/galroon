// useKeyboardShortcuts — global keyboard shortcuts for Galroon.
//
// J/K: navigate, F: favorite, /: focus search, Esc: back/clear

import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';

export function useKeyboardShortcuts() {
      const navigate = useNavigate();

      useEffect(() => {
            function handler(e: KeyboardEvent) {
                  const target = e.target as HTMLElement;
                  const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

                  // "/" focuses search (unless already in input)
                  if (e.key === '/' && !isInput) {
                        e.preventDefault();
                        const search = document.getElementById('gallery-search') as HTMLInputElement | null;
                        search?.focus();
                        return;
                  }

                  // Esc: blur input or go back
                  if (e.key === 'Escape') {
                        if (isInput) {
                              (target as HTMLElement).blur();
                        } else {
                              navigate(-1);
                        }
                        return;
                  }

                  // Skip remaining shortcuts when typing
                  if (isInput) return;

                  // Navigation
                  if (e.key === 'g') { navigate('/library'); return; }
                  if (e.key === 'd') { navigate('/'); return; }
                  if (e.key === 's') { navigate('/settings'); return; }
            }

            window.addEventListener('keydown', handler);
            return () => window.removeEventListener('keydown', handler);
      }, [navigate]);
}
