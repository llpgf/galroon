/**
 * WorkshopFocusBar - Filtering and search using shadcn/ui
 * 
 * Design System:
 * - bg-neutral-950 (background)
 * - bg-neutral-900 (panel)
 * - border-neutral-800
 * - purple-500 (accent)
 * - rounded-xl
 */

import { useState, useRef, useEffect } from 'react';
import { ChevronDown, Search, X, Plus, Minus, Tag as TagIcon } from 'lucide-react';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Badge } from '../ui/badge';
import { Popover, PopoverContent, PopoverTrigger } from '../ui/popover';
import { WorkshopItem } from './PosterCard';

export interface FocusTag {
      name: string;
      state: 'include' | 'exclude';
}

interface WorkshopFocusBarProps {
      tags: FocusTag[];
      onAddTag: (name: string) => void;
      onRemoveTag: (name: string) => void;
      onToggleTagState: (name: string) => void;
      searchQuery: string;
      onSearchChange: (query: string) => void;
      searchResults: WorkshopItem[];
      onSearchResultClick: (item: WorkshopItem) => void;
      recentSearches: string[];
      availableTags: string[];
}

export function WorkshopFocusBar({
      tags,
      onAddTag,
      onRemoveTag,
      onToggleTagState,
      searchQuery,
      onSearchChange,
      searchResults,
      onSearchResultClick,
      recentSearches,
      availableTags,
}: WorkshopFocusBarProps) {
      const [isSearchExpanded, setIsSearchExpanded] = useState(false);
      const [isTagPopoverOpen, setIsTagPopoverOpen] = useState(false);
      const searchInputRef = useRef<HTMLInputElement>(null);

      useEffect(() => {
            if (isSearchExpanded && searchInputRef.current) {
                  searchInputRef.current.focus();
            }
      }, [isSearchExpanded]);

      const unusedTags = availableTags.filter(t => !tags.find(ft => ft.name === t));

      return (
            <div className="px-6 py-3 bg-neutral-950 border-b border-neutral-800">
                  <div className="flex items-center gap-3 flex-wrap">
                        {/* Focus Dropdown */}
                        <Popover open={isTagPopoverOpen} onOpenChange={setIsTagPopoverOpen}>
                              <PopoverTrigger asChild>
                                    <Button
                                          variant="outline"
                                          size="sm"
                                          className="bg-neutral-900 border-neutral-800 text-neutral-300 hover:bg-neutral-800 hover:text-white rounded-lg"
                                    >
                                          <TagIcon className="w-4 h-4 mr-2" strokeWidth={1.5} />
                                          聚焦
                                          <ChevronDown className="w-3 h-3 ml-2" strokeWidth={2} />
                                    </Button>
                              </PopoverTrigger>
                              <PopoverContent className="w-48 p-2 bg-neutral-900 border-neutral-800 rounded-xl">
                                    {unusedTags.length > 0 ? (
                                          unusedTags.slice(0, 10).map((tag) => (
                                                <Button
                                                      key={tag}
                                                      variant="ghost"
                                                      size="sm"
                                                      onClick={() => {
                                                            onAddTag(tag);
                                                            setIsTagPopoverOpen(false);
                                                      }}
                                                      className="w-full justify-start text-neutral-300 hover:text-white hover:bg-neutral-800"
                                                >
                                                      {tag}
                                                </Button>
                                          ))
                                    ) : (
                                          <p className="text-neutral-500 text-sm p-2">沒有更多標籤</p>
                                    )}
                              </PopoverContent>
                        </Popover>

                        {/* Active Tags */}
                        {tags.map((tag) => (
                              <Badge
                                    key={tag.name}
                                    variant="outline"
                                    className={`
              flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm
              ${tag.state === 'include'
                                                ? 'bg-purple-500 text-white border-purple-500'
                                                : 'bg-red-500 text-white border-red-500'
                                          }
            `}
                              >
                                    <button
                                          onClick={() => onToggleTagState(tag.name)}
                                          className="hover:opacity-80 transition-opacity"
                                    >
                                          {tag.state === 'include' ? (
                                                <Plus className="w-3 h-3" strokeWidth={3} />
                                          ) : (
                                                <Minus className="w-3 h-3" strokeWidth={3} />
                                          )}
                                    </button>
                                    <span>{tag.name}</span>
                                    <button
                                          onClick={() => onRemoveTag(tag.name)}
                                          className="hover:opacity-80 transition-opacity"
                                    >
                                          <X className="w-3 h-3" strokeWidth={3} />
                                    </button>
                              </Badge>
                        ))}

                        {/* Spacer */}
                        <div className="flex-1" />

                        {/* Search */}
                        <Popover>
                              {isSearchExpanded ? (
                                    <div className="flex items-center gap-2 bg-neutral-900 rounded-xl px-4 py-2 border border-neutral-800">
                                          <Search className="w-4 h-4 text-neutral-500" strokeWidth={1.5} />
                                          <Input
                                                ref={searchInputRef}
                                                type="text"
                                                value={searchQuery}
                                                onChange={(e) => onSearchChange(e.target.value)}
                                                placeholder="搜尋作品..."
                                                className="bg-transparent border-0 text-white text-sm placeholder-neutral-500 w-48 h-auto p-0 focus-visible:ring-0"
                                          />
                                          <Button
                                                variant="ghost"
                                                size="icon"
                                                onClick={() => {
                                                      onSearchChange('');
                                                      setIsSearchExpanded(false);
                                                }}
                                                className="h-6 w-6 text-neutral-500 hover:text-white"
                                          >
                                                <X className="w-4 h-4" strokeWidth={1.5} />
                                          </Button>
                                    </div>
                              ) : (
                                    <Button
                                          variant="outline"
                                          size="icon"
                                          onClick={() => setIsSearchExpanded(true)}
                                          className="bg-neutral-900 border-neutral-800 text-neutral-500 hover:text-white hover:bg-neutral-800 rounded-xl"
                                    >
                                          <Search className="w-4 h-4" strokeWidth={1.5} />
                                    </Button>
                              )}

                              {/* Search Results Dropdown */}
                              {isSearchExpanded && searchQuery && searchResults.length > 0 && (
                                    <PopoverContent className="w-72 p-2 bg-neutral-900 border-neutral-800 rounded-xl">
                                          {searchResults.slice(0, 8).map((item) => (
                                                <Button
                                                      key={item.id}
                                                      variant="ghost"
                                                      onClick={() => onSearchResultClick(item)}
                                                      className="w-full justify-start hover:bg-neutral-800 flex items-center gap-3 p-2"
                                                >
                                                      {item.coverImage && (
                                                            <img
                                                                  src={item.coverImage}
                                                                  alt=""
                                                                  className="w-8 h-10 object-cover rounded"
                                                            />
                                                      )}
                                                      <div className="min-w-0 flex-1 text-left">
                                                            <p className="text-sm text-white truncate">{item.title || 'Untitled'}</p>
                                                            <p className="text-xs text-neutral-500">
                                                                  {item.isInGallery ? 'Gallery' : 'Workshop'}
                                                            </p>
                                                      </div>
                                                </Button>
                                          ))}
                                    </PopoverContent>
                              )}
                        </Popover>
                  </div>

                  {/* Recent Searches */}
                  {recentSearches.length > 0 && !searchQuery && (
                        <div className="mt-2 flex items-center gap-2 text-xs">
                              <span className="text-neutral-600">最近搜尋：</span>
                              {recentSearches.slice(0, 5).map((term) => (
                                    <Button
                                          key={term}
                                          variant="link"
                                          size="sm"
                                          onClick={() => {
                                                onSearchChange(term);
                                                setIsSearchExpanded(true);
                                          }}
                                          className="text-neutral-500 hover:text-white p-0 h-auto"
                                    >
                                          {term}
                                    </Button>
                              ))}
                        </div>
                  )}
            </div>
      );
}
