import { GameCardData } from '../types/GameCard';
import { ImageWithFallback } from './figma/ImageWithFallback';
import { FolderOpen, Layers } from 'lucide-react';

interface GameCardProps {
  data: GameCardData;
  featured?: boolean;
}

export function GameCard({ data, featured = false }: GameCardProps) {
  const { entry_type, display_title, cover_image, instance_count } = data;

  // Canonical: Big cover + bold title
  if (entry_type === 'canonical') {
    return (
      <div className={`group cursor-pointer ${featured ? 'col-span-2 row-span-2' : ''}`}>
        <div className={`relative overflow-hidden bg-[var(--color-surface-strong)] rounded ${featured ? 'aspect-square' : 'aspect-square'}`}>
          {cover_image && (
            <ImageWithFallback 
              src={cover_image} 
              alt={display_title}
              className="w-full h-full object-cover transition-transform duration-500 group-hover:scale-105"
            />
          )}
        </div>
        <div className={`px-1 ${featured ? 'mt-4' : 'mt-3'}`}>
          <h3 className={`text-[var(--color-text-strong)] ${featured ? 'text-lg' : 'text-sm'}`}>{display_title}</h3>
          {instance_count !== undefined && (
            <p className={`text-[var(--color-text-tertiary)] ${featured ? 'mt-1' : 'mt-0.5'}`}>
              <small>{instance_count} {instance_count === 1 ? 'Version' : 'Versions'}</small>
            </p>
          )}
        </div>
      </div>
    );
  }

  // Suggested: Stacked icon + accent color title
  if (entry_type === 'suggested') {
    return (
      <div className="group cursor-pointer">
        <div className="relative aspect-square bg-[var(--color-surface-muted)] border-2 border-dashed border-[var(--color-border-strong)] p-4 flex items-center justify-center rounded">
          {/* Stacked layers visual */}
          <div className="relative w-full h-full flex items-center justify-center">
            <Layers className="w-20 h-20 text-[var(--color-accent-blue)] opacity-30" strokeWidth={1} />
            
            {/* Count badge */}
            {instance_count !== undefined && (
              <div className="absolute top-2 right-2 bg-[var(--color-surface-hover)] border border-[var(--color-border-strong)] px-3 py-1 rotate-3">
                <code className="text-[var(--color-accent-blue)]">{instance_count}</code>
              </div>
            )}
          </div>
        </div>
        
        <div className="mt-3 px-1">
          <h3 className="text-[var(--color-accent-blue)] text-sm">{display_title}</h3>
        </div>
      </div>
    );
  }

  // Orphan: Folder icon + monospace title
  if (entry_type === 'orphan') {
    return (
      <div className="group cursor-pointer">
        <div className="relative aspect-square flex flex-col items-center justify-center bg-[var(--color-surface-muted)] border border-[var(--color-border-medium)] rounded p-6 transition-colors duration-300 hover:bg-[var(--color-surface-strong)] hover:border-[var(--color-border-strong)]">
          <FolderOpen 
            className="w-12 h-12 text-[var(--color-text-tertiary)] mb-4 transition-colors duration-300 group-hover:text-[var(--color-text-secondary)]" 
            strokeWidth={1}
          />
          
          <div className="text-center w-full">
            <code className="text-[var(--color-text-tertiary)] text-xs block break-all text-center line-clamp-3">
              {display_title}
            </code>
          </div>
          
          {instance_count !== undefined && (
            <div className="absolute bottom-4 left-1/2 -translate-x-1/2">
              <small className="text-[var(--color-text-tertiary)] text-xs">{instance_count} items</small>
            </div>
          )}
        </div>
        
        <div className="mt-3 px-1">
          <h3 className="text-[var(--color-text-tertiary)] text-sm">Raw Data</h3>
        </div>
      </div>
    );
  }

  return null;
}
