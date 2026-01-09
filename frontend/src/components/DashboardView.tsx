/**
 * Dashboard View
 * 
 * New layout:
 * - Top 40%: Matrix Star Map with centered search overlay
 * - Bottom 60%: Stats charts grid
 */

import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Search, X } from 'lucide-react';
import DiscoveryLens from './dashboard/DiscoveryLens';
import StatsCharts from './dashboard/StatsCharts';

interface DashboardViewProps {
      onNavigateToGallery?: (filter?: string) => void;
}

export function DashboardView({ onNavigateToGallery }: DashboardViewProps) {
      const { t } = useTranslation();
      const [searchQuery, setSearchQuery] = useState('');

      const handleSearch = useCallback((query: string) => {
            setSearchQuery(query);
            // In real implementation, this would filter the star map nodes
      }, []);

      return (
            <div className="relative w-full h-full overflow-hidden bg-[#0a0a0a] flex flex-col">

                  {/* Top Section: Star Map (40vh) */}
                  <div className="relative" style={{ height: '40vh', minHeight: '300px' }}>

                        {/* Matrix Star Map Background */}
                        <div className="absolute inset-0">
                              <DiscoveryLens
                                    onNodeClick={(node) => onNavigateToGallery?.(node.id)}
                              />
                        </div>

                        {/* Centered Search Overlay */}
                        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
                              <div className="pointer-events-auto w-full max-w-lg px-8">
                                    <div className="relative group">
                                          <Search
                                                className="absolute left-5 top-1/2 -translate-y-1/2 w-5 h-5 text-white/30 group-focus-within:text-white/60 transition-colors"
                                                strokeWidth={1.5}
                                          />
                                          <input
                                                type="text"
                                                value={searchQuery}
                                                onChange={(e) => handleSearch(e.target.value)}
                                                placeholder={t('dashboard.searchStarMap')}
                                                className="w-full bg-black/40 backdrop-blur-xl border border-white/10 rounded-2xl pl-14 pr-12 py-4 text-white text-lg placeholder:text-white/30 focus:outline-none focus:border-white/30 focus:bg-black/60 transition-all shadow-2xl shadow-black/50"
                                          />
                                          {searchQuery && (
                                                <button
                                                      onClick={() => handleSearch('')}
                                                      className="absolute right-4 top-1/2 -translate-y-1/2 p-2 text-white/30 hover:text-white/60 rounded-full hover:bg-white/10 transition-colors"
                                                >
                                                      <X className="w-4 h-4" strokeWidth={2} />
                                                </button>
                                          )}
                                    </div>

                                    {/* Search results hint */}
                                    {searchQuery && (
                                          <p className="text-center text-white/40 text-sm mt-3">
                                                {t('dashboard.searchingFor', { query: searchQuery })}
                                          </p>
                                    )}
                              </div>
                        </div>

                        {/* Bottom gradient fade */}
                        <div className="absolute bottom-0 left-0 right-0 h-16 bg-gradient-to-t from-[#0a0a0a] to-transparent pointer-events-none" />
                  </div>

                  {/* Bottom Section: Stats (60%) */}
                  <div className="flex-1 overflow-y-auto">
                        {/* Section Header */}
                        <div className="px-8 pt-6 pb-2">
                              <h2 className="text-white/80 text-lg font-medium">{t('dashboard.libraryStats')}</h2>
                              <p className="text-white/40 text-sm">{t('dashboard.statsDescription')}</p>
                        </div>

                        {/* Stats Charts Grid */}
                        <StatsCharts />

                        {/* Bottom padding */}
                        <div className="h-8" />
                  </div>
            </div>
      );
}
