import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Game } from '../types';
import ResizeHandle from './ResizeHandle';

interface Props {
  games: Game[];
  selectedGame: Game | null;
  selectedGames?: Game[]; // Added for multi-selection support
  onSelectGame: (g: Game) => void;
  onPlay: (g: Game) => void;
  onEdit: (g: Game) => void;
  onContextMenu?: (e: React.MouseEvent, g: Game) => void;
  isGameRunning?: (id: string) => boolean;
  gameListWidth?: number;
  onGameListResize?: (delta: number) => void;
  isSelectionMode?: boolean; // Added
}

const fmt = (s: number, t: (k: string) => string) => {
  if (s === 0) return '-';
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60);
  return h > 0 ? h + t('games.hours') + ' ' + m + t('games.minutes') : m + t('games.minutes');
};

const fmtDate = (d: string | null) => d ? new Date(d).toLocaleDateString('ru-RU') : '-';

const coverUrl = (c: string | null) => {
  if (!c) return null;
  if (c.startsWith('http')) return c;
  try { return convertFileSrc(c); } catch { return null; }
};

const PlayIcon = () => <svg className="w-4 h-4 inline" fill="currentColor" viewBox="0 0 20 20"><path d="M6.3 2.841A1.5 1.5 0 004 4.11V15.89a1.5 1.5 0 002.3 1.269l9.344-5.89a1.5 1.5 0 000-2.538L6.3 2.84z"/></svg>;

