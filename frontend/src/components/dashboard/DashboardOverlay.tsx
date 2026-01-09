/**
 * Dashboard Overlay Component (Layer 1)
 * 
 * Glassmorphism cards floating above the Discovery Lens.
 * - Library Pulse (top-left)
 * - Recently Curated (bottom)
 * - Insights (right side)
 */

import { useTranslation } from 'react-i18next';
import { TrendingUp, Clock, Sparkles, Library, Heart } from 'lucide-react';
import { cn } from '../ui/utils';
import { ImageWithFallback } from '../figma/ImageWithFallback';

interface DashboardStats {
      totalGames: number;
      totalPlaytime: string;
      recentlyCurated: number;
}

interface RecentGame {
      id: string;
      title: string;
      coverImage?: string;
}

interface Insight {
      type: 'creator' | 'tag' | 'pattern';
      text: string;
}

interface DashboardOverlayProps {
      stats: DashboardStats;
      recentGames: RecentGame[];
      insights: Insight[];
      isVisible?: boolean;
      onGameClick?: (id: string) => void;
}

export function DashboardOverlay({
      stats,
      recentGames,
      insights,
      isVisible = true,
      onGameClick
}: DashboardOverlayProps) {
      const { t } = useTranslation();

      if (!isVisible) return null;

      return (
            <div className={cn(
                  "absolute inset-0 pointer-events-none transition-opacity duration-500",
                  isVisible ? "opacity-100" : "opacity-0"
            )}>
                  {/* Library Pulse - Top Left */}
                  <div className="absolute top-8 left-8 pointer-events-auto">
                        <div className="bg-[#1a1a1a]/70 backdrop-blur-xl border border-[#2a2a2a]/50 rounded-2xl p-6 space-y-4 w-64">
                              <div className="flex items-center gap-3">
                                    <Library className="w-5 h-5 text-[#6366f1]" />
                                    <h3 className="text-white text-sm font-medium">{t('dashboard.libraryPulse')}</h3>
                              </div>

                              <div className="space-y-3">
                                    <div className="flex items-center justify-between">
                                          <span className="text-[#6b6b6b] text-sm">{t('dashboard.totalGames')}</span>
                                          <span className="text-white text-lg font-semibold">{stats.totalGames}</span>
                                    </div>
                                    <div className="flex items-center justify-between">
                                          <span className="text-[#6b6b6b] text-sm">{t('dashboard.totalPlaytime')}</span>
                                          <span className="text-white text-lg font-semibold">{stats.totalPlaytime}</span>
                                    </div>
                                    <div className="flex items-center justify-between">
                                          <span className="text-[#6b6b6b] text-sm">{t('dashboard.recentlyCurated')}</span>
                                          <span className="text-[#6366f1] text-lg font-semibold">+{stats.recentlyCurated}</span>
                                    </div>
                              </div>
                        </div>
                  </div>

                  {/* Insights - Right Side */}
                  <div className="absolute top-8 right-8 pointer-events-auto">
                        <div className="bg-[#1a1a1a]/70 backdrop-blur-xl border border-[#2a2a2a]/50 rounded-2xl p-6 space-y-4 w-72">
                              <div className="flex items-center gap-3">
                                    <Sparkles className="w-5 h-5 text-amber-400" />
                                    <h3 className="text-white text-sm font-medium">{t('dashboard.insights')}</h3>
                              </div>

                              <div className="space-y-3">
                                    {insights.map((insight, index) => (
                                          <div
                                                key={index}
                                                className="flex items-start gap-3 p-3 bg-[#2a2a2a]/50 rounded-lg"
                                          >
                                                {insight.type === 'creator' && <Heart className="w-4 h-4 text-pink-400 mt-0.5 flex-shrink-0" />}
                                                {insight.type === 'tag' && <TrendingUp className="w-4 h-4 text-green-400 mt-0.5 flex-shrink-0" />}
                                                {insight.type === 'pattern' && <Clock className="w-4 h-4 text-blue-400 mt-0.5 flex-shrink-0" />}
                                                <p className="text-[#b3b3b3] text-sm leading-relaxed">{insight.text}</p>
                                          </div>
                                    ))}
                              </div>
                        </div>
                  </div>

                  {/* Recently Curated - Bottom */}
                  <div className="absolute bottom-8 left-8 right-8 pointer-events-auto">
                        <div className="bg-[#1a1a1a]/70 backdrop-blur-xl border border-[#2a2a2a]/50 rounded-2xl p-6">
                              <div className="flex items-center gap-3 mb-4">
                                    <Clock className="w-5 h-5 text-purple-400" />
                                    <h3 className="text-white text-sm font-medium">{t('dashboard.recentlyCuratedTitle')}</h3>
                              </div>

                              {/* Horizontal Scroll */}
                              <div className="flex gap-4 overflow-x-auto pb-2 scrollbar-hide">
                                    {recentGames.map((game) => (
                                          <button
                                                key={game.id}
                                                onClick={() => onGameClick?.(game.id)}
                                                className="group flex-shrink-0 w-24"
                                          >
                                                <div className="relative aspect-[3/4] rounded-lg overflow-hidden bg-[#2a2a2a] mb-2 ring-2 ring-purple-500/30 group-hover:ring-purple-500/60 transition-all">
                                                      {game.coverImage ? (
                                                            <ImageWithFallback
                                                                  src={game.coverImage}
                                                                  alt={game.title}
                                                                  className="w-full h-full object-cover group-hover:scale-110 transition-transform duration-300"
                                                            />
                                                      ) : (
                                                            <div className="w-full h-full flex items-center justify-center">
                                                                  <span className="text-2xl text-[#4a4a4a]">{game.title.charAt(0)}</span>
                                                            </div>
                                                      )}

                                                      {/* Purple glow effect */}
                                                      <div className="absolute inset-0 bg-gradient-to-t from-purple-500/20 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
                                                </div>
                                                <p className="text-white text-xs text-center truncate">{game.title}</p>
                                          </button>
                                    ))}
                              </div>
                        </div>
                  </div>
            </div>
      );
}
