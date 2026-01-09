import { Grid3x3, LayoutGrid, List, Rows3 } from 'lucide-react';

export type ViewMode = 'grid' | 'compact' | 'detail' | 'strip';

// View Mode Selector Component
interface ViewModeSelectorProps {
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
}

export function ViewModeSelector({ viewMode, onViewModeChange }: ViewModeSelectorProps) {
  const viewModes: { mode: ViewMode; icon: typeof Grid3x3; label: string }[] = [
    { mode: 'grid', icon: Grid3x3, label: 'Grid View' },
    { mode: 'compact', icon: LayoutGrid, label: 'Compact View' },
    { mode: 'detail', icon: List, label: 'Detail View' },
    { mode: 'strip', icon: Rows3, label: 'Strip View' }
  ];

  return (
    <div className="flex items-center gap-1 bg-[#161616] rounded-lg p-1">
      {viewModes.map(({ mode, icon: Icon, label }) => (
        <button
          key={mode}
          onClick={() => onViewModeChange(mode)}
          className={`p-2 rounded transition-all ${
            viewMode === mode
              ? 'bg-[#2a2a2a] text-white'
              : 'text-[#6b6b6b] hover:text-white hover:bg-[#1e1e1e]'
          }`}
          aria-label={label}
          title={label}
        >
          <Icon className="w-4 h-4" strokeWidth={1.5} />
        </button>
      ))}
    </div>
  );
}

// Viewing Distance Slider Component
interface ViewingDistanceSliderProps {
  viewingDistance: number; // 0-100
  onViewingDistanceChange: (distance: number) => void;
}

export function ViewingDistanceSlider({
  viewingDistance,
  onViewingDistanceChange
}: ViewingDistanceSliderProps) {
  return (
    <div className="flex items-center gap-3">
      <span className="text-xs text-[#6b6b6b] uppercase tracking-wider">Viewing Distance</span>
      <input
        type="range"
        min="0"
        max="100"
        value={viewingDistance}
        onChange={(e) => onViewingDistanceChange(Number(e.target.value))}
        className="w-32 h-1 bg-[#2a2a2a] rounded-full appearance-none cursor-pointer
          [&::-webkit-slider-thumb]:appearance-none
          [&::-webkit-slider-thumb]:w-3
          [&::-webkit-slider-thumb]:h-3
          [&::-webkit-slider-thumb]:rounded-full
          [&::-webkit-slider-thumb]:bg-white
          [&::-webkit-slider-thumb]:cursor-pointer
          [&::-webkit-slider-thumb]:transition-transform
          [&::-webkit-slider-thumb]:hover:scale-125"
      />
      <span className="text-xs text-[#6b6b6b] min-w-[2rem]">{viewingDistance}%</span>
    </div>
  );
}

// Legacy combined component (for backward compatibility)
interface DisplayGrammarControlsProps {
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  viewingDistance: number;
  onViewingDistanceChange: (distance: number) => void;
}

export function DisplayGrammarControls({
  viewMode,
  onViewModeChange,
  viewingDistance,
  onViewingDistanceChange
}: DisplayGrammarControlsProps) {
  return (
    <div className="flex items-center gap-6">
      <ViewingDistanceSlider
        viewingDistance={viewingDistance}
        onViewingDistanceChange={onViewingDistanceChange}
      />
      <ViewModeSelector
        viewMode={viewMode}
        onViewModeChange={onViewModeChange}
      />
    </div>
  );
}
