// GalleryGrid — responsive CSS grid or list of cards.

import { WorkSummary, toAssetUrl, formatRating, statusLabel, statusColor } from '../hooks/api';
import { GalleryCard } from './GalleryCard';
import './GalleryGrid.css';

interface Props {
      works: WorkSummary[];
      onCardClick: (id: string) => void;
      isLoading: boolean;
      viewMode?: 'grid' | 'list';
}

export function GalleryGrid({ works, onCardClick, isLoading, viewMode = 'grid' }: Props) {
      if (isLoading) {
            return (
                  <div className="gallery-grid-skeleton">
                        {Array.from({ length: 12 }).map((_, i) => (
                              <div key={i} className="skeleton-card">
                                    <div className="skeleton-cover" />
                                    <div className="skeleton-info">
                                          <div className="skeleton-title" />
                                          <div className="skeleton-developer" />
                                    </div>
                              </div>
                        ))}
                  </div>
            );
      }

      if (works.length === 0) {
            return (
                  <div className="gallery-empty">
                        <div className="empty-icon">📂</div>
                        <h2>No games found</h2>
                        <p>Add library folders in Settings, then click Scan to discover your games.</p>
                  </div>
            );
      }

      if (viewMode === 'list') {
            return (
                  <div className="gallery-list">
                        <div className="list-header">
                              <span className="lh-title">Title</span>
                              <span className="lh-dev">Developer</span>
                              <span className="lh-rating">Rating</span>
                              <span className="lh-status">Status</span>
                        </div>
                        {works.map(work => (
                              <div key={work.id} className="list-row" onClick={() => onCardClick(work.id)}>
                                    <div className="lr-cover">
                                          {work.cover_path ? (
                                                <img src={toAssetUrl(work.cover_path) || ''} alt="" />
                                          ) : (
                                                <span className="lr-cover-ph">🎮</span>
                                          )}
                                    </div>
                                    <span className="lr-title">{work.title}</span>
                                    <span className="lr-dev">{work.developer || '—'}</span>
                                    <span className="lr-rating">{work.rating ? `★ ${formatRating(work.rating)}` : '—'}</span>
                                    <span className="lr-status" style={{ color: statusColor(work.library_status) }}>
                                          {statusLabel(work.library_status)}
                                    </span>
                              </div>
                        ))}
                  </div>
            );
      }

      return (
            <div className="gallery-grid">
                  {works.map(work => (
                        <GalleryCard key={work.id} work={work} onClick={onCardClick} />
                  ))}
            </div>
      );
}
