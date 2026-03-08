// Characters — browse characters across all works.

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { toAssetUrl } from '../../hooks/api';
import './Characters.css';

interface CharacterSummary {
      id: string;
      name: string;
      name_original: string | null;
      role: string | null;
      work_id: string | null;
      work_title: string | null;
      image_url?: string | null;
}

export default function Characters() {
      const navigate = useNavigate();
      const [characters, setCharacters] = useState<CharacterSummary[]>([]);
      const [searchQuery, setSearchQuery] = useState('');
      const [isLoading, setIsLoading] = useState(true);

      useEffect(() => {
            loadCharacters('');
      }, []);

      async function loadCharacters(query: string) {
            setIsLoading(true);
            try {
                  const data = await invoke<CharacterSummary[]>('search_characters', {
                        query,
                        limit: 200,
                  });
                  setCharacters(Array.isArray(data) ? data : []);
            } catch {
                  setCharacters([]);
            } finally {
                  setIsLoading(false);
            }
      }

      useEffect(() => {
            const timer = setTimeout(() => {
                  loadCharacters(searchQuery.trim());
            }, 250);
            return () => clearTimeout(timer);
      }, [searchQuery]);

      return (
            <div className="characters-page">
                  <div className="characters-header">
                        <h1>Characters</h1>
                        <div className="characters-search-wrap">
                              <label className="characters-search-label" htmlFor="characters-search">Search characters</label>
                              <input
                                    id="characters-search"
                                    name="characters-search"
                                    type="text"
                                    className="characters-search"
                                    placeholder="Search characters..."
                                    value={searchQuery}
                                    onChange={(e) => setSearchQuery(e.target.value)}
                              />
                        </div>
                  </div>

                  {isLoading ? (
                        <div className="characters-loading">Loading...</div>
                  ) : characters.length === 0 ? (
                        <div className="characters-empty">
                              <span className="empty-icon">👤</span>
                              <p>No characters found</p>
                              <p className="empty-hint">Characters appear once work-character links are imported</p>
                        </div>
                  ) : (
                        <div className="characters-grid">
                              {characters.map((char) => (
                                    <article
                                          key={`${char.id}-${char.work_id ?? 'standalone'}`}
                                          className={`character-card ${char.work_id ? 'clickable' : ''}`}
                                          onClick={() => char.work_id && navigate(`/work/${char.work_id}`)}
                                    >
                                          <div className="character-avatar">
                                                {char.image_url ? (
                                                      <img
                                                            src={toAssetUrl(char.image_url) ?? char.image_url}
                                                            alt={char.name}
                                                            loading="lazy"
                                                      />
                                                ) : (
                                                      char.name.charAt(0)
                                                )}
                                          </div>
                                          <div className="character-info">
                                                <h3 className="character-name">{char.name}</h3>
                                                {char.name_original && (
                                                      <p className="character-name-jp">{char.name_original}</p>
                                                )}
                                                {char.role && (
                                                      <span className="character-role">{char.role}</span>
                                                )}
                                                {char.work_title ? (
                                                      <p className="character-work">{char.work_title}</p>
                                                ) : (
                                                      <p className="character-work muted">No linked work yet</p>
                                                )}
                                          </div>
                                    </article>
                              ))}
                        </div>
                  )}
            </div>
      );
}

