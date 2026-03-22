import { useTranslation } from 'react-i18next';
import type { SelectedSource } from '../types';

type ViewMode = 'grid' | 'list' | 'details' | 'links';

interface HeaderProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  onScan: () => void;
  gameCount: number;
  isSelectionMode?: boolean;
  onToggleSelectionMode?: () => void;
  selectedSource?: SelectedSource | null;
}

export default function Header({
  searchQuery,
  onSearchChange,
  viewMode,
  onViewModeChange,
  onScan,
  gameCount,
  isSelectionMode,
  onToggleSelectionMode,
  selectedSource,
}: HeaderProps) {
  const { t } = useTranslation();

  const getGamesWord = (count: number): string => {
    const lastTwo = count % 100;
    const lastOne = count % 10;
    if (lastTwo >= 11 && lastTwo <= 19) return 'игр';
    if (lastOne === 1) return 'игра';
    if (lastOne >= 2 && lastOne <= 4) return 'игры';
    return 'игр';
  };

  return (
    <header className="h-14 bg-surface-300 border-b border-surface-100 flex items-center justify-between px-6">
      <div className="flex items-center gap-4">
        <div className="relative">
          <svg
            className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            type="text"
            placeholder={t('common.search')}
            value={searchQuery}
            onChange={e => onSearchChange(e.target.value)}
            className="w-64 pl-10 pr-4 py-2 bg-surface-200 rounded-lg text-sm placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-accent"
          />
        </div>
        <span className="text-sm text-gray-500">{gameCount} {getGamesWord(gameCount)}</span>
      </div>

      <div className="flex items-center gap-2">
        {onToggleSelectionMode && (
          <button 
            onClick={onToggleSelectionMode} 
            className={`btn btn-sm ${isSelectionMode ? 'btn-accent' : 'btn-secondary text-sm'}`}
            title="Toggle Selection Mode"
          >
            {isSelectionMode ? '✓' : '☐'}
          </button>
        )}
        
        {/* Scan folder button - only shown when no source is selected */}
        {!selectedSource && (
          <button
            onClick={onScan}
            className="btn btn-primary text-sm"
            title={t('actions.scanFolder')}
          >
            {t('actions.scanFolder')}
          </button>
        )}

        {/* View mode toggle - 3 buttons */}
        <div className="flex bg-surface-200 rounded-lg p-1">
          {/* Details view */}
          <button
            onClick={() => onViewModeChange('details')}
            className={`p-2 rounded ${viewMode === 'details' ? 'bg-surface-100 text-white' : 'text-gray-500 hover:text-white'}`}
            title="Details"
          >
            <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
              <path d="M3 4a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1H4a1 1 0 01-1-1V4zm0 8a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1H4a1 1 0 01-1-1v-4zm8-8a1 1 0 011-1h4a1 1 0 011 1v12a1 1 0 01-1 1h-4a1 1 0 01-1-1V4z" />
            </svg>
          </button>

          {/* Grid view */}
          <button
            onClick={() => onViewModeChange('grid')}
            className={`p-2 rounded ${viewMode === 'grid' ? 'bg-surface-100 text-white' : 'text-gray-500 hover:text-white'}`}
            title="Grid"
          >
            <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
              <path d="M5 3a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2V5a2 2 0 00-2-2H5zM5 11a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2v-2a2 2 0 00-2-2H5zM11 5a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V5zM11 13a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
            </svg>
          </button>

          {/* List view */}
          <button
            onClick={() => onViewModeChange('list')}
            className={`p-2 rounded ${viewMode === 'list' ? 'bg-surface-100 text-white' : 'text-gray-500 hover:text-white'}`}
            title="List"
          >
            <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1z" clipRule="evenodd" />
            </svg>
          </button>
        </div>
      </div>
    </header>
  );
}
