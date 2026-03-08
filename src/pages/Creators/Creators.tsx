// Creators — browse voice actors, artists, writers.

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { toAssetUrl } from '../../hooks/api';
import './Creators.css';

interface Creator {
      id: string;
      name: string;
      name_original: string | null;
      role_type: string;
      works_count: number;
      image_url: string | null;
}

const ROLE_LABELS: Record<string, string> = {
      voice_actor: 'Voice Actor',
      artist: 'Artist',
      writer: 'Writer',
      composer: 'Composer',
      director: 'Director',
      staff: 'Staff',
};

const ROLE_ICONS: Record<string, string> = {
      voice_actor: '🎙️',
      artist: '🎨',
      writer: '✍️',
      composer: '🎵',
      director: '🎬',
      staff: '👤',
};

export default function Creators() {
      const navigate = useNavigate();
      const [creators, setCreators] = useState<Creator[]>([]);
      const [roleFilter, setRoleFilter] = useState<string | null>(null);
      const [isLoading, setIsLoading] = useState(true);

      useEffect(() => {
            loadCreators();
      }, []);

      async function loadCreators() {
            setIsLoading(true);
            try {
                  const data = await invoke<Creator[]>('list_creators', { limit: 500 });
                  setCreators(Array.isArray(data) ? data : []);
            } catch {
                  setCreators([]);
            } finally {
                  setIsLoading(false);
            }
      }

      const filtered = roleFilter
            ? creators.filter((creator) => creator.role_type === roleFilter)
            : creators;

      return (
            <div className="creators-page">
                  <h1>Creators</h1>

                  <div className="creators-filters">
                        <button
                              className={`role-pill ${!roleFilter ? 'active' : ''}`}
                              onClick={() => setRoleFilter(null)}
                        >
                              All
                        </button>
                        {Object.entries(ROLE_LABELS).map(([key, label]) => (
                              <button
                                    key={key}
                                    className={`role-pill ${roleFilter === key ? 'active' : ''}`}
                                    onClick={() => setRoleFilter(key)}
                              >
                                    {ROLE_ICONS[key]} {label}
                              </button>
                        ))}
                  </div>

                  {isLoading ? (
                        <div className="creators-loading">Loading...</div>
                  ) : filtered.length === 0 ? (
                        <div className="creators-empty">
                              <span className="empty-icon">🎭</span>
                              <p>No creators found</p>
                              <p className="empty-hint">Creator data appears when persons and work credits are imported</p>
                        </div>
                  ) : (
                        <div className="creators-grid">
                              {filtered.map((creator) => (
                                    <article
                                          key={creator.id}
                                          className="creator-card"
                                          onClick={() => navigate(`/person/${creator.id}`)}
                                    >
                                          <div className="creator-icon">
                                                {creator.image_url ? (
                                                      <img
                                                            src={toAssetUrl(creator.image_url) ?? creator.image_url}
                                                            alt={creator.name}
                                                            loading="lazy"
                                                      />
                                                ) : (
                                                      ROLE_ICONS[creator.role_type] || '👤'
                                                )}
                                          </div>
                                          <div className="creator-info">
                                                <h3>{creator.name}</h3>
                                                {creator.name_original && (
                                                      <p className="creator-name-jp">{creator.name_original}</p>
                                                )}
                                                <div className="creator-meta">
                                                      <span className="creator-role-badge">
                                                            {ROLE_LABELS[creator.role_type] || creator.role_type}
                                                      </span>
                                                      <span className="creator-works-count">
                                                            {creator.works_count} work{creator.works_count !== 1 ? 's' : ''}
                                                      </span>
                                                </div>
                                          </div>
                                          <span className="creator-open">Open</span>
                                    </article>
                              ))}
                        </div>
                  )}
            </div>
      );
}
