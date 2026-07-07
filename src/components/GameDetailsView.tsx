import { useState, useEffect, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import i18n from '../lib/i18n';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen, type Event } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-shell';
import type { Game, GameLink, Install, ItchUpload, DownloadGameLinkResponse } from '../types';
import ResizeHandle from './ResizeHandle';
import DeleteInstallDialog from './DeleteInstallDialog';
import MetadataUpdateDialog from './MetadataUpdateDialog';
import SelectTargetSpaceDialog from './SelectTargetSpaceDialog';
import SelectUploadDialog from './SelectUploadDialog';
import SettingsDialog from './SettingsDialog';
import { useSpaces } from '../hooks/useSpaces';

interface Props {
  games: Game[];
  selectedGame: Game | null;
  selectedGames?: Game[]; // Added for multi-selection support
  selectedSpaceId?: string | null;
  onSelectGame: (g: Game, shiftKey?: boolean) => void;
  onPlay: (g: Game, install?: Install) => void;
  onEdit: (g: Game) => void;
  onContextMenu?: (e: React.MouseEvent, g: Game) => void;
  isGameRunning?: (id: string) => boolean;
  gameListWidth?: number;
  onGameListResize?: (delta: number) => void;
  isSelectionMode?: boolean; // Added
  onSave?: () => void;
  onGameDownloaded?: (game: Game, spaceId: string, sourcePath: string) => void;
}

const fmt = (s: number, t: (k: string) => string) => {
  if (s === 0) return '-';
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60);
  return h > 0 ? h + t('games.hours') + ' ' + m + t('games.minutes') : m + t('games.minutes');
};

const fmtDate = (d: string | null) => d ? new Date(d).toLocaleDateString(i18n.language || 'ru-RU') : '-';

const formatBytes = (bytes: number) => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
};

const coverUrl = (c: string | null) => {
  if (!c) return null;
  if (c.startsWith('http')) return c;
  try { return convertFileSrc(c); } catch { return null; }
};

const PlayIcon = () => <svg className="w-4 h-4 inline" fill="currentColor" viewBox="0 0 20 20"><path d="M6.3 2.841A1.5 1.5 0 004 4.11V15.89a1.5 1.5 0 002.3 1.269l9.344-5.89a1.5 1.5 0 000-2.538L6.3 2.84z"/></svg>;

