import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';
import { useScanDirectory, useCreateGame } from '../hooks/useGames';
import { useSpaceSources, useScanSpaceSources } from '../hooks/useSpaces';
import type { Space, ScannedGame, SpaceSource } from '../types';

interface ScanDialogProps {
  spaces: Space[];
  onClose: () => void;
  initialMode?: 'custom' | 'space_sources';
  initialSpaceId?: string;
}

interface EditableGame extends ScannedGame {
  edited_title: string;
  selected_executable: string | null;
  selected_cover: string | null;
}

export default function ScanDialog({ 
  spaces, 
  onClose, 
  initialMode = 'custom',
  initialSpaceId 
}: ScanDialogProps) {
  const { t } = useTranslation();
  const scanDirectory = useScanDirectory();
  const createGame = useCreateGame();
  const scanSpaceSources = useScanSpaceSources();
  
  const [scanPath, setScanPath] = useState('');
  const [targetSpaceId, setTargetSpaceId] = useState(initialSpaceId || spaces[0]?.id || '');
  const [fetchMetadata, setFetchMetadata] = useState(false);
  const [scannedGames, setScannedGames] = useState<EditableGame[]>([]);
  const [selectedGames, setSelectedGames] = useState<Set<string>>(new Set());
  const [isAdding, setIsAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedGame, setExpandedGame] = useState<string | null>(null);
  const [scanMode, setScanMode] = useState<'custom' | 'space_sources'>(initialMode);
  
  // Load space sources if using space mode
  const { data: spaceSources = [] } = useSpaceSources(targetSpaceId);
  
  // Auto-scan space sources when switching to that mode and we have active sources
  useEffect(() => {
    if (scanMode === 'space_sources' && targetSpaceId) {
      handleScanSpaceSources();
    }
  }, [scanMode, targetSpaceId]);
  
  const handleSelectFolder = async () => {
    setError(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('scan.selectFolder'),
      });
      
      if (selected && typeof selected === 'string') {
        setScanPath(selected);
        setScanMode('custom');
        handleCustomScan(selected);
      }
    } catch (err) {
      console.error('Failed to open folder dialog:', err);
      setError(String(err));
    }
  };
  
  const handleCustomScan = async (path: string) => {
    setError(null);
    setScannedGames([]);
    setExpandedGame(null);
    
    try {
      const games = await scanDirectory.mutateAsync(path);
      const editableGames: EditableGame[] = games.map(g => ({
        ...g,
        edited_title: g.title,
        selected_executable: g.executable,
        selected_cover: g.cover_candidates[0] || null,
      }));
      setScannedGames(editableGames);
      setSelectedGames(new Set(games.map(g => g.path)));
    } catch (err) {
      console.error('Scan failed:', err);
      setError(String(err));
    }
  };
  
  const handleScanSpaceSources = async () => {
    setError(null);
    setScannedGames([]);
    setExpandedGame(null);
    
    try {
      const games = await scanSpaceSources.mutateAsync(targetSpaceId);
      const editableGames: EditableGame[] = games.map(g => ({
        ...g,
        edited_title: g.title,
        selected_executable: g.executable,
        selected_cover: g.cover_candidates[0] || null,
      }));
      setScannedGames(editableGames);
      setSelectedGames(new Set(games.map(g => g.path)));
    } catch (err) {
      console.error('Space scan failed:', err);
      setError(String(err));
    }
  };
  
  const handleManualScan = () => {
    if (scanPath.trim()) {
      handleCustomScan(scanPath.trim());
    }
  };
  
  const toggleGame = (path: string) => {
    setSelectedGames(prev => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };
  
  const selectAll = () => {
    setSelectedGames(new Set(scannedGames.map(g => g.path)));
  };
  
  const deselectAll = () => {
    setSelectedGames(new Set());
  };
  
  const updateGame = (path: string, updates: Partial<EditableGame>) => {
    setScannedGames(prev => prev.map(g => 
      g.path === path ? { ...g, ...updates } : g
    ));
  };
  
  const handleAddSelected = async () => {
    if (!targetSpaceId || selectedGames.size === 0) return;
    
    setIsAdding(true);
    setError(null);
    
    try {
      const gamesToAdd = scannedGames.filter(game => selectedGames.has(game.path));
      
      await Promise.all(gamesToAdd.map(game => 
        createGame.mutateAsync({
          title: game.edited_title,
          space_id: targetSpaceId,
          install_path: game.path,
          executable_path: game.selected_executable || undefined,
          cover_image: game.selected_cover || undefined,
          developer: game.exe_metadata?.company_name || undefined,
          fetch_metadata: fetchMetadata,
        })
      ));

      onClose();
    } catch (err) {
      console.error('Failed to add games:', err);
      setError(String(err));
    } finally {
      setIsAdding(false);
    }
  };
  
  const formatSize = (bytes: number): string => {
    const gb = bytes / (1024 * 1024 * 1024);
    if (gb >= 1) return `${gb.toFixed(1)} GB`;
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(0)} MB`;
  };
  
  const hasNoSpaces = spaces.length === 0;
  const activeSources = spaceSources.filter(s => s.is_active);
  
  // If we have an initialSpaceId, set it as target when spaces load
  useEffect(() => {
    if (initialSpaceId && spaces.find(s => s.id === initialSpaceId)) {
      setTargetSpaceId(initialSpaceId);
    }
  }, [initialSpaceId, spaces]);
  
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-surface-300 rounded-xl w-full max-w-3xl max-h-[85vh] flex flex-col shadow-2xl">
        {/* Header */}
        <div className="p-4 border-b border-surface-100 flex items-center justify-between flex-shrink-0">
          <h2 className="text-lg font-semibold">{t('scan.title')}</h2>
          <button onClick={onClose} className="text-gray-500 hover:text-white transition-colors">
            ✕
          </button>
        </div>
        
        {/* Content */}
        <div className="p-4 flex-1 overflow-hidden flex flex-col">
          {/* Warning if no spaces */}
          {hasNoSpaces && (
            <div className="mb-4 p-3 bg-warning/20 border border-warning/50 rounded-lg text-warning text-sm">
              ⚠️ {t('scan.noSpacesWarning')}
            </div>
          )}
          
          {/* Error message */}
          {error && (
            <div className="mb-4 p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm">
              ❌ {error}
            </div>
          )}
          
          {/* Scan mode selector */}
          {!hasNoSpaces && (
            <div className="mb-4 flex gap-2">
              <button
                onClick={() => setScanMode('custom')}
                className={`btn ${scanMode === 'custom' ? 'btn-primary' : 'btn-secondary'}`}
              >
                📁 {t('scan.customScan')}
              </button>
              <button
                onClick={() => setScanMode('space_sources')}
                className={`btn ${scanMode === 'space_sources' ? 'btn-primary' : 'btn-secondary'}`}
                disabled={activeSources.length === 0}
              >
                📚 {t('scan.spaceSourcesScan', { space: spaces.find(s => s.id === targetSpaceId)?.name })}
                {` (${activeSources.length})`}
              </button>
            </div>
          )}
          
          {/* Folder selection (custom mode) */}
          {scanMode === 'custom' && (
            <div className="flex gap-2 mb-4">
              <input
                type="text"
                value={scanPath}
                onChange={e => setScanPath(e.target.value)}
                placeholder={t('scan.pathPlaceholder')}
                className="flex-1 px-3 py-2 bg-surface-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-accent"
                onKeyDown={e => e.key === 'Enter' && handleManualScan()}
              />
              <button 
                onClick={handleManualScan}
                disabled={!scanPath.trim() || scanDirectory.isPending}
                className="btn btn-secondary disabled:opacity-50"
              >
                🔍
              </button>
              <button onClick={handleSelectFolder} className="btn btn-primary">
                📁 {t('space.selectFolder')}
              </button>
            </div>
          )}
          
          {/* Space sources info (space mode) */}
          {scanMode === 'space_sources' && (
            <div className="mb-4 p-3 bg-surface-400 rounded-lg">
              <p className="text-sm text-gray-300 mb-1">{t('space.scanningSources')}:</p>
              <ul className="text-sm text-gray-400 space-y-1">
                {activeSources.map((source: SpaceSource) => (
                  <li key={source.source_path} className="truncate">
                    • {source.source_path}
                  </li>
                ))}
              </ul>
            </div>
          )}
          
          {/* Scanning state */}
          {(scanDirectory.isPending || scanSpaceSources.isPending) && (
            <div className="flex items-center justify-center py-8">
              <div className="animate-spin w-8 h-8 border-2 border-accent border-t-transparent rounded-full" />
              <span className="ml-3 text-gray-400">{t('scan.scanning')}</span>
            </div>
          )}
          
          {/* Results */}
          {scannedGames.length > 0 && (
            <>
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm text-gray-400">
                  {t('scan.foundGames', { count: scannedGames.length })}
                </span>
                <div className="flex gap-2">
                  <button onClick={selectAll} className="text-xs text-accent hover:underline">
                    {t('scan.selectAll')}
                  </button>
                  <button onClick={deselectAll} className="text-xs text-gray-500 hover:underline">
                    {t('scan.deselectAll')}
                  </button>
                </div>
              </div>
              
              {/* Games list */}
              <div className="flex-1 overflow-auto border border-surface-100 rounded-lg min-h-0">
                {scannedGames.map((game: EditableGame) => (
                  <div key={game.path} className="border-b border-surface-100 last:border-0">
                    {/* Game row */}
                    <div className="flex items-center gap-3 p-3 hover:bg-surface-200">
                      <input
                        type="checkbox"
                        checked={selectedGames.has(game.path)}
                        onChange={() => toggleGame(game.path)}
                        className="w-4 h-4 rounded accent-accent flex-shrink-0"
                      />
                      
                      {/* Icon preview */}
                      <div className="w-10 h-10 rounded bg-surface-100 flex items-center justify-center flex-shrink-0 overflow-hidden">
                        {game.icon_path ? (
                          <img 
                            src={`file://${game.icon_path}`} 
                            alt="" 
                            className="w-full h-full object-contain"
                            onError={(e) => { e.currentTarget.style.display = 'none'; }}
                          />
                        ) : (
                          <span className="text-2xl opacity-50">🎮</span>
                        )}
                      </div>
                      
                      <div className="flex-1 min-w-0">
                        <input
                          type="text"
                          value={game.edited_title}
                          onChange={e => updateGame(game.path, { edited_title: e.target.value })}
                          onClick={e => e.stopPropagation()}
                          className="w-full bg-transparent font-medium focus:outline-none focus:bg-surface-100 px-1 rounded"
                          title={t('scan.editTitle')}
                        />
                        <p className="text-xs text-gray-500 truncate px-1">{game.path}</p>
                        {game.exe_metadata?.company_name && (
                          <p className="text-xs text-gray-400 px-1">
                            {game.exe_metadata.company_name}
                          </p>
                        )}
                      </div>
                      
                      <div className="text-sm text-gray-500 flex-shrink-0">
                        {formatSize(game.size_bytes)}
                      </div>
                      
                      {/* Exe status */}
                      {game.all_executables.length > 0 ? (
                        <span className="text-xs text-green-500 flex-shrink-0" title={game.selected_executable || ''}>
                          ✅ {game.all_executables.length} exe
                        </span>
                      ) : (
                        <span className="text-xs text-yellow-500 flex-shrink-0">⚠️ no exe</span>
                      )}
                      
                      {/* Expand button */}
                      <button
                        onClick={() => setExpandedGame(expandedGame === game.path ? null : game.path)}
                        className="p-1 hover:bg-surface-100 rounded transition-colors flex-shrink-0"
                        title={t('scan.showDetails')}
                      >
                        {expandedGame === game.path ? '▼' : '▶'}
                      </button>
                    </div>
                    
                    {/* Expanded details */}
                    {expandedGame === game.path && (
                      <div className="px-4 pb-4 pt-2 bg-surface-200/50 space-y-4">
                        {/* Executable selection */}
                        {game.all_executables.length > 0 && (
                          <div>
                            <label className="block text-xs font-medium text-gray-400 mb-1">
                              {t('scan.selectExecutable')}
                            </label>
                            <select
                              value={game.selected_executable || ''}
                              onChange={e => updateGame(game.path, { selected_executable: e.target.value || null })}
                              className="w-full px-2 py-1.5 bg-surface-200 rounded text-sm focus:outline-none focus:ring-1 focus:ring-accent"
                            >
                              {game.all_executables.map((exe: string) => (
                                <option key={exe} value={exe}>{exe}</option>
                              ))}
                            </select>
                          </div>
                        )}
                        
                        {/* Cover selection */}
                        {game.cover_candidates.length > 0 && (
                          <div>
                            <label className="block text-xs font-medium text-gray-400 mb-1">
                              {t('scan.selectCover')}
                            </label>
                            <div className="flex gap-2 overflow-x-auto pb-2">
                              <button
                                onClick={() => updateGame(game.path, { selected_cover: null })}
                                className={`flex-shrink-0 w-16 h-20 rounded border-2 flex items-center justify-center text-xs ${
                                  !game.selected_cover ? 'border-accent' : 'border-surface-100'
                                }`}
                              >
                                {t('scan.noCover')}
                              </button>
                              {game.cover_candidates.map((cover: string) => (
                                <button
                                  key={cover}
                                  onClick={() => updateGame(game.path, { selected_cover: cover })}
                                  className={`flex-shrink-0 w-16 h-20 rounded border-2 overflow-hidden ${
                                    game.selected_cover === cover ? 'border-accent' : 'border-surface-100'
                                  }`}
                                  title={cover}
                                >
                                  <img
                                    src={`file://${game.path}/${cover}`}
                                    alt={cover}
                                    className="w-full h-full object-cover"
                                    onError={(e) => {
                                      e.currentTarget.parentElement!.style.display = 'none';
                                    }}
                                  />
                                </button>
                              ))}
                            </div>
                          </div>
                        )}
                        
                        {/* Metadata from exe */}
                        {game.exe_metadata && (
                          <div className="text-xs text-gray-400 space-y-1">
                            <p className="font-medium text-gray-300">{t('scan.exeMetadata')}</p>
                            {game.exe_metadata.product_name && (
                              <p>📦 {t('scan.productName')}: {game.exe_metadata.product_name}</p>
                            )}
                            {game.exe_metadata.company_name && (
                              <p>🏢 {t('scan.companyName')}: {game.exe_metadata.company_name}</p>
                            )}
                            {game.exe_metadata.file_description && (
                              <p>📝 {t('scan.fileDescription')}: {game.exe_metadata.file_description}</p>
                            )}
                            {game.exe_metadata.file_version && (
                              <p>🔢 {t('scan.fileVersion')}: {game.exe_metadata.file_version}</p>
                            )}
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                ))}
              </div>
              
              {/* Target space selection */}
              <div className="mt-4">
                <label className="block text-sm font-medium mb-1">
                  {t('scan.targetSpace')}
                </label>
                <select
                  value={targetSpaceId}
                  onChange={e => setTargetSpaceId(e.target.value)}
                  className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-accent"
                  disabled={hasNoSpaces}
                >
                  {spaces.map(space => (
                    <option key={space.id} value={space.id}>
                      {space.icon} {space.name}
                    </option>
                  ))}
                </select>
              </div>

              {/* Fetch Metadata Option */}
              <div className="mt-4 flex items-center gap-2">
                <input
                  type="checkbox"
                  id="fetchMetadata"
                  checked={fetchMetadata}
                  onChange={e => setFetchMetadata(e.target.checked)}
                  className="w-4 h-4 rounded accent-accent"
                />
                <label htmlFor="fetchMetadata" className="text-sm font-medium cursor-pointer select-none">
                  {t('scan.fetchMetadata')} (Steam/Itch.io)
                </label>
              </div>
            </>
          )}
          
          {/* Empty state after scan */}
          {!scanDirectory.isPending && !scanSpaceSources.isPending && scannedGames.length === 0 && (scanPath || scanMode === 'space_sources') && !error && (
            <div className="text-center py-8 text-gray-500">
              {t('scan.noGamesFound')}
            </div>
          )}
          
          {/* Initial state */}
          {!scanPath && scanMode === 'custom' && !scanDirectory.isPending && (
            <div className="flex-1 flex flex-col items-center justify-center text-gray-500">
              <span className="text-4xl mb-4">📁</span>
              <p>{t('scan.selectFolderHint')}</p>
            </div>
          )}
          
          {/* Space sources empty state */}
          {scanMode === 'space_sources' && activeSources.length === 0 && (
            <div className="text-center py-8 text-gray-500">
              {t('space.noSources')}
            </div>
          )}
        </div>
        
        {/* Actions */}
        <div className="p-4 border-t border-surface-100 flex justify-end gap-2 flex-shrink-0">
          <button onClick={onClose} className="btn btn-secondary">
            {t('common.cancel')}
          </button>
          <button
            onClick={handleAddSelected}
            disabled={selectedGames.size === 0 || !targetSpaceId || isAdding || hasNoSpaces}
            className="btn btn-primary disabled:opacity-50"
          >
            {isAdding ? t('common.loading') : `${t('scan.addSelected')} (${selectedGames.size})`}
          </button>
        </div>
      </div>
    </div>
  );
}