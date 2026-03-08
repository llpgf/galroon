// Year-in-Review — annual stats visualization.

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Bar } from 'react-chartjs-2';
import './YearInReview.css';

interface YirData {
      year: number;
      total_added: number;
      total_completed: number;
      total_hours_est: number;
      top_brands: { name: string; count: number }[];
      monthly_breakdown: { month: string; count: number }[];
}

const MONTH_NAMES = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

export default function YearInReview() {
      const [data, setData] = useState<YirData | null>(null);
      const [year, setYear] = useState(new Date().getFullYear());

      useEffect(() => { load(); }, [year]);

      async function load() {
            try {
                  const d = await invoke<YirData>('get_year_in_review', { year });
                  setData(d);
            } catch { setData(null); }
      }

      const chartData = data ? {
            labels: data.monthly_breakdown.map(m => MONTH_NAMES[parseInt(m.month, 10) - 1] || m.month),
            datasets: [{
                  label: 'Works Added',
                  data: data.monthly_breakdown.map(m => m.count),
                  backgroundColor: 'rgba(79, 140, 255, 0.6)',
                  borderRadius: 4,
            }],
      } : null;

      return (
            <div className="yir-page">
                  <div className="yir-header">
                        <h1>📊 Year in Review</h1>
                        <select value={year} onChange={e => setYear(Number(e.target.value))}>
                              {[2026, 2025, 2024, 2023].map(y => <option key={y} value={y}>{y}</option>)}
                        </select>
                  </div>

                  {data ? (
                        <>
                              <div className="yir-stats">
                                    <div className="yir-stat"><span className="yir-num">{data.total_added}</span><span className="yir-label">Added</span></div>
                                    <div className="yir-stat"><span className="yir-num">{data.total_completed}</span><span className="yir-label">Completed</span></div>
                                    <div className="yir-stat"><span className="yir-num">{Math.round(data.total_hours_est)}</span><span className="yir-label">Hours Est.</span></div>
                              </div>

                              {chartData && (
                                    <div className="yir-chart">
                                          <h2>Monthly Activity</h2>
                                          <Bar data={chartData} options={{ responsive: true, plugins: { legend: { display: false } }, scales: { y: { beginAtZero: true, ticks: { color: '#888' } }, x: { ticks: { color: '#888' } } } }} />
                                    </div>
                              )}

                              {data.top_brands.length > 0 && (
                                    <div className="yir-brands">
                                          <h2>Top Brands</h2>
                                          {data.top_brands.map((b, i) => (
                                                <div key={b.name} className="yir-brand-row">
                                                      <span className="yir-rank">#{i + 1}</span>
                                                      <span className="yir-brand-name">{b.name}</span>
                                                      <span className="yir-brand-count">{b.count} works</span>
                                                </div>
                                          ))}
                                    </div>
                              )}
                        </>
                  ) : (
                        <div className="yir-empty">No data for {year}</div>
                  )}
            </div>
      );
}
