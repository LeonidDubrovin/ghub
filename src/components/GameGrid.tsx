import { useTranslation } from 'react-i18next';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Game } from '../types';
import GameCard from './GameCard';

const getCoverUrl = (cover: string | null): string | null => {
  if (!cover) return null;
  if (cover.startsWith('http://') || cover.startsWith('https://')) return cover;
  try { return convertFileSrc(cover); } catch { return null; }
};

const PlayIcon = () => (
  <svg className="w-4 h-4 inline" fill="currentColor" viewBox="0 0 20 20">
    <path d="M6.3 2.841A1.5 1.5 0 004 4.11V15.89a1.5 1.5 0 002.3 1.269l9.344-5.89a1.5 1.5 0 000-2.538L6.3 2.84z" />
  </svg>
);

const StarIcon = () => (
  <svg className="w-4 h-4 inline" fill="currentColor" viewBox="0 0 24 24">
    <path d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
  </svg>
);

const EditIcon = () => (
  <svg className="w-4 h-4 inline" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
  </svg>
);

const GamepadIcon = () => (
  <svg className="w-8 h-8 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M15 5v2m0 4v2m0 4v2M5 5a2 2 0 00-2 2v3a2 2 0 110 4v3a2 2 0 002 2h14a2 2 0 002-2v-3a2 2 0 110-4V7a2 2 0 00-2-2H5z" />
  </svg>
);

interface GameGridProps {
  games: Game[];
  viewMode: 'grid' | 'list';
  onEdit: (game: Game) => void;
  onPlay?: (game: Game) => void;
  onContextMenu?: (e: React.MouseEvent, game: Game) => void;
  isGameRunning?: (gameId: string) => boolean;
  isSelectionMode?: boolean;
  selectedGameIds?: Set<string>;
  onToggleSelection?: (gameId: string) => void;
  updatingGameIds?: Set<string>;
}

export default function GameGrid({
  games,
  viewMode,
  onEdit,
  onPlay,
  onContextMenu,
  isGameRunning,
  isSelectionMode,
  selectedGameIds,
  onToggleSelection,
  updatingGameIds,
}: GameGridProps) {
  const { t } = useTranslation();

  if (viewMode === 'list') {
    return (
      <div className="space-y-2">
        {games.map(game => {
          const running = isGameRunning?.(game.id) ?? false;
          const selected = selectedGameIds?.has(game.id) ?? false;
          const updating = updatingGameIds?.has(game.id) ?? false;
          const coverUrl = getCoverUrl(game.cover_image);
          return (
            <div
              key={game.id}
              onContextMenu={(e) => onContextMenu?.(e, game)}
              onClick={() => isSelectionMode && onToggleSelection?.(game.id)}
              onDoubleClick={() => !isSelectionMode && onEdit(game)}
              className={`flex items-center gap-4 p-3 bg-surface-200 rounded-lg hover:bg-surface-100 transition-colors cursor-pointer group 
                ${running ? 'ring-2 ring-green-500/50' : ''}
                ${selected ? 'ring-2 ring-accent bg-surface-100' : ''}
                ${updating ? 'bg-yellow-500/20 animate-pulse' : ''}
              `}
            >
              {isSelectionMode && (
                <div className={`w-5 h-5 rounded border flex items-center justify-center ${selected ? 'bg-accent border-accent' : 'border-gray-500'}`}>
                  {selected && <span className="text-white text-xs">✓</span>}
                </div>
              )}
              <div className="w-16 h-16 bg-surface-300 rounded overflow-hidden flex-shrink-0 relative">
                {coverUrl ? (
                  <img src={coverUrl} alt={game.title} className="w-full h-full object-cover" />
                ) : (
                  <div className="w-full h-full flex items-center justify-center">
                    <GamepadIcon />
                  </div>
                )}
                {updating && (
                  <div className="absolute inset-0 bg-yellow-500/40 flex items-center justify-center">
                    <div className="bg-black/70 px-2 py-1 rounded text-yellow-300 text-xs font-medium animate-pulse">
                      ⏳
                    </div>
                  </div>
                )}
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <h3 className="font-medium truncate">{game.title}</h3>
                  {game.is_favorite && <span className="text-yellow-400"><StarIcon /></span>}
                  {running && <span className="px-1.5 py-0.5 bg-green-500/20 text-green-400 text-xs rounded">{t('details.running')}</span>}
                  {updating && <span className="px-1.5 py-0.5 bg-yellow-500/20 text-yellow-400 text-xs rounded animate-pulse">{t('details.updating')}</span>}
                </div>
                <p className="text-sm text-gray-500 truncate">{game.developer || 'Unknown'}</p>
              </div>
              <div className="flex gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                <button onClick={(e) => { e.stopPropagation(); onEdit(game); }} className="btn btn-secondary text-sm flex items-center gap-1">
                  <EditIcon />
                </button>
                <button
                  onClick={(e) => { e.stopPropagation(); onPlay?.(game); }}
                  disabled={running}
                  className={`btn flex items-center gap-1 ${running ? 'bg-green-600 cursor-not-allowed' : 'btn-primary'}`}
                >
                  <PlayIcon /> {t('actions.play')}
                </button>
              </div>
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
      {games.map(game => {
        const updating = updatingGameIds?.has(game.id) ?? false;
        return (
          <div key={game.id} className="relative group" onClick={() => isSelectionMode && onToggleSelection?.(game.id)}>
            {/* Selection Overlay for Grid */}
            {isSelectionMode && (
              <div className={`absolute top-2 right-2 z-20 w-6 h-6 rounded-full border-2 flex items-center justify-center transition-colors cursor-pointer
                ${selectedGameIds?.has(game.id) ? 'bg-accent border-accent' : 'bg-black/50 border-white/50 hover:border-white'}`}
              >
                {selectedGameIds?.has(game.id) && <span className="text-white text-xs font-bold">✓</span>}
              </div>
            )}
            {updating && (
              <div className="absolute top-2 left-2 z-20 bg-yellow-500/80 px-2 py-1 rounded text-white text-xs font-medium animate-pulse">
                ⏳ {t('details.updating')}
              </div>
            )}
            <GameCard
              game={game}
              onEdit={onEdit}
              onPlay={onPlay}
              onContextMenu={onContextMenu}
              isRunning={isGameRunning?.(game.id) ?? false}
              onClick={isSelectionMode ? () => {
                // Click handled by overlay
              } : undefined}
            />
            {isSelectionMode && <div className="absolute inset-0 z-30 cursor-pointer" />} 
          </div>
        );
      })}
    </div>
  );
}