// WorkspacePicker — first launch / workspace selection page.

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import './WorkspacePicker.css';

export default function WorkspacePicker() {
      const navigate = useNavigate();
      const [recentWorkspaces, setRecentWorkspaces] = useState<string[]>([]);
      const [newPath, setNewPath] = useState('');
      const [error, setError] = useState('');

      useEffect(() => {
            loadRecent();
      }, []);

      async function loadRecent() {
            try {
                  const recent = await invoke<string[]>('get_recent_workspaces');
                  setRecentWorkspaces(recent);
            } catch {
                  // No recent workspaces
            }
      }

      async function openWorkspace(path: string) {
            setError('');
            try {
                  await invoke('switch_workspace', { path });
                  navigate('/');
            } catch (e) {
                  setError(`Failed to open workspace: ${e}`);
            }
      }

      function handleCreate() {
            if (!newPath.trim()) {
                  setError('Please enter a workspace path');
                  return;
            }
            openWorkspace(newPath.trim());
      }

      return (
            <div className="workspace-picker">
                  <div className="workspace-picker-card">
                        <div className="picker-icon">🎮</div>
                        <h1>Galroon</h1>
                        <p className="picker-subtitle">Choose or create a workspace</p>

                        {/* Recent workspaces */}
                        {recentWorkspaces.length > 0 && (
                              <div className="recent-section">
                                    <h3>Recent Workspaces</h3>
                                    <div className="recent-list">
                                          {recentWorkspaces.map((ws) => (
                                                <button
                                                      key={ws}
                                                      className="recent-item"
                                                      onClick={() => openWorkspace(ws)}
                                                      title={ws}
                                                >
                                                      <span className="recent-icon">📂</span>
                                                      <span className="recent-path">{ws}</span>
                                                </button>
                                          ))}
                                    </div>
                              </div>
                        )}

                        {/* Create / open */}
                        <div className="create-section">
                              <h3>Open or Create Workspace</h3>
                              <div className="path-input-row">
                                    <input
                                          type="text"
                                          placeholder="D:\MyGames\GalroonWorkspace"
                                          value={newPath}
                                          onChange={(e) => setNewPath(e.target.value)}
                                    />
                                    <button className="open-btn" onClick={handleCreate}>
                                          Open
                                    </button>
                              </div>
                        </div>

                        {error && <div className="picker-error">{error}</div>}

                        <p className="picker-hint">
                              Workspace contains all your data (DB, thumbnails, config).
                              <br />
                              Backup = copy the folder. Restore = point here.
                        </p>
                  </div>
            </div>
      );
}
