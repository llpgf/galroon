/**
 * Stats Charts Component
 * 
 * Collection of visualization cards for library statistics.
 * Uses CSS/SVG for lightweight charts without heavy dependencies.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';

interface StatsChartsProps {
      data?: {
            genres: { name: string; count: number; color: string }[];
            yearlyTrend: { year: number; count: number }[];
            topTags: { name: string; count: number }[];
            ratings: { rating: number; count: number }[];
      };
}

// Mock data
const mockData = {
      genres: [
            { name: 'Drama', count: 45, color: '#6366f1' },
            { name: 'Romance', count: 38, color: '#ec4899' },
            { name: 'Mystery', count: 28, color: '#8b5cf6' },
            { name: 'Sci-Fi', count: 22, color: '#06b6d4' },
            { name: 'Fantasy', count: 18, color: '#f59e0b' },
            { name: 'Other', count: 15, color: '#6b7280' }
      ],
      yearlyTrend: [
            { year: 2019, count: 12 },
            { year: 2020, count: 28 },
            { year: 2021, count: 45 },
            { year: 2022, count: 38 },
            { year: 2023, count: 52 },
            { year: 2024, count: 41 }
      ],
      topTags: [
            { name: '劇情向', count: 89 },
            { name: '純愛', count: 67 },
            { name: '科幻', count: 45 },
            { name: '校園', count: 42 },
            { name: '奇幻', count: 38 },
            { name: '推理', count: 31 }
      ],
      ratings: [
            { rating: 5, count: 28 },
            { rating: 4, count: 45 },
            { rating: 3, count: 32 },
            { rating: 2, count: 12 },
            { rating: 1, count: 5 }
      ]
};

function StatsCharts({ data = mockData }: StatsChartsProps) {
      const { t } = useTranslation();
      const total = data.genres.reduce((sum, g) => sum + g.count, 0);
      const maxYearCount = Math.max(...data.yearlyTrend.map(y => y.count));
      const maxTagCount = Math.max(...data.topTags.map(t => t.count));

      return (
            <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 p-4">

                  {/* Genre Distribution - Donut Chart */}
                  <div className="bg-white/5 backdrop-blur-sm border border-medium rounded-2xl p-5">
                        <h3 className="text-white/60 text-xs uppercase tracking-wider mb-4">
                              {t('dashboard.genreDistribution')}
                        </h3>
                        <div className="relative w-32 h-32 mx-auto">
                              <svg viewBox="0 0 100 100" className="transform -rotate-90">
                                    {data.genres.reduce((acc, genre, i) => {
                                          const percentage = (genre.count / total) * 100;
                                          const offset = acc.offset;
                                          acc.elements.push(
                                                <circle
                                                      key={genre.name}
                                                      cx="50"
                                                      cy="50"
                                                      r="40"
                                                      fill="transparent"
                                                      stroke={genre.color}
                                                      strokeWidth="12"
                                                      strokeDasharray={`${percentage * 2.51} 251`}
                                                      strokeDashoffset={-offset * 2.51}
                                                      className="transition-all duration-700"
                                                />
                                          );
                                          acc.offset += percentage;
                                          return acc;
                                    }, { elements: [] as React.ReactNode[], offset: 0 }).elements}
                              </svg>
                              <div className="absolute inset-0 flex items-center justify-center">
                                    <span className="text-2xl font-bold text-white">{total}</span>
                              </div>
                        </div>
                        <div className="mt-4 space-y-1">
                              {data.genres.slice(0, 4).map(g => (
                                    <div key={g.name} className="flex items-center gap-2 text-xs">
                                          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: g.color }} />
                                          <span className="text-white/60">{g.name}</span>
                                          <span className="ml-auto text-white/40">{g.count}</span>
                                    </div>
                              ))}
                        </div>
                  </div>

                  {/* Yearly Trend - Bar Chart */}
                  <div className="bg-white/5 backdrop-blur-sm border border-medium rounded-2xl p-5">
                        <h3 className="text-white/60 text-xs uppercase tracking-wider mb-4">
                              {t('dashboard.yearlyTrend')}
                        </h3>
                        <div className="flex items-end justify-between h-32 gap-2">
                              {data.yearlyTrend.map(item => (
                                    <div key={item.year} className="flex-1 flex flex-col items-center gap-1">
                                          <div
                                                className="w-full bg-gradient-to-t from-[#6366f1] to-[#8b5cf6] rounded-t transition-all duration-500"
                                                style={{ height: `${(item.count / maxYearCount) * 100}%` }}
                                          />
                                          <span className="text-[10px] text-white/40">{item.year.toString().slice(-2)}</span>
                                    </div>
                              ))}
                        </div>
                  </div>

                  {/* Top Tags - Horizontal Bars */}
                  <div className="bg-white/5 backdrop-blur-sm border border-medium rounded-2xl p-5">
                        <h3 className="text-white/60 text-xs uppercase tracking-wider mb-4">
                              {t('dashboard.topTags')}
                        </h3>
                        <div className="space-y-3">
                              {data.topTags.slice(0, 5).map((tag, i) => (
                                    <div key={tag.name} className="space-y-1">
                                          <div className="flex items-center justify-between text-xs">
                                                <span className="text-white/70">{tag.name}</span>
                                                <span className="text-white/40">{tag.count}</span>
                                          </div>
                                          <div className="h-1.5 bg-white/5 rounded-full overflow-hidden">
                                                <div
                                                      className="h-full bg-gradient-to-r from-[#6366f1] to-[#ec4899] rounded-full transition-all duration-700"
                                                      style={{ width: `${(tag.count / maxTagCount) * 100}%` }}
                                                />
                                          </div>
                                    </div>
                              ))}
                        </div>
                  </div>

                  {/* Rating Distribution */}
                  <div className="bg-white/5 backdrop-blur-sm border border-medium rounded-2xl p-5">
                        <h3 className="text-white/60 text-xs uppercase tracking-wider mb-4">
                              {t('dashboard.ratings')}
                        </h3>
                        <div className="space-y-2">
                              {data.ratings.map(r => (
                                    <div key={r.rating} className="flex items-center gap-3">
                                          <div className="flex gap-0.5">
                                                {Array.from({ length: 5 }).map((_, i) => (
                                                      <div
                                                            key={i}
                                                            className={`w-3 h-3 rounded-sm ${i < r.rating ? 'bg-amber-400' : 'bg-white/10'}`}
                                                      />
                                                ))}
                                          </div>
                                          <div className="flex-1 h-2 bg-white/5 rounded-full overflow-hidden">
                                                <div
                                                      className="h-full bg-amber-400/80 rounded-full transition-all duration-500"
                                                      style={{ width: `${(r.count / Math.max(...data.ratings.map(x => x.count))) * 100}%` }}
                                                />
                                          </div>
                                          <span className="text-xs text-white/40 w-8 text-right">{r.count}</span>
                                    </div>
                              ))}
                        </div>
                  </div>
            </div>
      );
}

export default StatsCharts;
