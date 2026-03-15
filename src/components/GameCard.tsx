import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Game } from '../types';

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
  <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
    <path d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
  </svg>
);

const EditIcon = () => (
  <svg className="w-4 h-4 inline" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
  </svg>
);

const GamepadIcon = () => (
  <svg className="w-16 h-16 opacity-30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M15 5v2m0 4v2m0 4v2M5 5a2 2 0 00-2 2v3a2 2 0 110 4v3a2 2 0 002 2h14a2 2 0 002-2v-3a2 2 0 110-4V7a2 2 0 00-2-2H5z" />
  </svg>
);

interface GameCardProps {
  game: Game;
  onEdit: (game: Game) => void;
  onPlay?: (game: Game) => void;
  onContextMenu?: (e: React.MouseEvent, game: Game) => void;
  isRunning?: boolean;
  // Make card pass clicks up if needed
  onClick?: (e: React.MouseEvent) => void;
}

export default function GameCard({
  game,
  onEdit,
  onPlay,
  onContextMenu,
  isRunning = false,
  onClick,
}: GameCardProps) {
  const { t } = useTranslation();
  const [isHovered, setIsHovered] = useState(false);
  const coverUrl = getCoverUrl(game.cover_image);

  return (
    <div
      className={`card group cursor-pointer relative ${isRunning ? 'ring-2 ring-green-500/50' : ''}`}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onContextMenu={(e) => { e.preventDefault(); onContextMenu?.(e, game); }}
      onClick={onClick}
      onDoubleClick={() => onEdit(game)}
    >
      <div className="aspect-[3/4] bg-surface-300 relative overflow-hidden">
        {coverUrl ? (
          <img
            src={coverUrl}
            alt={game.title}
            className="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center">
            <GamepadIcon />
          </div>
        )}

        {isRunning && (
          <div className="absolute inset-0 bg-green-500/10 flex items-center justify-center">
            <div className="bg-green-500/90 px-3 py-1 rounded-full text-white text-sm font-medium animate-pulse flex items-center gap-1">
              <PlayIcon /> {t('details.running')}
            </div>
          </div>
        )}

        <div className={`absolute inset-0 bg-gradient-to-t from-black/80 via-black/40 to-transparent transition-opacity duration-200 ${isHovered && !isRunning ? 'opacity-100' : 'opacity-0'}`}>
          <div className="absolute bottom-0 left-0 right-0 p-3 space-y-2">
            <button
              onClick={(e) => { e.stopPropagation(); if (!isRunning) onPlay?.(game); }}
              disabled={isRunning}
              className={`w-full btn flex items-center justify-center gap-1 ${isRunning ? 'bg-green-600 cursor-not-allowed' : 'btn-primary'}`}
            >
              <PlayIcon /> {isRunning ? t('details.running') : t('actions.play')}
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); onEdit(game); }}
              className="w-full btn btn-secondary text-sm flex items-center justify-center gap-1"
            >
              <EditIcon /> {t('actions.editGame')}
            </button>
          </div>
        </div>

        {game.is_favorite && (
          <div className="absolute top-2 right-2 text-yellow-400 drop-shadow">
            <StarIcon />
          </div>
        )}
      </div>

      <div className="p-3">
        <h3 className="font-medium text-sm truncate" title={game.title}>{game.title}</h3>
        {game.developer && <p className="text-xs text-gray-500 truncate">{game.developer}</p>}
        {game.space_name && (
          <div className="mt-1">
            <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-surface-300 text-gray-300">
              {game.space_name}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
