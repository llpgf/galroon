// Dashboard — Apple-style stats overview with Chart.js charts.

import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import {
      Chart as ChartJS,
      ArcElement,
      CategoryScale,
      LinearScale,
      BarElement,
      Tooltip,
      Legend,
} from 'chart.js';
import { Doughnut, Bar } from 'react-chartjs-2';
import { useToast } from '../../components/Toast';
import { toAssetUrl } from '../../hooks/api';
import './Dashboard.css';

ChartJS.register(ArcElement, CategoryScale, LinearScale, BarElement, Tooltip, Legend);

interface BrandCount { name: string; count: number; }
interface RatingBucket { bucket: string; count: number; }
interface RecentWork { id: string; title: string; cover_path: string | null; developer: string | null; }
interface YearlyCount { year: string; count: number; }

interface DashboardStats {
      total_works: number;
      total_brands: number;
      total_matched: number;
      total_favorites: number;
      avg_rating: number;
      match_percent: number;
      top_brands: BrandCount[];
      rating_distribution: RatingBucket[];
      recent_works: RecentWork[];
      yearly_counts: YearlyCount[];
}

function getGreeting(): string {
      const h = new Date().getHours();
      if (h < 6) return '🌙 Late night gaming?';
      if (h < 12) return '☀️ Good morning';
      if (h < 18) return '🌤️ Good afternoon';
      return '🌙 Good evening';
}

