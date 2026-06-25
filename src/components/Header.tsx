import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { SelectedSource, SortField, SortOrder } from '../types';

interface HeaderProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  onScan: () => void;
  gameCount: number;
  isSelectionMode?: boolean;
  onToggleSelectionMode?: () => void;
  selectedSource?: SelectedSource | null;
  sortBy: SortField;
  sortOrder: SortOrder;
  onSortChange: (field: SortField) => void;
}

export default function Header({
  searchQuery,
  onSearchChange,
  onScan,
  gameCount,
  isSelectionMode,
  onToggleSelectionMode,
  selectedSource,
  sortBy,
  sortOrder,
  onSortChange,
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

  const sortOptions: { field: SortField; label: string }[] = [
    { field: 'title', label: t('sort.title') },
    { field: 'last_played', label: t('sort.lastPlayed') },
    { field: 'playtime', label: t('sort.playtime') },
    { field: 'added_at', label: t('sort.addedOn') },
    { field: 'developer', label: t('sort.developer') },
  ];

  const currentSortLabel = sortOptions.find(opt => opt.field === sortBy)?.label || '';
  const sortDirection = sortOrder === 'asc' ? '↑' : '↓';

  const [isDropdownOpen, setIsDropdownOpen] = useState(false);

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
        
        {/* Sort dropdown */}
        <div className="relative">
          <button
            onClick={() => setIsDropdownOpen(!isDropdownOpen)}
            className="flex items-center gap-2 px-3 py-2 bg-surface-200 rounded-lg text-sm hover:bg-surface-100 transition-colors"
            title={t('common.sortBy')}
          >
            <svg className="w-4 h-4 text-gray-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 4h13M3 8h9m-9 4h5M4 4l5 5M4 9l5-5" />
            </svg>
            <span className="text-gray-700">{currentSortLabel}</span>
            <span className="text-gray-500">{sortDirection}</span>
          </button>
          
          {isDropdownOpen && (
            <>
              <div 
                className="fixed inset-0 z-10" 
                onClick={() => setIsDropdownOpen(false)}
              />
              <div className="absolute top-full left-0 mt-1 w-48 bg-surface-200 rounded-lg shadow-lg border border-surface-100 z-20">
                {sortOptions.map(option => (
                  <button
                    key={option.field}
                    onClick={() => {
                      onSortChange(option.field);
                      setIsDropdownOpen(false);
                    }}
                    className={`w-full px-4 py-2 text-left text-sm hover:bg-surface-100 flex items-center justify-between ${
                      sortBy === option.field ? 'text-accent font-medium' : 'text-gray-700'
                    }`}
                  >
                    <span>{option.label}</span>
                    {sortBy === option.field && (
                      <span className="text-accent">{sortDirection}</span>
                    )}
                  </button>
                ))}
              </div>
            </>
          )}
        </div>
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


      </div>
    </header>
  );
}
