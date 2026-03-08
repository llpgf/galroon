// PersonDetail — detail page for a creator/VA/staff member.

import { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { toAssetUrl } from '../../hooks/api';
import './PersonDetail.css';

interface PersonWork {
      id: string;
      title: string;
      cover_path: string | null;
      rating: number | null;
      release_date: string | null;
      role: string;
      character_name: string | null;
      notes: string | null;
}

interface CreatorDetailData {
      id: string;
      name: string;
      name_original: string | null;
      role_type: string;
      image_url: string | null;
      description: string | null;
      roles: string[];
      works: PersonWork[];
}

const ROLE_LABELS: Record<string, string> = {
      voice_actor: '🎙️ Voice Actor',
      artist: '🎨 Artist',
      writer: '✍️ Writer',
      composer: '🎵 Composer',
      director: '🎬 Director',
      staff: '👤 Staff',
};

function formatRole(role: string): string {
      return ROLE_LABELS[role]?.replace(/^[^\s]+\s/, '') || role.replace(/_/g, ' ');
}

export default function PersonDetail() {
      const { id } = useParams<{ id: string }>();
      const navigate = useNavigate();
      const [person, setPerson] = useState<CreatorDetailData | null>(null);
      const [isLoading, setIsLoading] = useState(true);

      useEffect(() => {
            if (!id) return;
            loadPerson(id);
      }, [id]);

      async function loadPerson(personId: string) {
            setIsLoading(true);
            try {
                  const data = await invoke<CreatorDetailData | null>('get_creator_detail', { id: personId });
                  setPerson(data);
            } catch {
                  setPerson(null);
            } finally {
                  setIsLoading(false);
            }
      }

      if (isLoading) return <div className="person-detail loading">Loading...</div>;
      if (!person) return (
            <div className="person-detail empty">
                  <h2>Person not found</h2>
                  <button onClick={() => navigate('/creators')}>← Back</button>
            </div>
      );

      return (
            <div className="person-detail">
                  <header className="person-header">
                        <button className="back-btn" onClick={() => navigate(-1)}>← Back</button>
                        <div className="person-hero">
                              <div className="person-avatar">
                                    {person.image_url ? (
                                          <img src={toAssetUrl(person.image_url) ?? person.image_url} alt={person.name} loading="lazy" />
                                    ) : (
                                          person.name.charAt(0)
                                    )}
                              </div>
                              <div>
                                    <h1>{person.name}</h1>
                                    {person.name_original && <p className="person-jp">{person.name_original}</p>}
                                    <span className="person-role-badge">
                                          {ROLE_LABELS[person.role_type] || person.role_type}
                                    </span>
                                    {person.roles.length > 1 && (
                                          <div className="person-role-list">
                                                {person.roles.map((role) => (
                                                      <span key={role} className="person-role-chip">{formatRole(role)}</span>
                                                ))}
                                          </div>
                                    )}
                              </div>
                        </div>
                  </header>

                  {person.description && (
                        <section className="person-section person-bio">
                              <h3>Bio</h3>
                              <p>{person.description}</p>
                        </section>
                  )}

                  {person.works.length > 0 && (
                        <section className="person-section">
                              <h3>Works ({person.works.length})</h3>
                              <div className="person-works-grid">
                                    {person.works.map(w => (
                                          <article key={w.id} className="pw-card" onClick={() => navigate(`/work/${w.id}`)}>
                                                <div className="pw-cover">
                                                      {w.cover_path
                                                            ? <img src={toAssetUrl(w.cover_path) ?? w.cover_path} alt={w.title} loading="lazy" />
                                                            : <span className="pw-placeholder">🎮</span>
                                                      }
                                                </div>
                                                <div className="pw-info">
                                                      <h4>{w.title}</h4>
                                                      <div className="pw-meta">
                                                            <span className="pw-role">{formatRole(w.role)}</span>
                                                            {w.character_name && <span className="pw-character">as {w.character_name}</span>}
                                                      </div>
                                                      {w.release_date && <span className="pw-date">{w.release_date}</span>}
                                                      {w.notes && <p className="pw-notes">{w.notes}</p>}
                                                </div>
                                          </article>
                                    ))}
                              </div>
                        </section>
                  )}
            </div>
      );
}
