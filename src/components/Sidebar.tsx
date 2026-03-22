import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Space } from '../types';
import clsx from 'clsx';
import SpaceSettingsDialog from './SpaceSettingsDialog';
import SpaceItem from './SpaceItem';

type FilterType = 'all' | 'favorites' | 'recent' | 'links';

interface SidebarProps {
  spaces: Space[];
  selectedSpaceId: string | null;
  selectedFilter: FilterType;
  selectedSourcePath: string | null;
  onSelectSpace: (id: string | null) => void;
  onSelectFilter: (filter: FilterType) => void;
  onSelectSource: (spaceId: string, path: string | null) => void;
  onAddSpace: () => void;
  onAddLink: () => void;
  onDeleteSpace?: (space: Space) => void;
  isLoading: boolean;
  favoritesCount: number;
  recentCount: number;
}

// SVG Icons
const AllGamesIcon = () => (
  <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
  </svg>
);

const StarIcon = () => (
  <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
    <path d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
  </svg>
);

const ClockIcon = () => (
  <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
  </svg>
);

const LinkIcon = () => (
  <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
  </svg>
);

const PlusIcon = () => (
  <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
  </svg>
);

const TrashIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
  </svg>
);

export default function Sidebar({
  spaces,
  selectedSpaceId,
  selectedFilter,
  selectedSourcePath,
  onSelectSpace,
  onSelectFilter,
  onSelectSource,
  onAddSpace,
  onAddLink,
  onDeleteSpace,
  isLoading,
  favoritesCount,
  recentCount,
}: SidebarProps) {
  const { t } = useTranslation();
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; space: Space } | null>(null);
  const [settingsSpace, setSettingsSpace] = useState<Space | null>(null);

  const handleFilterClick = (filter: FilterType) => {
    onSelectFilter(filter);
    onSelectSpace(null);
  };

  const handleSpaceSettings = (space: Space, e: React.MouseEvent) => {
    e.stopPropagation();
    setSettingsSpace(space);
  };

  const handleCloseSettings = () => {
    setSettingsSpace(null);
  };

  return (
    <>
      <aside className="flex-1 flex flex-col overflow-hidden">
        <div className="p-4 border-b border-surface-100">
          <h1 className="text-xl font-bold text-accent">GHub</h1>
          <p className="text-xs text-gray-500">Game Library</p>
        </div>

        <nav className="p-3 space-y-1">
          <button
            onClick={() => handleFilterClick('all')}
            className={clsx('sidebar-item w-full', selectedFilter === 'all' && !selectedSpaceId && 'active')}
          >
            <AllGamesIcon />
            <span>{t('sidebar.allGames')}</span>
          </button>
          
          <button
            onClick={() => handleFilterClick('favorites')}
            className={clsx('sidebar-item w-full', selectedFilter === 'favorites' && 'active')}
          >
            <span className="text-yellow-400"><StarIcon /></span>
            <span className="flex-1 text-left">{t('sidebar.favorites')}</span>
            {favoritesCount > 0 && (
              <span className="text-xs bg-yellow-500/20 text-yellow-400 px-1.5 py-0.5 rounded">{favoritesCount}</span>
            )}
          </button>
          
          <button
            onClick={() => handleFilterClick('recent')}
            className={clsx('sidebar-item w-full', selectedFilter === 'recent' && 'active')}
          >
            <ClockIcon />
            <span className="flex-1 text-left">{t('sidebar.recentlyPlayed')}</span>
            {recentCount > 0 && (
              <span className="text-xs bg-accent/20 text-accent px-1.5 py-0.5 rounded">{recentCount}</span>
            )}
          </button>

          <button
            onClick={() => handleFilterClick('links')}
            className={clsx('sidebar-item w-full', selectedFilter === 'links' && 'active')}
          >
            <LinkIcon />
            <span className="flex-1 text-left">{t('links.title')}</span>
          </button>
        </nav>

        <div className="flex-1 overflow-auto">
          <div className="px-4 py-2">
            <span className="text-xs font-semibold text-gray-500 uppercase tracking-wider">
              {t('sidebar.spaces')}
            </span>
          </div>

          {isLoading ? (
            <div className="px-4 py-2 text-gray-500 text-sm">{t('common.loading')}</div>
          ) : spaces.length === 0 ? (
            <div className="px-4 py-2 text-gray-500 text-sm">{t('sidebar.noSpaces')}</div>
          ) : (
            <nav className="px-3 space-y-1">
              {spaces.map(space => (
                <SpaceItem
                  key={space.id}
                  space={space}
                  isSelected={selectedSpaceId === space.id}
                  selectedSourcePath={selectedSourcePath}
                  onSelectSpace={onSelectSpace}
                  onSelectSource={onSelectSource}
                  onSettings={handleSpaceSettings}
                />
              ))}
            </nav>
          )}
        </div>

        <div className="p-3 border-t border-surface-100 flex flex-col gap-1">
          <button
            onClick={onAddLink}
            className="sidebar-item w-full justify-center text-accent hover:bg-accent/10"
          >
            <LinkIcon />
            <span>{t('sidebar.addLink')}</span>
          </button>
          <button
            onClick={onAddSpace}
            className="sidebar-item w-full justify-center text-accent hover:bg-accent/10"
          >
            <PlusIcon />
            <span>{t('sidebar.addSpace')}</span>
          </button>
        </div>
      </aside>

      {/* Space Settings Dialog */}
      {settingsSpace && (
        <SpaceSettingsDialog
          space={settingsSpace}
          onClose={handleCloseSettings}
        />
      )}

      {/* Context Menu */}
      {contextMenu && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setContextMenu(null)} />
          <div
            className="fixed z-50 bg-surface-400 rounded-lg shadow-lg py-1 min-w-[150px] border border-surface-100"
            style={{ top: contextMenu.y, left: contextMenu.x }}
          >
            <button
              onClick={() => { onDeleteSpace?.(contextMenu.space); setContextMenu(null); }}
              className="flex items-center gap-3 w-full px-4 py-2 text-sm text-left text-red-400 hover:bg-red-500/20"
            >
              <TrashIcon />
              <span>{t('actions.delete')}</span>
            </button>
          </div>
        </>
      )}
    </>
  );
}