export default function GameDetailsView({ games, selectedGame, selectedGames = [], onSelectGame, onPlay, onEdit, onContextMenu, isGameRunning, gameListWidth = 280, onGameListResize, isSelectionMode }: Props) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  const [hov, setHov] = useState<string | null>(null);

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if (['INPUT','TEXTAREA'].includes(document.activeElement?.tagName || '')) return;
      if (!games.length) return;
      
      // If in selection mode, maybe use arrows to move focus but not selection? 
      // Or move selection? For now keep existing behavior for single selection navigation.
      // But if multiple selected, what does arrow key do?
      // Let's keep it simple: arrow keys navigate the *focused* (primary selected) game.
      
      const i = selectedGame ? games.findIndex(g => g.id === selectedGame.id) : -1;
      if (e.key === 'ArrowDown') { e.preventDefault(); onSelectGame(games[i < games.length - 1 ? i + 1 : 0]); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); onSelectGame(games[i > 0 ? i - 1 : games.length - 1]); }
      else if (e.key === 'Enter' && selectedGame && !isGameRunning?.(selectedGame.id)) { e.preventDefault(); onPlay(selectedGame); }
    };
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, [games, selectedGame, onSelectGame, onPlay, isGameRunning]);

  useEffect(() => {
    if (selectedGame && ref.current) ref.current.querySelector(`[data-id="${selectedGame.id}"]`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }, [selectedGame]);

  useEffect(() => { if (!selectedGame && games.length) onSelectGame(games[0]); }, [games, selectedGame, onSelectGame]);

  const bg = selectedGame?.cover_image ? coverUrl(selectedGame.cover_image) : null;
  const run = selectedGame ? isGameRunning?.(selectedGame.id) ?? false : false;

  const openExternalLink = async (url: string) => {
    try {
      await import('@tauri-apps/plugin-shell').then(m => m.open(url));
    } catch (e) {
      console.error('Failed to open link:', e);
    }
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
              onClick={() => {
                 onSelectGame(g);
              }} 
              onDoubleClick={() => !r && !isSelectionMode && onPlay(g)} 
              onContextMenu={e => onContextMenu?.(e, g)} 
              onMouseEnter={() => setHov(g.id)} 
              onMouseLeave={() => setHov(null)}
              className={`flex items-center gap-3 mx-2 px-2 py-2 rounded-lg cursor-pointer ${isSelected ? 'bg-accent/30 ring-1 ring-accent' : hov === g.id ? 'bg-surface-200/70' : 'hover:bg-surface-200/40'} ${r && !isSelected ? 'bg-green-500/10' : ''}`}>
              
              {isSelectionMode && (
                <div className="flex-shrink-0 mr-1">
                   <div className={`w-4 h-4 border rounded ${isSelected ? 'bg-accent border-accent flex items-center justify-center' : 'border-gray-500'}`}>
                     {isSelected && <span className="text-white text-xs">✓</span>}
                   </div>
                </div>
              )}
              
              <div className="w-9 h-12 bg-surface-300 rounded overflow-hidden flex-shrink-0">{cv ? <img src={cv} className="w-full h-full object-cover" alt="" /> : <div className="w-full h-full flex items-center justify-center text-gray-500">?</div>}</div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1"><span className={`text-sm truncate ${isSelected ? 'text-white font-medium' : 'text-gray-200'}`}>{g.title}</span>{g.is_favorite && <span className="text-yellow-400 text-xs">*</span>}{r && <span className="text-green-400 text-xs"><PlayIcon/></span>}</div>
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
                <div className="w-52 h-72 rounded-lg overflow-hidden shadow-2xl flex-shrink-0 bg-surface-300">{coverUrl(selectedGame.cover_image) ? <img src={coverUrl(selectedGame.cover_image)!} className="w-full h-full object-cover" alt=""/> : <div className="w-full h-full flex items-center justify-center text-gray-500 text-4xl">?</div>}</div>
                <div className="flex-1 flex flex-col justify-end pb-2">
                  <div className="flex gap-2 mb-2">{selectedGame.is_favorite && <span className="px-2 py-0.5 bg-yellow-500/20 text-yellow-400 rounded text-xs">{t('details.favorite')}</span>}{run && <span className="px-2 py-0.5 bg-green-500/20 text-green-400 rounded text-xs animate-pulse">{t('details.running')}</span>}</div>
                  <h1 className="text-4xl font-bold text-white mb-2">{selectedGame.title}</h1>
                  <div className="text-gray-400 mb-6 text-sm">{selectedGame.developer}{selectedGame.publisher && ` | ${selectedGame.publisher}`}</div>
                  <div className="flex gap-3">
                    <button onClick={() => onPlay(selectedGame)} disabled={run} className={`px-8 py-3 rounded-lg font-semibold flex items-center gap-2 ${run ? 'bg-green-600' : 'bg-accent hover:bg-accent-hover'} text-white`}><PlayIcon/> {run ? t('details.running') : t('details.play')}</button>
                    {selectedGame.external_link && (
                      <button onClick={() => openExternalLink(selectedGame.external_link!)} className="px-6 py-3 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300 rounded-lg flex items-center gap-2">
                        🔗 Link
                      </button>
                    )}
                    <button onClick={() => onEdit(selectedGame)} className="px-6 py-3 bg-white/10 hover:bg-white/20 rounded-lg">{t('details.edit')}</button>
                    <button onClick={() => onEdit(selectedGame)} className="px-6 py-3 bg-purple-500/20 hover:bg-purple-500/30 text-purple-300 rounded-lg flex items-center gap-2">
                      🔄 {t('details.refreshMetadata')}
                    </button>
                  </div>
                </div>
              </div>
              <div className="flex gap-4 mb-8">
                <div className="bg-black/30 rounded-lg px-4 py-3"><div className="text-gray-500 text-xs mb-1">{t('details.playtime')}</div><div className="font-semibold text-lg text-white">{fmt(selectedGame.total_playtime_seconds, t)}</div></div>
                <div className="bg-black/30 rounded-lg px-4 py-3"><div className="text-gray-500 text-xs mb-1">{t('details.launches')}</div><div className="font-semibold text-lg text-white">{selectedGame.times_launched}</div></div>
                <div className="bg-black/30 rounded-lg px-4 py-3"><div className="text-gray-500 text-xs mb-1">{t('details.lastPlayed')}</div><div className="font-semibold text-lg text-white">{fmtDate(selectedGame.last_played_at)}</div></div>
              </div>
              
              {/* Install location and source links */}
              <div className="bg-black/30 rounded-xl p-5 mb-8">
                <h2 className="text-sm font-semibold text-gray-400 uppercase mb-3">{t('details.location')}</h2>
                {selectedGame.install_path ? (
                  <div className="space-y-2">
                    <div className="flex items-center gap-2">
                      <span className="text-gray-400 text-sm">{t('details.installPath')}:</span>
                      <span className="text-white text-sm font-mono">{selectedGame.install_path}</span>
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
                
                {selectedGame.external_link && (
                  <div className="mt-4 pt-4 border-t border-white/10">
                    <h3 className="text-sm font-semibold text-gray-400 uppercase mb-2">{t('details.sourceLinks')}</h3>
                    <button 
                      onClick={() => openExternalLink(selectedGame.external_link!)}
                      className="flex items-center gap-2 px-4 py-2 bg-blue-500/20 hover:bg-blue-500/30 text-blue-300 rounded-lg text-sm transition-colors"
                    >
                      <span>🔗</span>
                      <span>{selectedGame.external_link}</span>
                    </button>
                  </div>
                )}
              </div>
              {selectedGame.description && <div className="bg-black/30 rounded-xl p-5"><h2 className="text-sm font-semibold text-gray-400 uppercase mb-3">{t('details.description')}</h2><p className="text-gray-300">{selectedGame.description}</p></div>}
            </div>
          ) : <div className="h-full flex items-center justify-center text-gray-500"><div className="text-center"><p>{t('details.selectGame')}</p><p className="text-sm mt-2">{t('details.useArrows')}</p></div></div>}
        </div>
      </div>
    </div>
  );
}