export default function GameDetailsView({ 
  games, 
  selectedGame, 
  selectedGames = [], 
  selectedSpaceId,
  onSelectGame, 
  onPlay, 
  onEdit, 
  onContextMenu,
  isGameRunning,
  gameListWidth = 280,
  onGameListResize,
  isSelectionMode,
  onSave,
  onGameDownloaded
}: Props) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  const [hov, setHov] = useState<string | null>(null);
  const [gameLinks, setGameLinks] = useState<GameLink[]>([]);
  const [isUpdateDialogOpen, setIsUpdateDialogOpen] = useState(false);
  const [showTargetDialog, setShowTargetDialog] = useState(false);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [downloadErrorLinkId, setDownloadErrorLinkId] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<{ downloaded: number; total: number } | null>(null);
  const [downloadSpeed, setDownloadSpeed] = useState<number | null>(null);
  const lastProgressRef = useRef<{ downloaded: number; time: number } | null>(null);
  const [downloadingLinkId, setDownloadingLinkId] = useState<string | null>(null);
  const [isFetchingUploads, setIsFetchingUploads] = useState(false);
  const [installs, setInstalls] = useState<Install[]>([]);
  const [uploads, setUploads] = useState<ItchUpload[]>([]);
  const [showUploadDialog, setShowUploadDialog] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);
  const [installToDelete, setInstallToDelete] = useState<Install | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [selectedUpload, setSelectedUpload] = useState<ItchUpload | null>(null);
  const { data: spaces = [] } = useSpaces();

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (['INPUT','TEXTAREA'].includes(document.activeElement?.tagName || '')) return;
      if (!games.length) return;
      
      const i = selectedGame ? games.findIndex(g => g.id === selectedGame.id) : -1;
      if (e.key === 'ArrowDown') { e.preventDefault(); onSelectGame(games[i < games.length - 1 ? i + 1 : 0]); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); onSelectGame(games[i > 0 ? i - 1 : games.length - 1]); }
      else if (e.key === 'Enter' && selectedGame && !isGameRunning?.(selectedGame.id)) { e.preventDefault(); onPlay(selectedGame, installs[0]); }
    };
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, [games, selectedGame, onSelectGame, onPlay, isGameRunning]);

  useEffect(() => {
    if (selectedGame && ref.current) ref.current.querySelector(`[data-id="${selectedGame.id}"]`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }, [selectedGame]);

  useEffect(() => { if (!selectedGame && games.length) onSelectGame(games[0]); }, [games, selectedGame, onSelectGame]);

  // When the selected game changes, close upload/target dialogs and reset the upload-fetch state,
  // so another game's download UI never leaks onto the new card.
  useEffect(() => {
    setShowUploadDialog(false);
    setShowTargetDialog(false);
    setIsFetchingUploads(false);
    setSelectedUpload(null);
  }, [selectedGame?.id]);

  // Fetch game links when selected game changes
  useEffect(() => {
    const fetchLinks = async () => {
      if (selectedGame) {
        try {
          const links = await invoke<GameLink[]>('get_game_links', { gameId: selectedGame.id });
          setGameLinks(links);
        } catch (error) {
          console.error('Failed to fetch game links:', error);
          setGameLinks([]);
        }
      } else {
        setGameLinks([]);
      }
    };
    fetchLinks();
  }, [selectedGame]);

  // Fetch installed variants when the selected game changes
  useEffect(() => {
    const fetchInstalls = async () => {
      if (selectedGame) {
        try {
          const data = await invoke<Install[]>('get_game_installs', { gameId: selectedGame.id });
          setInstalls(data);
        } catch (error) {
          console.error('Failed to fetch installs:', error);
          setInstalls([]);
        }
      } else {
        setInstalls([]);
      }
    };
    fetchInstalls();
  }, [selectedGame]);

  const bg = selectedGame?.cover_image ? coverUrl(selectedGame.cover_image) : null;
  const run = selectedGame ? isGameRunning?.(selectedGame.id) ?? false : false;

  const activeLink = useMemo(() => {
    if (!selectedGame || (selectedSpaceId !== 'incoming' && selectedSpaceId !== 'online')) return null;
    return gameLinks.find(l => l.queue_space === selectedSpaceId) || gameLinks[0] || null;
  }, [selectedGame, selectedSpaceId, gameLinks]);

  const itchLink = useMemo(() => {
    if (!selectedGame) return null;
    return gameLinks.find(l => l.source_type === 'itch') || null;
  }, [selectedGame, gameLinks]);

  // The download state is global (because progress comes from backend events), but we only render
  // it for the currently selected game/link so it never leaks onto another card.
  const isThisDownloading = useMemo(() => {
    const activeId = activeLink?.id || itchLink?.id;
    return !!activeId && activeId === downloadingLinkId;
  }, [activeLink, itchLink, downloadingLinkId]);

  const isThisError = useMemo(() => {
    const activeId = activeLink?.id || itchLink?.id;
    return !!activeId && activeId === downloadErrorLinkId;
  }, [activeLink, itchLink, downloadErrorLinkId]);

  const isThisButtonBusy = isThisDownloading || isFetchingUploads;

  // Listen for download progress events for the active link
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      unlisten = await listen('download-progress', (e: Event<{ link_id: string; downloaded: number; total: number }>) => {
        if (downloadingLinkId && e.payload.link_id === downloadingLinkId) {
          const now = Date.now();
          const prev = lastProgressRef.current;
          let speed: number | null = null;
          if (prev && now > prev.time) {
            const bytesDelta = e.payload.downloaded - prev.downloaded;
            const timeDelta = (now - prev.time) / 1000;
            if (timeDelta > 0 && bytesDelta > 0) {
              speed = bytesDelta / timeDelta;
            }
          }
          lastProgressRef.current = { downloaded: e.payload.downloaded, time: now };
          setDownloadProgress({ downloaded: e.payload.downloaded, total: e.payload.total });
          setDownloadSpeed(speed);
        }
      });
    };
    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [downloadingLinkId]);

  const openGameLink = async (link: GameLink) => {
    try {
      await open(link.url);
    } catch (e) {
      console.error('Failed to open link:', e);
    }
  };

  const handleCopyLink = async (url: string) => {
    try {
      await navigator.clipboard.writeText(url);
    } catch (e) {
      console.error('Failed to copy link:', e);
    }
  };

  const CopyButton = ({ url }: { url: string | undefined }) => {
    if (!url) return null;
    return (
      <button
        onClick={() => handleCopyLink(url)}
        className="px-2 py-3 bg-blue-500/10 hover:bg-blue-500/20 text-blue-300 rounded-lg text-sm flex items-center"
        title={t('details.copyLink')}
      >
        📋
      </button>
    );
  };

  const handleMoveLink = async (targetQueueSpace: 'incoming' | 'online') => {
    if (!activeLink) return;
    try {
      await invoke('move_game_link', { linkId: activeLink.id, queueSpace: targetQueueSpace });
      onSave?.();
    } catch (e) {
      console.error('Failed to move link:', e);
    }
  };

  const handlePlayInstall = (install: Install) => {
    if (!selectedGame) return;
    onPlay(selectedGame, install);
  };

  const handleOpenInstallFolder = async (install: Install) => {
    try {
      await open(install.install_path);
    } catch (e) {
      console.error('Failed to open install folder:', e);
    }
  };

  const handleUpdateMetadata = () => {
    if (!selectedGame) return;
    setIsUpdateDialogOpen(true);
  };

  const handleDeleteInstall = (install: Install) => {
    setInstallToDelete(install);
    setShowDeleteDialog(true);
  };

  const handleDeleteConfirm = async (deleteFiles: boolean) => {
    if (!installToDelete) return;
    setIsDeleting(true);
    try {
      await invoke('delete_game_install', {
        installId: installToDelete.id,
        deleteFiles,
      });
      const data = await invoke<Install[]>('get_game_installs', { gameId: installToDelete.game_id });
      setInstalls(data);
      setShowDeleteDialog(false);
      setInstallToDelete(null);
      onSave?.();
    } catch (e) {
      console.error('Failed to delete install:', e);
      throw e;
    } finally {
      setIsDeleting(false);
    }
  };

  const handleStartDownload = async () => {
    if (!selectedGame || !itchLink) return;
    setDownloadError(null);
    setDownloadErrorLinkId(null);
    setDownloadProgress(null);
    setShowUploadDialog(true);
    setIsFetchingUploads(true);
    try {
      const key = await invoke<string | null>('get_itch_api_key');
      if (!key) {
        setShowUploadDialog(false);
        setShowSettings(true);
        return;
      }
      const data = await invoke<ItchUpload[]>('get_itch_game_uploads', { gameUrl: itchLink.url, gameTitle: selectedGame.title });
      if (data.length === 0) {
        setShowUploadDialog(false);
        setDownloadError(t('errors.noUploads'));
        setDownloadErrorLinkId(itchLink.id);
        return;
      }
      setUploads(data);
    } catch (e) {
      console.error('Failed to start download:', e);
      const err = String(e);
      setShowUploadDialog(false);
      setDownloadError(err);
      setDownloadErrorLinkId(itchLink.id);
      alert(err);
    } finally {
      setIsFetchingUploads(false);
    }
  };

  const handleSelectUpload = (upload: ItchUpload) => {
    setSelectedUpload(upload);
    setShowUploadDialog(false);
    setShowTargetDialog(true);
  };

  const handleDownloadTarget = async (spaceId: string, sourcePath: string) => {
    if (!selectedGame || !itchLink || !selectedUpload) return;
    setShowTargetDialog(false);
    setDownloadingLinkId(itchLink.id);
    setDownloadError(null);
    setDownloadErrorLinkId(null);
    setDownloadProgress(null);
    try {
      const response = await invoke<DownloadGameLinkResponse>('download_game_link', {
        gameId: selectedGame.id,
        linkId: itchLink.id,
        uploadId: selectedUpload.id,
        uploadName: selectedUpload.display_name || selectedUpload.filename,
        uploadFilename: selectedUpload.filename,
        uploadPlatforms: selectedUpload.platforms
          ? Object.entries(selectedUpload.platforms)
              .filter(([, enabled]) => enabled)
              .map(([platform]) => platform)
          : null,
        spaceId,
        sourcePath,
      });
      const data = await invoke<Install[]>('get_game_installs', { gameId: selectedGame.id });
      setInstalls(data);
      const links = await invoke<GameLink[]>('get_game_links', { gameId: selectedGame.id });
      setGameLinks(links);
      if (response.status === 'downloaded') {
        onGameDownloaded?.(response.game, spaceId, sourcePath);
      } else {
        onSave?.();
      }
    } catch (e) {
      console.error('Failed to download game:', e);
      const err = String(e);
      setDownloadError(err);
      setDownloadErrorLinkId(itchLink.id);
      alert(err);
      onSave?.();
    } finally {
      setDownloadingLinkId(null);
      setDownloadProgress(null);
      setDownloadSpeed(null);
      lastProgressRef.current = null;
      setSelectedUpload(null);
    }
  };


  const getSourceIcon = (sourceType: string | null) => {
    switch (sourceType) {
      case 'steam':
        return '🎮';
      case 'itch':
        return '🎨';
      case 'gog':
        return '🛡️';
      case 'epic':
        return '⚔️';
      default:
        return '🔗';
    }
  };

  const getSourceLabel = (link: GameLink) => {
    if (link.title) return link.title;
    const sourceKey = link.source_type?.toLowerCase() || 'other';
    return t(`sources.${sourceKey}`, link.source_type || 'Other');
  };

  return (
    <div className="flex h-full overflow-hidden">
      <div ref={ref} className="flex-shrink-0 bg-surface-400 overflow-y-auto py-2" style={{ width: gameListWidth }}>
        {games.map(g => {
          const isSelected = isSelectionMode 
            ? selectedGames.some(sg => sg.id === g.id) 
            : selectedGame?.id === g.id;
            
          const r = isGameRunning?.(g.id) ?? false;
          const cv = coverUrl(g.cover_image);

          return (
            <div key={g.id} data-id={g.id}
              onClick={(e) => {
                e.preventDefault();
                onSelectGame(g, e.shiftKey);
              }}
              onDoubleClick={() => !r && !isSelectionMode && onPlay(g)}
              onContextMenu={e => onContextMenu?.(e, g)}
              onMouseEnter={() => setHov(g.id)}
              onMouseLeave={() => setHov(null)}
              className={`flex items-center gap-3 mx-2 px-2 py-2 rounded-lg cursor-pointer select-none
                ${isSelected ? 'bg-accent/30 ring-1 ring-accent' : hov === g.id ? 'bg-surface-200/70' : 'hover:bg-surface-200/40'}
                ${g.times_launched === 0 && !isSelected && !r ? 'bg-surface-100/50 border-l-4 border-solid border-gray-500' : ''}
                ${r && !isSelected ? 'bg-green-500/10' : ''}`}>

              {isSelectionMode && (
                <div className="flex-shrink-0 mr-1">
                   <div className={`w-4 h-4 border rounded ${isSelected ? 'bg-accent border-accent flex items-center justify-center' : 'border-gray-500'}`}>
                     {isSelected && <span className="text-white text-xs">✓</span>}
                   </div>
                </div>
              )}

              <div className="w-9 h-12 bg-surface-300 rounded overflow-hidden flex-shrink-0">{cv ? <img src={cv} className="w-full h-full object-cover" alt="" /> : <div className="w-full h-full flex items-center justify-center text-gray-500">?</div>}</div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1">
                  <span className={`text-sm truncate ${isSelected ? 'text-white font-medium' : 'text-gray-200'}`}>{g.title}</span>
                  {g.is_favorite && <span className="text-yellow-400 text-xs">*</span>}
                  {r && <span className="text-green-400 text-xs"><PlayIcon/></span>}
                </div>
                <div className="text-xs text-gray-500">{fmt(g.total_playtime_seconds, t)}</div>
              </div>
            </div>
          );
        })}
        {!games.length && <div className="p-6 text-center text-gray-500">{t('games.noGames')}</div>}
      </div>
      
      {onGameListResize && <ResizeHandle onResize={onGameListResize} />}
      
      <div className="flex-1 relative overflow-hidden">
        {bg && <div className="absolute inset-0" style={{backgroundImage:`url(${bg})`,backgroundSize:'cover',backgroundPosition:'center'}}><div className="absolute inset-0 bg-gradient-to-r from-surface-300/95 to-surface-300/70"/></div>}
        {!bg && <div className="absolute inset-0 bg-gradient-to-br from-surface-300 to-surface-400"/>}
        <div className="relative h-full overflow-y-auto p-8">
          {selectedGame ? (
            <div className="max-w-4xl">
              <div className="flex gap-8 mb-8">
                <div className="w-52 h-72 rounded-lg overflow-hidden shadow-2xl flex-shrink-0 bg-surface-300 relative">
                  {coverUrl(selectedGame.cover_image) ? <img src={coverUrl(selectedGame.cover_image)!} className="w-full h-full object-cover" alt=""/> : <div className="w-full h-full flex items-center justify-center text-gray-500 text-4xl">?</div>}
                </div>
                <div className="flex-1 flex flex-col justify-end pb-2">
                  <div className="flex gap-2 mb-2">
                    {selectedGame.is_favorite && <span className="px-2 py-0.5 bg-yellow-500/20 text-yellow-400 rounded text-xs">{t('details.favorite')}</span>}
                    {run && <span className="px-2 py-0.5 bg-green-500/20 text-green-400 rounded text-xs animate-pulse">{t('details.running')}</span>}
                  </div>
                  <h1 className="text-4xl font-bold text-white mb-2">{selectedGame.title}</h1>
                  <div className="text-gray-400 mb-6 text-sm">{selectedGame.developer}{selectedGame.publisher && ` | ${selectedGame.publisher}`}</div>
                   <div className="flex gap-3 flex-wrap">
                       {selectedSpaceId === 'incoming' && activeLink ? (
                         <>
                            {activeLink.source_type === 'itch' ? (
                              activeLink.download_status === 'browser' ? (
                                <button
                                  onClick={() => openGameLink(activeLink)}
                                  className="px-8 py-3 rounded-lg font-semibold flex items-center gap-2 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300"
                                >
                                  {t('actions.playInBrowser')}
                                </button>
                              ) : (
                              <button
                                onClick={handleStartDownload}
                                disabled={isThisButtonBusy}
                                className="px-8 py-3 rounded-lg font-semibold flex items-center gap-2 bg-accent hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed text-white"
                              >
                                {isThisButtonBusy ? (
                                  <span className="flex items-center gap-2">
                                    <span className="animate-spin">⏳</span> {t('common.loading')}
                                  </span>
                                ) : (
                                  <>
                                    <PlayIcon /> {activeLink.download_status === 'error' ? t('actions.retry') : t('actions.download')}
                                  </>
                                )}
                              </button>

                              )
                            ) : (
                              <button
                                onClick={() => openGameLink(activeLink)}
                                className="px-8 py-3 rounded-lg font-semibold flex items-center gap-2 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300"
                              >
                                {activeLink.source_type === 'steam' ? t('actions.openStore') : t('actions.openLink')}
                              </button>
                            )}
                           <button
                             onClick={() => handleMoveLink('online')}
                             className="px-6 py-3 bg-white/10 hover:bg-white/20 rounded-lg"
                           >
                             {t('actions.moveToOnline')}
                           </button>
                         </>
                       ) : selectedSpaceId === 'online' && activeLink ? (
                         <>
                           {activeLink.source_type === 'itch' ? (
                             <div className="flex items-center gap-1">
                               <button
                                 onClick={() => openGameLink(activeLink)}
                                 className="px-8 py-3 rounded-lg font-semibold flex items-center gap-2 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300"
                               >
                                 {t('actions.playInBrowser')}
                               </button>
                                <button
                                  onClick={handleStartDownload}
                                  disabled={isThisButtonBusy}
                                  className="px-4 py-3 bg-accent/20 hover:bg-accent/30 text-accent rounded-lg flex items-center gap-2 text-sm disabled:opacity-50 disabled:cursor-not-allowed"
                                >
                                  {isThisButtonBusy ? <span className="animate-spin">⏳</span> : '⬇'}
                                  <span>{t('actions.downloadVariant')}</span>
                                </button>

                             </div>
                           ) : (
                             <button
                               onClick={() => openGameLink(activeLink)}
                               className="px-8 py-3 rounded-lg font-semibold flex items-center gap-2 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300"
                             >
                               {activeLink.source_type === 'steam' ? t('actions.openStore') : t('actions.openLink')}
                             </button>
                           )}
                           <button
                             onClick={() => handleMoveLink('incoming')}
                             className="px-6 py-3 bg-white/10 hover:bg-white/20 rounded-lg"
                           >
                             {t('actions.moveToIncoming')}
                           </button>
                         </>
                       ) : (
                       <>
                         <button
                           onClick={() => onPlay(selectedGame, installs[0])}
                           disabled={run}
                           className={`px-8 py-3 rounded-lg font-semibold flex items-center gap-2 ${run ? 'bg-green-600' : 'bg-accent hover:bg-accent-hover'} text-white`}
                         >
                           <PlayIcon /> {run ? t('details.running') : t('details.play')}
                         </button>
                            {gameLinks.length > 0 && gameLinks.map(link => (
                              <button
                                key={link.id}
                                onClick={() => openGameLink(link)}
                                className="px-4 py-3 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300 rounded-lg flex items-center gap-2 text-sm"
                                title={link.url}
                              >
                                <span>{getSourceIcon(link.source_type)}</span>
                                <span>{getSourceLabel(link)}</span>
                              </button>
                            ))}
                           {itchLink && (
                             <button
                               onClick={handleStartDownload}
                               disabled={isThisButtonBusy}
                               className="px-4 py-3 bg-accent/20 hover:bg-accent/30 text-accent rounded-lg flex items-center gap-2 text-sm disabled:opacity-50 disabled:cursor-not-allowed"
                             >
                               {isThisButtonBusy ? <span className="animate-spin">⏳</span> : '⬇'}
                               <span>{t('actions.downloadVariant')}</span>
                             </button>
                           )}

                        </>
                     )}
                      <button
                        onClick={handleUpdateMetadata}
                        className="px-6 py-3 rounded-lg flex items-center gap-2 bg-purple-500/20 hover:bg-purple-500/30 text-purple-300"
                      >
                        🔄 {t('actions.updateMetadata')}
                      </button>

                      <button
                        onClick={() => onEdit(selectedGame)}
                        className="px-6 py-3 bg-white/10 hover:bg-white/20 rounded-lg"
                      >
                        {t('details.edit')}
                      </button>

                    </div>
                    {downloadError && isThisError && (
                      <div className="mt-3 text-sm text-red-400 bg-red-500/10 rounded-lg px-3 py-2">
                        {downloadError}
                      </div>
                    )}
                     {downloadProgress && isThisDownloading && (
                       <div className="mt-3 w-full">
                         <div className="h-2 bg-surface-100 rounded-full overflow-hidden">
                           <div
                             className="h-full bg-accent transition-all duration-200"
                             style={{ width: `${downloadProgress.total > 0 ? Math.min(100, Math.round((downloadProgress.downloaded / downloadProgress.total) * 100)) : 0}%` }}
                           />
                         </div>
                         <div className="text-xs text-gray-400 mt-1 flex justify-between">
                           <span>
                             {formatBytes(downloadProgress.downloaded)}
                             {downloadProgress.total > 0 ? ` / ${formatBytes(downloadProgress.total)}` : ''}
                             {downloadProgress.total > 0 ? ` (${Math.round((downloadProgress.downloaded / downloadProgress.total) * 100)}%)` : ''}
                           </span>
                           {downloadSpeed != null && downloadSpeed > 0 && (
                             <span>{formatBytes(downloadSpeed)}/s</span>
                           )}
                         </div>
                       </div>
                     )}
                 </div>
               </div>
               <div className="flex gap-4 mb-8">
                <div className="bg-black/30 rounded-lg px-4 py-3">
                  <div className="text-gray-500 text-xs mb-1">{t('details.playtime')}</div>
                  <div className="font-semibold text-lg text-white">{fmt(selectedGame.total_playtime_seconds, t)}</div>
                </div>
                <div className="bg-black/30 rounded-lg px-4 py-3">
                  <div className="text-gray-500 text-xs mb-1">{t('details.launches')}</div>
                  <div className="font-semibold text-lg text-white">{selectedGame.times_launched}</div>
                </div>
                <div className="bg-black/30 rounded-lg px-4 py-3">
                  <div className="text-gray-500 text-xs mb-1">{t('details.lastPlayed')}</div>
                  <div className="font-semibold text-lg text-white">{fmtDate(selectedGame.last_played_at)}</div>
                </div>
              </div>
              
              <div className="bg-black/30 rounded-xl p-5 mb-8">
                <h2 className="text-sm font-semibold text-gray-400 uppercase mb-3">{t('details.location')}</h2>
                {selectedGame.install_path ? (
                  <div className="space-y-2">
                    <div className="flex items-center gap-2">
                      <span className="text-gray-400 text-sm">{t('details.installPath')}:</span>
                      <span className="text-white text-sm font-mono flex-1 truncate">{selectedGame.install_path}</span>
                      <button
                        onClick={async () => {
                          try {
                            await invoke('open_folder', { path: selectedGame.install_path! });
                          } catch (error) {
                            console.error('Failed to open folder:', error);
                            alert(`Failed to open folder: ${error}`);
                          }
                        }}
                        className="px-3 py-1 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300 rounded text-xs flex items-center gap-1 transition-colors"
                        title={t('actions.openFolder')}
                      >
                        <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                        </svg>
                        {t('actions.openFolder')}
                      </button>
                    </div>
                    {selectedGame.space_name && (
                      <div className="flex items-center gap-2">
                        <span className="text-gray-400 text-sm">{t('details.space')}:</span>
                        <span className="text-white text-sm">{selectedGame.space_name}</span>
                      </div>
                    )}
                  </div>
                ) : (
                  <p className="text-gray-500 text-sm">{t('details.noInstallPath')}</p>
                )}
                
                 {gameLinks.length > 0 && (
                   <div className="mt-4 pt-4 border-t border-white/10">
                     <h3 className="text-sm font-semibold text-gray-400 uppercase mb-2">{t('details.sourceLinks')}</h3>
                     <div className="space-y-2">
                        {gameLinks.map(link => (
                          <div key={link.id} className="flex items-center gap-1">
                            <button
                              onClick={() => openGameLink(link)}
                              className="flex-1 flex items-center gap-2 px-4 py-2 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300 rounded-lg text-sm transition-colors text-left"
                              title={link.url}
                            >
                              <span className="text-base">{getSourceIcon(link.source_type)}</span>
                              <div className="flex-1 min-w-0">
                                <div className="truncate">{getSourceLabel(link)}</div>
                                <div className="text-xs text-blue-200/60 truncate">{link.url}</div>
                              </div>
                            </button>
                            <CopyButton url={link.url} />
                          </div>
                        ))}
                     </div>
                   </div>
                 )}
               </div>

               {installs.length > 0 && (
                 <div className="bg-black/30 rounded-xl p-5 mb-8">
                   <h2 className="text-sm font-semibold text-gray-400 uppercase mb-3">{t('details.installedVariants')}</h2>
                   <div className="space-y-2">
                      {installs.map(install => (
                        <div key={install.id} className="flex items-center gap-2 px-3 py-2 bg-surface-400 rounded-lg">
                          <div className="flex-1 min-w-0">
                            <div className="text-sm text-white truncate">{install.version || install.install_path}</div>
                            <div className="text-xs text-gray-500 truncate">{install.install_path}</div>
                          </div>
                          <button
                            onClick={() => handleOpenInstallFolder(install)}
                            className="px-3 py-1 bg-white/10 hover:bg-white/20 text-white rounded text-xs flex items-center gap-1"
                            title={t('actions.openFolder')}
                          >
                            📁
                          </button>
                          <CopyButton url={itchLink?.url || gameLinks[0]?.url} />
                          <button
                            onClick={() => handlePlayInstall(install)}
                            disabled={run}
                            className="px-3 py-1 bg-accent/20 hover:bg-accent/30 text-accent rounded text-xs flex items-center gap-1 disabled:opacity-50"
                          >
                            <PlayIcon /> {t('actions.play')}
                          </button>
                          <button
                            onClick={() => handleDeleteInstall(install)}
                            className="px-3 py-1 bg-red-500/20 hover:bg-red-500/30 text-red-400 rounded text-xs"
                          >
                            {t('actions.delete')}
                          </button>
                        </div>
                      ))}
                   </div>
                 </div>
               )}
               
               {selectedGame.description && (
                 <div className="bg-black/30 rounded-xl p-5">
                   <h2 className="text-sm font-semibold text-gray-400 uppercase mb-3">{t('details.description')}</h2>
                   <p className="text-gray-300">{selectedGame.description}</p>
                 </div>
               )}
            </div>
          ) : (
            <div className="h-full flex items-center justify-center text-gray-500">
              <div className="text-center">
                <p>{t('details.selectGame')}</p>
                <p className="text-sm mt-2">{t('details.useArrows')}</p>
              </div>
            </div>
          )}
        </div>
      </div>

      {selectedGame && (
        <MetadataUpdateDialog
          game={selectedGame}
          isOpen={isUpdateDialogOpen}
          onClose={() => setIsUpdateDialogOpen(false)}
          onSave={() => {
            setIsUpdateDialogOpen(false);
            onSave?.();
          }}
        />
      )}

      {selectedGame && showDeleteDialog && (
        <DeleteInstallDialog
          install={installToDelete}
          isOpen={showDeleteDialog}
          onClose={() => {
            setShowDeleteDialog(false);
            setInstallToDelete(null);
          }}
          onConfirm={handleDeleteConfirm}
          isPending={isDeleting}
        />
      )}

      {selectedGame && showTargetDialog && (
        <SelectTargetSpaceDialog
          spaces={spaces}
          onClose={() => setShowTargetDialog(false)}
          onSelect={handleDownloadTarget}
        />
      )}

      {selectedGame && showUploadDialog && (
        <SelectUploadDialog
          uploads={uploads}
          isLoading={isFetchingUploads}
          onClose={() => {
            setShowUploadDialog(false);
            setIsFetchingUploads(false);
          }}
          onSelect={handleSelectUpload}
        />
      )}

      {showSettings && (
        <SettingsDialog isOpen={showSettings} onClose={() => setShowSettings(false)} />
      )}
    </div>
  );
}