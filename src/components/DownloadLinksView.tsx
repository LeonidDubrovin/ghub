import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import type { Game, GameLink, Space, SpaceSource, SelectedSource } from '../types';
import { createLoggerForComponent } from '../lib/logger';
import { useSpaces } from '../hooks/useSpaces';

interface DownloadItem {
  game: Game;
  link: GameLink;
}

interface DownloadLinksViewProps {
  refreshKey?: number;
}

export default function DownloadLinksView({ refreshKey = 0 }: DownloadLinksViewProps) {
  const logger = createLoggerForComponent('DownloadLinksView');
  const { t } = useTranslation();
  const [items, setItems] = useState<DownloadItem[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [targetDialog, setTargetDialog] = useState<DownloadItem | null>(null);
  const { data: spaces = [] } = useSpaces();

  const loadItems = async () => {
    setIsLoading(true);
    try {
      const games = await invoke<Game[]>('get_download_games');
      const loaded = await Promise.all(
        games.map(async (game) => {
          const links = await invoke<GameLink[]>('get_game_links', { gameId: game.id });
          const link =
            links.find((l) => l.download_status && l.download_status !== 'downloaded') ||
            links[0];
          return { game, link };
        })
      );
      setItems(loaded);
    } catch (err) {
      logger.error('Failed to load download games:', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadItems();
  }, [refreshKey]);

  const handleOpen = async (item: DownloadItem) => {
    try {
      await invoke('open_game_link', {
        url: item.game.external_link || item.link.url,
        sourceType: item.link.source_type,
      });
    } catch (err) {
      logger.error('Failed to open link:', err);
      alert(String(err));
    }
  };

  const handleDownloadClick = (item: DownloadItem) => {
    setTargetDialog(item);
  };

  const handleConfirmDownload = async (target: SelectedSource) => {
    if (!targetDialog) return;
    setTargetDialog(null);
    setDownloadingId(targetDialog.game.id);
    try {
      await invoke('download_game_link', {
        gameId: targetDialog.game.id,
        linkId: targetDialog.link.id,
        spaceId: target.spaceId,
        sourcePath: target.sourcePath,
      });
      await loadItems();
    } catch (err) {
      logger.error('Failed to download game:', err);
      alert(String(err));
    } finally {
      setDownloadingId(null);
    }
  };

  const handleRemove = async (item: DownloadItem) => {
    if (!confirm(t('links.confirmDelete'))) return;
    try {
      await invoke('remove_game_link', { linkId: item.link.id });
      await loadItems();
    } catch (err) {
      logger.error('Failed to remove link:', err);
      alert(String(err));
    }
  };

  const getStatusLabel = (status: string | null) => {
    switch (status) {
      case 'pending':
        return t('links.statusPending');
      case 'external':
        return t('links.statusExternal');
      case 'browser':
        return t('links.statusBrowser');
      case 'error':
        return t('links.statusError');
      default:
        return t('links.statusUnknown');
    }
  };

  const renderActions = (item: DownloadItem) => {
    const status = item.link.download_status;
    const source = item.link.source_type;

    if (status === 'error') {
      return (
        <button
          onClick={() => handleDownloadClick(item)}
          disabled={downloadingId === item.game.id}
          className="btn btn-sm btn-primary"
        >
          {downloadingId === item.game.id ? t('common.loading') : t('links.retry')}
        </button>
      );
    }

    if (source === 'itch' && status === 'pending') {
      return (
        <button
          onClick={() => handleDownloadClick(item)}
          disabled={downloadingId === item.game.id}
          className="btn btn-sm btn-primary"
        >
          {downloadingId === item.game.id ? t('common.loading') : t('links.download')}
        </button>
      );
    }

    return (
      <button onClick={() => handleOpen(item)} className="btn btn-sm btn-secondary">
        {source === 'steam' ? t('links.openStore') : t('links.openInBrowser')}
      </button>
    );
  };

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center text-gray-500">
        {t('common.loading')}
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-gray-500">
        <span className="text-4xl mb-4">🔗</span>
        <p>{t('links.noLinks')}</p>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-4">
        {items.map((item) => (
          <div
            key={item.link.id}
            className="bg-surface-300 rounded-xl overflow-hidden shadow-lg group hover:ring-2 hover:ring-accent transition-all"
          >
            <div className="flex h-32">
              {/* Cover */}
              <div className="w-24 bg-black/20 flex-shrink-0 relative">
                {item.game.cover_image ? (
                  <img
                    src={item.game.cover_image}
                    alt=""
                    className="w-full h-full object-cover"
                  />
                ) : (
                  <div className="w-full h-full flex items-center justify-center text-2xl opacity-20">?</div>
                )}
                <div className="absolute top-1 left-1 bg-black/60 text-white text-[10px] px-1 rounded uppercase">
                  {getStatusLabel(item.link.download_status)}
                </div>
                {item.link.source_type && (
                  <div className="absolute bottom-1 left-1 bg-black/60 text-white text-[10px] px-1 rounded uppercase">
                    {item.link.source_type}
                  </div>
                )}
              </div>

              {/* Info */}
              <div className="flex-1 p-3 flex flex-col min-w-0">
                <div className="flex justify-between items-start gap-2">
                  <h3
                    className="font-bold text-white truncate"
                    title={item.game.title}
                  >
                    {item.game.title}
                  </h3>
                  <button
                    onClick={() => handleRemove(item)}
                    className="text-gray-500 hover:text-red-500 transition-colors"
                    title={t('common.delete')}
                  >
                    ✕
                  </button>
                </div>

                <div className="text-xs text-gray-400 line-clamp-2 mb-auto mt-1">
                  {item.game.description || t('links.noDescription')}
                </div>

                <div className="mt-2 flex items-center justify-between gap-2">
                  <span className="text-[10px] text-gray-500">{item.game.added_at}</span>
                  {renderActions(item)}
                </div>
              </div>
            </div>
          </div>
        ))}
      </div>

      {targetDialog && (
        <DownloadTargetDialog
          spaces={spaces}
          onClose={() => setTargetDialog(null)}
          onConfirm={handleConfirmDownload}
        />
      )}
    </div>
  );
}

interface DownloadTargetDialogProps {
  spaces: Space[];
  onClose: () => void;
  onConfirm: (target: SelectedSource) => void;
}

function DownloadTargetDialog({ spaces, onClose, onConfirm }: DownloadTargetDialogProps) {
  const { t } = useTranslation();
  const [sourcesBySpace, setSourcesBySpace] = useState<Record<string, SpaceSource[]>>({});
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<SelectedSource | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const entries = await Promise.all(
          spaces.map(async (space) => {
            const sources = await invoke<SpaceSource[]>('get_space_sources', { spaceId: space.id });
            return [space.id, sources] as [string, SpaceSource[]];
          })
        );
        if (!cancelled) {
          setSourcesBySpace(Object.fromEntries(entries));
          // Auto-select the first available source.
          for (const [spaceId, sources] of entries) {
            if (sources.length > 0) {
              setSelected({ spaceId, sourcePath: sources[0].source_path });
              break;
            }
          }
        }
      } catch (err) {
        console.error('Failed to load sources:', err);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    load();
    return () => {
      cancelled = true;
    };
  }, [spaces]);

  const hasAnySource = Object.values(sourcesBySpace).some((s) => s.length > 0);

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4">
      <div className="bg-surface-300 rounded-xl w-full max-w-md shadow-2xl ring-1 ring-white/10 p-6">
        <h2 className="text-xl font-bold mb-4">{t('links.selectTarget')}</h2>

        {loading ? (
          <div className="text-center text-gray-400 py-4">{t('common.loading')}</div>
        ) : !hasAnySource ? (
          <div className="text-center text-danger py-4">{t('links.noSources')}</div>
        ) : (
          <div className="space-y-2 max-h-[60vh] overflow-y-auto pr-1">
            {spaces.map((space) => {
              const sources = sourcesBySpace[space.id] || [];
              if (sources.length === 0) return null;
              return (
                <div key={space.id} className="bg-surface-200 rounded-lg p-3">
                  <div className="font-semibold text-sm mb-2">{space.name}</div>
                  <div className="space-y-1">
                    {sources.map((source) => {
                      const isSelected =
                        selected?.spaceId === space.id &&
                        selected?.sourcePath === source.source_path;
                      return (
                        <label
                          key={source.source_path}
                          className={`flex items-center gap-2 p-2 rounded cursor-pointer text-sm ${
                            isSelected ? 'bg-accent/20 ring-1 ring-accent' : 'hover:bg-surface-100'
                          }`}
                        >
                          <input
                            type="radio"
                            name="downloadTarget"
                            checked={isSelected}
                            onChange={() =>
                              setSelected({ spaceId: space.id, sourcePath: source.source_path })
                            }
                            className="accent-accent"
                          />
                          <span className="truncate" title={source.source_path}>
                            {source.source_path}
                          </span>
                        </label>
                      );
                    })}
                  </div>
                </div>
              );
            })}
          </div>
        )}

        <div className="flex justify-end gap-3 pt-4">
          <button onClick={onClose} className="btn btn-secondary" disabled={loading}>
            {t('common.cancel')}
          </button>
          <button
            onClick={() => selected && onConfirm(selected)}
            className="btn btn-primary"
            disabled={!selected || loading}
          >
            {t('links.download')}
          </button>
        </div>
      </div>
    </div>
  );
}
