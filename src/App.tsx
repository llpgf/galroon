// App layout — workspace check → sidebar + routed content OR workspace picker.

import { useState, useEffect } from 'react';
import { Outlet } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { Sidebar } from './components/Sidebar';
import { ToastProvider } from './components/Toast';
import './App.css';

type ThemeMode = 'system' | 'dark' | 'light';

function resolveTheme(mode: ThemeMode): 'dark' | 'light' {
      if (mode === 'system') {
            return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
      }
      return mode;
}

function applyTheme(mode: ThemeMode) {
      const resolved = resolveTheme(mode);
      document.documentElement.setAttribute('data-theme', resolved);
      document.documentElement.setAttribute('data-theme-mode', mode);
      localStorage.setItem('galroon_theme', mode);
}

applyTheme((localStorage.getItem('galroon_theme') as ThemeMode) || 'system');

type AppPhase = 'loading' | 'setup' | 'ready';

interface WorkspaceStatus {
      has_workspace: boolean;
      workspace_path: string | null;
      recent_workspaces: string[];
}

function App() {
      const [phase, setPhase] = useState<AppPhase>('loading');
      const [recentWorkspaces, setRecentWorkspaces] = useState<string[]>([]);
      const [wsPath, setWsPath] = useState('');
      const [error, setError] = useState('');

      useEffect(() => {
            checkWorkspace();
      }, []);

      useEffect(() => {
            const media = window.matchMedia('(prefers-color-scheme: light)');
            const handleThemeChange = () => {
                  const mode = (document.documentElement.getAttribute('data-theme-mode') as ThemeMode) || 'system';
                  if (mode === 'system') {
                        applyTheme('system');
                  }
            };
            media.addEventListener('change', handleThemeChange);
            return () => media.removeEventListener('change', handleThemeChange);
      }, []);

      async function checkWorkspace() {
            try {
                  const status = await invoke<WorkspaceStatus>('check_workspace_status');
                  if (status.has_workspace) {
                        try {
                              const settings = await invoke<{ theme?: ThemeMode }>('get_settings');
                              applyTheme((settings.theme as ThemeMode) || 'system');
                        } catch {
                              // Keep local fallback theme
                        }
                        setPhase('ready');
                  } else {
                        setRecentWorkspaces(status.recent_workspaces);
                        setPhase('setup');
                  }
            } catch {
                  // Backend might not have the command yet — assume ready
                  setPhase('ready');
            }
      }

      async function handleSetupWorkspace(path: string) {
            setError('');
            if (!path.trim()) {
                  setError('Please enter a workspace path');
                  return;
            }
            try {
                  await invoke('init_workspace', { path: path.trim() });
                  setPhase('ready');
                  window.location.reload(); // Reload to re-init backend with new workspace
            } catch (e) {
                  setError(`Failed: ${e}`);
            }
      }

      // ── Loading ──
      if (phase === 'loading') {
            return (
                  <div className="app-loading">
                        <div className="loading-spinner" />
                        <p>Loading Galroon...</p>
                  </div>
            );
      }

      // ── First-launch workspace setup ──
      if (phase === 'setup') {
            return (
                  <div className="workspace-setup">
                        <div className="setup-card">
                              <div className="setup-icon">🎮</div>
                              <h1>Welcome to Galroon</h1>
                              <p className="setup-subtitle">Choose where to store your library data</p>

                              {recentWorkspaces.length > 0 && (
                                    <div className="setup-section">
                                          <h3>Recent Workspaces</h3>
                                          {recentWorkspaces.map((ws) => (
                                                <button
                                                      key={ws}
                                                      className="recent-ws-btn"
                                                      onClick={() => handleSetupWorkspace(ws)}
                                                >
                                                      📂 {ws}
                                                </button>
                                          ))}
                                    </div>
                              )}

                              <div className="setup-section">
                                    <h3>Create or Open Workspace</h3>
                                    <div className="setup-input-row">
                                          <input
                                                type="text"
                                                placeholder="D:\MyGames\GalroonWorkspace"
                                                value={wsPath}
                                                onChange={(e) => setWsPath(e.target.value)}
                                                onKeyDown={(e) => e.key === 'Enter' && handleSetupWorkspace(wsPath)}
                                          />
                                          <button onClick={() => handleSetupWorkspace(wsPath)}>
                                                Start
                                          </button>
                                    </div>
                              </div>

                              {error && <p className="setup-error">{error}</p>}

                              <p className="setup-hint">
                                    All data (database, thumbnails, config) lives in this folder.
                                    <br />Backup = copy the folder. Restore = point here again.
                              </p>
                        </div>
                  </div>
            );
      }

      // ── Normal app ──
      return <ReadyApp />;
}

function ReadyApp() {
      return (
            <ToastProvider>
                  <div className="app-layout">
                        <Sidebar />
                        <main className="app-content">
                              <Outlet />
                        </main>
                  </div>
            </ToastProvider>
      );
}

export default App;
