// BrandDetail — shows all works by a brand/developer.

import { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import './BrandDetail.css';

interface BrandWork {
      id: string;
      title: string;
      cover_path: string | null;
      rating: number | null;
      release_date: string | null;
}

interface BrandDetailData {
      name: string;
      works_count: number;
      works: BrandWork[];
}

export default function BrandDetail() {
      const { name } = useParams<{ name: string }>();
      const navigate = useNavigate();
      const [brand, setBrand] = useState<BrandDetailData | null>(null);
      const [isLoading, setIsLoading] = useState(true);

      useEffect(() => {
            if (!name) return;
            loadBrand(decodeURIComponent(name));
      }, [name]);

      async function loadBrand(brandName: string) {
            setIsLoading(true);
            try {
                  const data = await invoke<BrandDetailData>('get_brand_detail', { name: brandName });
                  setBrand(data);
            } catch {
                  setBrand(null);
            } finally {
                  setIsLoading(false);
            }
      }

      if (isLoading) return <div className="brand-detail loading">Loading...</div>;
      if (!brand) return (
            <div className="brand-detail empty">
                  <h2>Brand not found</h2>
                  <button onClick={() => navigate('/creators')}>← Back</button>
            </div>
      );

      return (
            <div className="brand-detail">
                  <header className="brand-header">
                        <button className="back-btn" onClick={() => navigate(-1)}>← Back</button>
                        <div className="brand-hero">
                              <div className="brand-icon">🏢</div>
                              <div>
                                    <h1>{brand.name}</h1>
                                    <p className="brand-count">{brand.works_count} works</p>
                              </div>
                        </div>
                  </header>

                  <div className="brand-works-grid">
                        {brand.works.map(w => (
                              <article key={w.id} className="brand-work-card" onClick={() => navigate(`/work/${w.id}`)}>
                                    <div className="bw-cover">
                                          {w.cover_path
                                                ? <img src={convertFileSrc(w.cover_path)} alt={w.title} loading="lazy" />
                                                : <span className="bw-placeholder">🎮</span>
                                          }
                                    </div>
                                    <div className="bw-info">
                                          <h3>{w.title}</h3>
                                          {w.release_date && <span className="bw-date">{w.release_date}</span>}
                                          {w.rating && <span className="bw-rating">★ {w.rating.toFixed(1)}</span>}
                                    </div>
                              </article>
                        ))}
                  </div>
            </div>
      );
}