export default function Dashboard() {
      const { showToast } = useToast();
      const navigate = useNavigate();
      const [stats, setStats] = useState<DashboardStats | null>(null);
      const [isLoading, setIsLoading] = useState(true);
      const [sfwMode, setSfwMode] = useState(false);
      const hasLoadedRef = useRef(false);

      useEffect(() => {
            if (hasLoadedRef.current) {
                  return;
            }
            hasLoadedRef.current = true;
            loadStats();
      }, []);

      async function loadStats() {
            setIsLoading(true);
            try {
                  const data = await invoke<DashboardStats>('get_dashboard_stats');
                  setStats(data);
            } catch {
                  showToast('Failed to load dashboard stats', 'error');
            } finally {
                  setIsLoading(false);
            }
      }

      async function handleToggleSfw() {
            try {
                  const newVal = await invoke<boolean>('toggle_sfw');
                  setSfwMode(newVal);
                  showToast(newVal ? 'SFW mode ON' : 'SFW mode OFF', 'success');
            } catch {
                  showToast('Failed to toggle SFW mode', 'error');
            }
      }

      if (isLoading) {
            return (
                  <div className="dashboard">
                        <div className="dash-loading">
                              <div className="dash-spinner" />
                              <p>Loading your library...</p>
                        </div>
                  </div>
            );
      }

      if (!stats) {
            return (
                  <div className="dashboard">
                        <div className="dash-empty">
                              <span className="dash-empty-icon">📊</span>
                              <h2>No data yet</h2>
                              <p>Add library folders in Settings and scan to get started.</p>
                        </div>
                  </div>
            );
      }

      // Chart: Rating Distribution (Doughnut)
      const ratingColors = ['#22c55e', '#4ade80', '#facc15', '#fb923c', '#ef4444'];
      const ratingData = {
            labels: stats.rating_distribution.map(r => r.bucket),
            datasets: [{
                  data: stats.rating_distribution.map(r => r.count),
                  backgroundColor: ratingColors,
                  borderWidth: 0,
                  hoverOffset: 6,
            }],
      };
      const hasRatingData = stats.rating_distribution.some((bucket) => bucket.count > 0);
      const hasBrandData = stats.top_brands.length > 0;

      const doughnutOpts = {
            responsive: true,
            maintainAspectRatio: false,
            cutout: '65%',
            plugins: {
                  legend: { position: 'bottom' as const, labels: { color: '#aaa', padding: 12, font: { size: 11 } } },
            },
      };

      // Chart: Top Brands (Horizontal Bar)
      const brandData = {
            labels: stats.top_brands.map(b => b.name),
            datasets: [{
                  data: stats.top_brands.map(b => b.count),
                  backgroundColor: '#4f8cff',
                  borderRadius: 4,
                  barThickness: 18,
            }],
      };
      const barOpts = {
            indexAxis: 'y' as const,
            responsive: true,
            maintainAspectRatio: false,
            plugins: { legend: { display: false } },
            scales: {
                  x: {
                        beginAtZero: true,
                        grid: { color: '#222' },
                        ticks: { color: '#888', precision: 0, stepSize: 1 },
                  },
                  y: {
                        grid: { display: false },
                        ticks: {
                              color: '#ccc',
                              font: { size: 11 },
                              callback: (_value: unknown, index: number) => {
                                    const label = stats.top_brands[index]?.name ?? '';
                                    return label.length > 18 ? `${label.slice(0, 18)}…` : label;
                              },
                        },
                  },
            },
      };

      return (
            <div className="dashboard">
                  {/* Hero greeting */}
                  <div className="dash-hero">
                        <div className="dash-greeting">
                              <h1>{getGreeting()}</h1>
                              <p className="dash-subtitle">Your library at a glance</p>
                        </div>
                        <button className="sfw-toggle" onClick={handleToggleSfw} title="Toggle SFW mode">
                              {sfwMode ? '🔒 SFW' : '🔓 NSFW'}
                        </button>
                  </div>

                  {/* Stat cards */}
                  <div className="stats-grid">
                        <div className="stat-card">
                              <span className="stat-icon">🎮</span>
                              <div className="stat-value">{stats.total_works.toLocaleString()}</div>
                              <div className="stat-label">Works</div>
                        </div>
                        <div className="stat-card">
                              <span className="stat-icon">🏢</span>
                              <div className="stat-value">{stats.total_brands.toLocaleString()}</div>
                              <div className="stat-label">Brands</div>
                        </div>
                        <div className="stat-card">
                              <span className="stat-icon">⭐</span>
                              <div className="stat-value">{stats.avg_rating.toFixed(1)}</div>
                              <div className="stat-label">Avg Rating</div>
                        </div>
                        <div className="stat-card">
                              <span className="stat-icon">✅</span>
                              <div className="stat-value">{stats.match_percent.toFixed(0)}%</div>
                              <div className="stat-label">Matched</div>
                        </div>
                        <div className="stat-card health-card">
                              <span className="stat-icon">🩺</span>
                              <div className="stat-value">{stats.total_works - stats.total_matched}</div>
                              <div className="stat-label">Need Attention</div>
                        </div>
                  </div>

                  {/* Charts row */}
                  <div className="charts-row">
                        <div className="chart-card">
                              <h3>Rating Distribution</h3>
                              <div className="chart-wrap">
                                    {hasRatingData ? (
                                          <Doughnut data={ratingData} options={doughnutOpts} />
                                    ) : (
                                          <div className="chart-empty">
                                                <span className="chart-empty-icon">⭐</span>
                                                <p>No ratings yet</p>
                                                <span>Ratings will appear after you enrich or score works.</span>
                                          </div>
                                    )}
                              </div>
                        </div>
                        <div className="chart-card">
                              <h3>Top Brands</h3>
                              <div className="chart-wrap">
                                    {hasBrandData ? (
                                          <Bar data={brandData} options={barOpts} />
                                    ) : (
                                          <div className="chart-empty">
                                                <span className="chart-empty-icon">🏢</span>
                                                <p>No developers detected</p>
                                                <span>Folder parsing has not extracted brand names from this library yet.</span>
                                          </div>
                                    )}
                              </div>
                        </div>
                  </div>

                  {/* Recent works */}
                  {stats.recent_works.length > 0 && (
                        <div className="dash-section">
                              <h3>Recently Added</h3>
                              <div className="recent-row">
                                    {stats.recent_works.map(w => (
                                          <button
                                                key={w.id}
                                                className="recent-card"
                                                type="button"
                                                onClick={() => navigate(`/work/${w.id}`)}
                                          >
                                                <div className="recent-cover">
                                                      {w.cover_path
                                                            ? <img src={toAssetUrl(w.cover_path) || ''} alt={w.title} />
                                                            : <span className="recent-placeholder">🎮</span>
                                                      }
                                                </div>
                                                <div className="recent-title">{w.title}</div>
                                                {w.developer && <div className="recent-dev">{w.developer}</div>}
                                          </button>
                                    ))}
                              </div>
                        </div>
                  )}
            </div>
      );
}


