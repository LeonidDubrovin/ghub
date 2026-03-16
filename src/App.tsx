import { useState, useEffect, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import Sidebar from './components/Sidebar';
import GameGrid from './components/GameGrid';
import GameDetailsView from './components/GameDetailsView';
import Header from './components/Header';
import AddSpaceDialog from './components/AddSpaceDialog';
import AddLinkDialog from './components/AddLinkDialog';
import ScanDialog from './components/ScanDialog';
import EditGameDialog from './components/EditGameDialog';
import ContextMenu, { ContextMenuItem } from './components/ContextMenu';
import ResizeHandle from './components/ResizeHandle';
import { useSpaces, useDeleteSpace } from './hooks/useSpaces';
import { useGames, useDeleteGame } from './hooks/useGames';
import type { Game, Space } from './types';

import DownloadLinksView from './components/DownloadLinksView';
import BatchMetadataDialog from './components/BatchMetadataDialog';

type ViewMode = 'grid' | 'list' | 'details' | 'links';
type FilterType = 'all' | 'favorites' | 'recent' | 'links';

const SIDEBAR_MIN = 180;
const SIDEBAR_MAX = 400;
const GAME_LIST_MIN = 200;
const GAME_LIST_MAX = 500;

function App() {
  const { t } = useTranslation();
  const [selectedSpaceId, setSelectedSpaceId] = useState<string | null>(() => localStorage.getItem('selectedSpaceId') || null);
  const [selectedFilter, setSelectedFilter] = useState<FilterType>(() => (localStorage.getItem('selectedFilter') as FilterType) || 'all');
  const [searchQuery, setSearchQuery] = useState('');
  const [viewMode, setViewMode] = useState<ViewMode>(() => (localStorage.getItem('viewMode') as ViewMode) || 'details');
  const [showAddSpace, setShowAddSpace] = useState(false);
  const [showAddLink, setShowAddLink] = useState(false);
  const [showScan, setShowScan] = useState(false);
  const [editingGame, setEditingGame] = useState<Game | null>(null);
  const [selectedGameForDetails, setSelectedGameForDetails] = useState<Game | null>(null);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; game: Game } | null>(null);
  const [spaceToDelete, setSpaceToDelete] = useState<Space | null>(null);
  const [runningGames, setRunningGames] = useState<Set<string>>(new Set());
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [updatingGameIds, setUpdatingGameIds] = useState<Set<string>>(new Set());
  
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedGameIds, setSelectedGameIds] = useState<Set<string>>(new Set());
  const [showBatchMetadata, setShowBatchMetadata] = useState(false);

  const [sidebarWidth, setSidebarWidth] = useState(240);
  const [gameListWidth, setGameListWidth] = useState(280);

  const { data: spaces = [], isLoading: spacesLoading } = useSpaces();
  const { data: games = [], isLoading: gamesLoading, refetch: refetchGames } = useGames(selectedSpaceId);
  const deleteSpaceMutation = useDeleteSpace();
  const deleteGameMutation = useDeleteGame();

  useEffect(() => {
    if (selectedSpaceId) localStorage.setItem('selectedSpaceId', selectedSpaceId);
    else localStorage.removeItem('selectedSpaceId');
  }, [selectedSpaceId]);

  useEffect(() => {
    localStorage.setItem('selectedFilter', selectedFilter);
  }, [selectedFilter]);

  useEffect(() => {
    localStorage.setItem('viewMode', viewMode);
  }, [viewMode]);

  useEffect(() => {
    const handleContextMenu = (e: MouseEvent) => { e.preventDefault(); };
    document.addEventListener('contextmenu', handleContextMenu);
    return () => document.removeEventListener('contextmenu', handleContextMenu);
  }, []);

  useEffect(() => {
    if (selectedGameForDetails) {
      const updatedGame = games.find(g => g.id === selectedGameForDetails.id);
      if (updatedGame && updatedGame !== selectedGameForDetails) {
        setSelectedGameForDetails(updatedGame);
      }
    }
  }, [games, selectedGameForDetails]);

  useEffect(() => {
    const checkActiveSessions = async () => {
      try {
        const sessions = await invoke<[string, string, number][]>('get_active_sessions');
        setRunningGames(new Set(sessions.map(([, gameId]) => gameId)));
      } catch (err) {
        console.error('Failed to get active sessions:', err);
      }
    };
    checkActiveSessions();
    const interval = setInterval(checkActiveSessions, 2000);
    return () => clearInterval(interval);
  }, []);

  const filteredGames = useMemo(() => {
    let result = games;
    if (searchQuery) {
      result = result.filter(game =>
        game.title.toLowerCase().includes(searchQuery.toLowerCase())
      );
    }
    if (selectedFilter === 'favorites') {
      result = result.filter(game => game.is_favorite);
    } else if (selectedFilter === 'recent') {
      const weekAgo = new Date();
      weekAgo.setDate(weekAgo.getDate() - 7);
      result = result
        .filter(game => game.last_played_at && new Date(game.last_played_at) > weekAgo)
        .sort((a, b) => {
          const dateA = a.last_played_at ? new Date(a.last_played_at).getTime() : 0;
          const dateB = b.last_played_at ? new Date(b.last_played_at).getTime() : 0;
          return dateB - dateA;
        });
    }
    return result;
  }, [games, searchQuery, selectedFilter]);

  const favoritesCount = useMemo(() => games.filter(g => g.is_favorite).length, [games]);
  const recentCount = useMemo(() => {
    const weekAgo = new Date();
    weekAgo.setDate(weekAgo.getDate() - 7);
    return games.filter(g => g.last_played_at && new Date(g.last_played_at) > weekAgo).length;
  }, [games]);

  const handleToggleSelection = (gameId: string) => {
    setSelectedGameIds(prev => {
      const next = new Set(prev);
      if (next.has(gameId)) next.delete(gameId);
      else next.add(gameId);
      return next;
    });
  };

  const handleSelectAll = () => {
    if (selectedGameIds.size === filteredGames.length) {
      setSelectedGameIds(new Set());
    } else {
      setSelectedGameIds(new Set(filteredGames.map(g => g.id)));
    }
  };

  const handleBatchUpdate = () => {
    setShowBatchMetadata(true);
  };

  const handleSidebarResize = useCallback((delta: number) => {
    setSidebarWidth(w => Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, w + delta)));
  }, []);

  const handleGameListResize = useCallback((delta: number) => {
    setGameListWidth(w => Math.min(GAME_LIST_MAX, Math.max(GAME_LIST_MIN, w + delta)));
  }, []);

  const handleSelectFilter = (filter: FilterType) => {
    setSelectedFilter(filter);
    if (filter === 'links') {
      setViewMode('links');
    } else if (viewMode === 'links') {
      setViewMode('details');
    }
  };

  const handleEditGame = (game: Game) => setEditingGame(game);

  const handlePlayGame = async (game: Game) => {
    if (runningGames.has(game.id)) {
      setLaunchError(t('errors.gameAlreadyRunning', { title: game.title }));
      setTimeout(() => setLaunchError(null), 3000);
      return;
    }
    const spaceId = selectedSpaceId || spaces[0]?.id;
    if (!spaceId) return;
    try {
      setRunningGames(prev => new Set([...prev, game.id]));
      await invoke('launch_game', { gameId: game.id, spaceId });
    } catch (err) {
      setRunningGames(prev => {
        const next = new Set(prev);
        next.delete(game.id);
        return next;
      });
      setLaunchError(String(err));
      setTimeout(() => setLaunchError(null), 5000);
    }
  };

  const handleDeleteGame = async (game: Game) => {
    try {
      await deleteGameMutation.mutateAsync(game.id);
      refetchGames();
    } catch (err) {
      console.error(err);
    }
  };

  const handleToggleFavorite = async (game: Game) => {
    try {
      await invoke('update_game', { request: { id: game.id, is_favorite: !game.is_favorite } });
      refetchGames();
    } catch (err) {
      console.error(err);
    }
  };

  const handleFetchMetadata = (game: Game) => {
    setEditingGame(game);
  };

  const handleDeleteSpace = (space: Space) => setSpaceToDelete(space);

  const confirmDeleteSpace = async () => {
    if (!spaceToDelete) return;
    try {
      await deleteSpaceMutation.mutateAsync(spaceToDelete.id);
      if (selectedSpaceId === spaceToDelete.id) setSelectedSpaceId(null);
      setSpaceToDelete(null);
    } catch (err) {
      console.error(err);
    }
  };

  const handleGameSaved = () => refetchGames();
  const isGameRunning = (gameId: string) => runningGames.has(gameId);

  const handleRefreshMetadata = async (game: Game) => {
    if (updatingGameIds.has(game.id)) return;
    
    setUpdatingGameIds(prev => new Set([...prev, game.id]));
    try {
      await invoke('refresh_game_from_local', { gameId: game.id });
      refetchGames();
    } catch (err) {
      console.error('Failed to refresh metadata:', err);
    } finally {
      setUpdatingGameIds(prev => {
        const next = new Set(prev);
        next.delete(game.id);
        return next;
      });
    }
  };

  const handleGameContextMenu = (e: React.MouseEvent, game: Game) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, game });
  };

  const getGameContextMenuItems = (game: Game): ContextMenuItem[] => [
    { label: t('actions.play'), icon: '▶', onClick: () => handlePlayGame(game), disabled: isGameRunning(game.id) },
    { separator: true, label: '', onClick: () => {} },
    { label: t('actions.editGame'), icon: '✏️', onClick: () => handleEditGame(game) },
    { label: t('actions.fetchMetadata'), icon: '🔍', onClick: () => handleFetchMetadata(game) },
    { label: t('actions.refreshFromLocal'), icon: '🔄', onClick: () => handleRefreshMetadata(game) },
    { separator: true, label: '', onClick: () => {} },
    { label: isSelectionMode ? 'Exit Selection' : 'Select Games', icon: '☑️', onClick: () => setIsSelectionMode(!isSelectionMode) },
    { separator: true, label: '', onClick: () => {} },
    {
      label: game.is_favorite ? t('actions.removeFromFavorites') : t('actions.addToFavorites'),
      icon: game.is_favorite ? '⭐' : '☆',
      onClick: () => handleToggleFavorite(game),
    },
    { separator: true, label: '', onClick: () => {} },
    { label: t('actions.delete'), icon: '🗑️', onClick: () => handleDeleteGame(game), danger: true },
  ];

  return (
    <div className="flex h-screen overflow-hidden">
      <aside 
        className="bg-surface-400 border-r border-surface-100 flex flex-col flex-shrink-0"
        style={{ width: sidebarWidth }}
      >
        <Sidebar
          spaces={spaces}
          selectedSpaceId={selectedSpaceId}
          selectedFilter={selectedFilter}
          onSelectSpace={setSelectedSpaceId}
          onSelectFilter={handleSelectFilter}
          onAddSpace={() => setShowAddSpace(true)}
          onAddLink={() => setShowAddLink(true)}
          onDeleteSpace={handleDeleteSpace}
          isLoading={spacesLoading}
          favoritesCount={favoritesCount}
          recentCount={recentCount}
        />
      </aside>
      
      <ResizeHandle onResize={handleSidebarResize} />

      <main className="flex-1 flex flex-col overflow-hidden">
        <Header
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          viewMode={viewMode}
          onViewModeChange={setViewMode}
          onScan={() => setShowScan(true)}
          gameCount={filteredGames.length}
          isSelectionMode={isSelectionMode}
          onToggleSelectionMode={() => setIsSelectionMode(!isSelectionMode)}
        />

        {isSelectionMode && (
          <div className="bg-surface-300 p-2 flex items-center justify-between border-b border-surface-100 px-6">
            <div className="flex items-center gap-4">
              <span className="text-sm font-medium">{selectedGameIds.size} selected</span>
              <button onClick={handleSelectAll} className="text-xs text-accent hover:underline">
                {selectedGameIds.size === filteredGames.length ? t('scan.deselectAll') : t('scan.selectAll')}
              </button>
            </div>
            <div className="flex gap-2">
              <button 
                onClick={handleBatchUpdate}
                disabled={selectedGameIds.size === 0}
                className="btn btn-primary btn-sm"
              >
                {t('actions.updateMetadata')}
              </button>
              <button onClick={() => { setIsSelectionMode(false); setSelectedGameIds(new Set()); }} className="btn btn-secondary btn-sm">
                {t('common.cancel')}
              </button>
            </div>
          </div>
        )}

        {launchError && (
          <div className="mx-6 mt-2 p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm">
            {launchError}
          </div>
        )}

        <div className="flex-1 overflow-hidden">
          {viewMode === 'links' ? (
            <DownloadLinksView />
          ) : gamesLoading ? (
            <div className="flex items-center justify-center h-full">
              <div className="text-gray-400">{t('common.loading')}</div>
            </div>
          ) : filteredGames.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-gray-400">
              <p className="text-lg mb-2">{t('games.noGames')}</p>
              {selectedFilter === 'all' && (
                <button onClick={() => setShowScan(true)} className="btn btn-primary mt-4">
                  {t('actions.scanFolder')}
                </button>
              )}
            </div>
          ) : viewMode === 'details' ? (
            <GameDetailsView
              games={filteredGames}
              selectedGame={selectedGameForDetails}
              selectedGames={filteredGames.filter(g => selectedGameIds.has(g.id))}
              onSelectGame={(game) => {
                 if (isSelectionMode) {
                   handleToggleSelection(game.id);
                 } else {
                   setSelectedGameForDetails(game);
                 }
              }}
              onPlay={handlePlayGame}
              onEdit={handleEditGame}
              onContextMenu={handleGameContextMenu}
              isGameRunning={isGameRunning}
              gameListWidth={gameListWidth}
              onGameListResize={handleGameListResize}
              isSelectionMode={isSelectionMode}
              onRefreshFromLocal={handleRefreshMetadata}
              updatingGameIds={updatingGameIds}
            />
          ) : (
            <div className="h-full overflow-auto p-6">
              <GameGrid
                games={filteredGames}
                viewMode={viewMode}
                onEdit={handleEditGame}
                onPlay={handlePlayGame}
                onContextMenu={handleGameContextMenu}
                isGameRunning={isGameRunning}
                isSelectionMode={isSelectionMode}
                selectedGameIds={selectedGameIds}
                onToggleSelection={handleToggleSelection}
                updatingGameIds={updatingGameIds}
              />
            </div>
          )}
        </div>
      </main>

      {showAddSpace && <AddSpaceDialog onClose={() => setShowAddSpace(false)} />}
      {showAddLink && <AddLinkDialog onClose={() => setShowAddLink(false)} onAdd={refetchGames} />}
      {showScan && <ScanDialog spaces={spaces} onClose={() => setShowScan(false)} />}
      {editingGame && <EditGameDialog game={editingGame} onClose={() => setEditingGame(null)} onSave={handleGameSaved} />}
      {showBatchMetadata && (
        <BatchMetadataDialog 
          games={games.filter(g => selectedGameIds.has(g.id))}
          onClose={() => setShowBatchMetadata(false)}
          onSave={() => {
            handleGameSaved();
            setShowBatchMetadata(false);
            setIsSelectionMode(false);
            setSelectedGameIds(new Set());
          }}
        />
      )}

      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={getGameContextMenuItems(contextMenu.game)}
          onClose={() => setContextMenu(null)}
        />
      )}

      {spaceToDelete && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
          <div className="bg-surface-300 rounded-xl p-6 shadow-lg max-w-sm w-full">
            <h3 className="text-lg font-semibold mb-4">{t('spaces.confirmDeleteTitle')}</h3>
            <p className="text-gray-300 mb-6">{t('spaces.confirmDeleteMessage', { name: spaceToDelete.name })}</p>
            <div className="flex justify-end gap-3">
              <button onClick={() => setSpaceToDelete(null)} className="btn btn-secondary">{t('common.cancel')}</button>
              <button onClick={confirmDeleteSpace} className="btn btn-primary bg-danger hover:bg-danger/80">{t('actions.delete')}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;