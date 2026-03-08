// GalleryCard — poster card with cover, rating, favorite, brand, external badges.

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { WorkSummary, formatRating, statusColor, toAssetUrl } from '../hooks/api';
import './GalleryCard.css';

interface Props {
      work: WorkSummary;
      onClick: (id: string) => void;
}

const ASSET_LABELS: Record<string, string> = {
      game: 'Game',
      dlc: 'DLC',
      update: 'Update',
      ost: 'OST',
      voice_drama: 'Voice',
      bonus: 'Bonus',
      crack: 'Crack',
      save: 'Save',
      guide: 'Guide',
      unknown: 'Other',
};

export function GalleryCard({ work, onClick }: Props) {
      const [thumbUrl, setThumbUrl] = useState<string | null>(null);
      const [isFav, setIsFav] = useState(work.library_status === 'completed');

      useEffect(() => {
            let cancelled = false;

            async function loadThumb() {
                  try {
                        if (work.cover_path) {
                              setThumbUrl(toAssetUrl(work.cover_path));
                              return;
                        }
                        const path = await invoke<string | null>('get_thumbnail', {
                              workId: work.id, size: 250,
                        });
                        if (!cancelled && path) {
                              setThumbUrl(toAssetUrl(path));
                        }
                  } catch { }
            }

            loadThumb();
            return () => { cancelled = true; };
      }, [work.id, work.cover_path]);

      function handleFav(e: React.MouseEvent) {
            e.stopPropagation();
            setIsFav(!isFav);
            invoke('update_work_field', {
                  id: work.id,
                  field: 'library_status',
                  value: isFav ? 'unplayed' : 'completed',
            }).catch(() => { });
      }

      const hasRating = work.rating !== null && work.rating !== undefined;
      const hasVndb = !!work.vndb_id;
      const hasBgm = !!work.bangumi_id;
      const hasDlsite = !!work.dlsite_id;
      const hasVariants = work.variant_count > 1;
      const assetBadges = work.asset_types.slice(0, 2).map((assetType) => ASSET_LABELS[assetType] || assetType);
      const placeholderMonogram = work.title
            .replace(/^\[[^\]]+\]\s*/g, '')
            .trim()
            .slice(0, 2)
            .toUpperCase() || 'VN';

      return (
            <article
                  className="gallery-card"
                  onClick={() => onClick(work.id)}
                  onKeyDown={(e) => e.key === 'Enter' && onClick(work.id)}
                  tabIndex={0}
                  role="button"
                  aria-label={work.title}
            >
                  <div className="card-cover">
                        {thumbUrl ? (
                              <img src={thumbUrl} alt={work.title} loading="lazy" draggable={false} />
                        ) : (
                              <div className="card-cover-placeholder" aria-hidden="true">
                                    <div className="placeholder-orb" />
                                    <div className="placeholder-grid" />
                                    <div className="placeholder-content">
                                          <span className="placeholder-chip">No Cover</span>
                                          <span className="placeholder-icon">{placeholderMonogram}</span>
                                          <span className="placeholder-meta">{work.developer || 'Archive Library'}</span>
                                    </div>
                              </div>
                        )}

                        <button
                              className={`card-fav ${isFav ? 'active' : ''}`}
                              onClick={handleFav}
                              title={isFav ? 'Remove favorite' : 'Add favorite'}
                        >
                              {isFav ? '♥' : '♡'}
                        </button>

                        {hasRating && (
                              <div className="card-rating">★ {formatRating(work.rating)}</div>
                        )}

                        {hasVariants && (
                              <div className="card-stack-badge" title={`${work.variant_count} variants grouped`}>
                                    ×{work.variant_count}
                              </div>
                        )}

                        <div
                              className="card-status-dot"
                              style={{ background: statusColor(work.library_status) }}
                              title={work.library_status}
                        />
                  </div>

                  <div className="card-info">
                        <h3 className="card-title" title={work.title}>{work.title}</h3>
                        {work.developer && (
                              <p className="card-developer">🏢 {work.developer}</p>
                        )}

                        {assetBadges.length > 0 && (
                              <div className="card-asset-strip">
                                    {assetBadges.map((label) => (
                                          <span key={label} className="card-asset-pill">{label}</span>
                                    ))}
                                    {work.asset_types.length > assetBadges.length && (
                                          <span className="card-asset-pill muted">+{work.asset_types.length - assetBadges.length}</span>
                                    )}
                              </div>
                        )}

                        <div className="card-badges">
                              {hasVndb && <span className="badge badge-vndb">VNDB</span>}
                              {hasBgm && <span className="badge badge-bgm">BGM</span>}
                              {hasDlsite && <span className="badge badge-bgm">DLsite</span>}
                              {work.enrichment_state === 'matched' && (
                                    <span className="badge badge-matched">✓</span>
                              )}
                              {work.asset_count > 0 && <span className="badge badge-asset-count">{work.asset_count} files</span>}
                        </div>
                  </div>
            </article>
      );
}
