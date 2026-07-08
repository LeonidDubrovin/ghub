import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import type { Game, MetadataSearchResult, ScannedGame, Install, GameLink, AddGameLinkResponse } from '../types';
import { createLoggerForComponent } from '../lib/logger';

type SourceStatus = 'idle' | 'loading' | 'done' | 'empty' | 'error';

interface MetadataSearchDialogProps {
  isOpen: boolean;
  games: Game[];
  onClose: () => void;
  onSave: () => void;
  onOpenGame?: (game: Game, link?: GameLink) => void;
  mode?: 'search' | 'update';
}

interface SourceState {
  steam: boolean;
  itch: boolean;
}

interface SourceStatusMap {
  steam: SourceStatus;
  itch: SourceStatus;
}

interface SourceErrorMap {
  steam: string | null;
  itch: string | null;
}

interface FieldSelection {
  title: boolean;
  description: boolean;
  developer: boolean;
  publisher: boolean;
  cover: boolean;
}

export default function MetadataSearchDialog({
  isOpen,
  games,
  onClose,
  onSave,
  onOpenGame,
  mode = 'search',
}: MetadataSearchDialogProps) {
  const logger = createLoggerForComponent('MetadataSearchDialog');
  const { t } = useTranslation();

  const isUpdateMode = mode === 'update';
  const isBatch = !isUpdateMode && games.length > 1;
  const gameIdsKey = games.map(g => g.id).join(',');

  const [currentIndex, setCurrentIndex] = useState(0);
  const [query, setQuery] = useState('');
  const [includeSources, setIncludeSources] = useState<SourceState>({ steam: true, itch: true });
  const [sourceStatus, setSourceStatus] = useState<SourceStatusMap>({
    steam: 'idle',
    itch: 'idle',
  });
  const [sourceErrors, setSourceErrors] = useState<SourceErrorMap>({
    steam: null,
    itch: null,
  });
  const [results, setResults] = useState<MetadataSearchResult[]>([]);
  const [selectedResult, setSelectedResult] = useState<MetadataSearchResult | null>(null);
  const [duplicateLink, setDuplicateLink] = useState<{ game: Game; link: GameLink } | null>(null);
  const [fields, setFields] = useState<FieldSelection>({
    title: true,
    description: true,
    developer: true,
    publisher: true,
    cover: true,
  });
  const [isApplying, setIsApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Unified update mode state
  const [activeTab, setActiveTab] = useState<'local' | 'internet'>('internet');
  const [installs, setInstalls] = useState<Install[]>([]);
  const [localScanned, setLocalScanned] = useState<ScannedGame | null>(null);
  const [isLocalLoading, setIsLocalLoading] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [urlInput, setUrlInput] = useState('');
  const [isUrlFetching, setIsUrlFetching] = useState(false);
  const [urlError, setUrlError] = useState<string | null>(null);

  const currentGame = games[currentIndex] || null;

  const resetState = useCallback(() => {
    setSourceStatus({ steam: 'idle', itch: 'idle' });
    setSourceErrors({ steam: null, itch: null });
    setResults([]);
    setSelectedResult(null);
    setError(null);
  }, []);

  const defaultFields = useCallback((result: MetadataSearchResult): FieldSelection => ({
    title: true,
    description: !!result.description,
    developer: !!result.developer,
    publisher: !!result.publisher,
    cover: !!result.cover_url,
  }), []);

  const searchSource = useCallback(async (source: keyof SourceState) => {
    const trimmedQuery = query.trim();
    if (!trimmedQuery) return;

    setSourceStatus(prev => ({ ...prev, [source]: 'loading' }));
    setSourceErrors(prev => ({ ...prev, [source]: null }));

    try {
      const sourceResults = await invoke<MetadataSearchResult[]>('search_game_metadata', {
        query: trimmedQuery,
        sources: [source],
      });

      setResults(prev => {
        const withoutSource = prev.filter(r => r.source !== source);
        return [...withoutSource, ...sourceResults].sort((a, b) => {
          if (a.source === b.source) return a.name.localeCompare(b.name);
          return a.source === 'steam' ? -1 : 1;
        });
      });

      setSourceStatus(prev => ({
        ...prev,
        [source]: sourceResults.length > 0 ? 'done' : 'empty',
      }));
    } catch (err) {
      logger.error(`${source} search failed:`, err);
      setSourceStatus(prev => ({ ...prev, [source]: 'error' }));
      setSourceErrors(prev => ({ ...prev, [source]: String(err) }));
    }
  }, [query, logger]);

  const runSearch = useCallback(() => {
    resetState();
    const trimmedQuery = query.trim();
    if (!trimmedQuery) return;

    (Object.keys(includeSources) as (keyof SourceState)[]).forEach(source => {
      if (includeSources[source]) {
        searchSource(source);
      }
    });
  }, [query, includeSources, resetState, searchSource]);

  const inferSourceType = useCallback((url: string): 'itch' | 'steam' | null => {
    const lower = url.toLowerCase();
    if (lower.includes('itch.io')) return 'itch';
    if (lower.includes('steampowered.com/app/') || lower.includes('store.steampowered.com/app/')) return 'steam';
    return null;
  }, []);

  const fetchExactMetadata = useCallback(async (sourceType: string, url: string) => {
    setIsUrlFetching(true);
    setUrlError(null);
    try {
      const exact = await invoke<MetadataSearchResult | null>('fetch_metadata_by_url_command', {
        sourceType,
        url,
      });
      if (exact) {
        setSelectedResult(exact);
        setFields(defaultFields(exact));
      } else {
        setUrlError(t('metadataSearch.noExactMetadata'));
      }
    } catch (err) {
      logger.warn('Exact metadata fetch failed for URL:', err);
      setUrlError(String(err));
    } finally {
      setIsUrlFetching(false);
    }
  }, [defaultFields, logger, t]);

  const handleFetchByUrl = useCallback(() => {
    const trimmed = urlInput.trim();
    if (!trimmed) return;
    const sourceType = inferSourceType(trimmed);
    if (!sourceType) {
      setUrlError(t('metadataSearch.unknownUrl'));
      return;
    }
    fetchExactMetadata(sourceType, trimmed);
  }, [urlInput, inferSourceType, fetchExactMetadata, t]);

  const handleApplyLocal = useCallback(async (): Promise<boolean> => {
    if (!currentGame) return false;
    setIsApplying(true);
    setLocalError(null);
    try {
      await invoke('refresh_game_from_local', { gameId: currentGame.id });
      onSave();
      return true;
    } catch (err) {
      logger.error('Failed to refresh from local:', err);
      setLocalError(String(err));
      return false;
    } finally {
      setIsApplying(false);
    }
  }, [currentGame, onSave, logger]);

  const coverUrl = useCallback((path: string | null) => {
    if (!path) return null;
    if (path.startsWith('http')) return path;
    try { return convertFileSrc(path); } catch { return null; }
  }, []);

  // Reset index when the dialog opens or the set of games changes
  useEffect(() => {
    if (!isOpen || games.length === 0) return;
    setCurrentIndex(0);
  }, [isOpen, gameIdsKey]);

  // Update query and run search when the current game changes.
  // In update mode only run when the Internet tab is active.
  useEffect(() => {
    if (!isOpen || !currentGame) return;
    if (isUpdateMode && activeTab !== 'internet') return;

    setQuery(currentGame.title);
    resetState();
    const timer = setTimeout(() => {
      runSearch();
    }, 100);
    return () => clearTimeout(timer);
  }, [isOpen, currentGame?.id, isUpdateMode, activeTab, resetState, runSearch]);

  // Update mode: load installs, game links, and pick default tab.
  useEffect(() => {
    if (!isOpen || !isUpdateMode || !currentGame) return;

    let cancelled = false;
    const load = async () => {
      try {
        const [installData, linkData] = await Promise.all([
          invoke<Install[]>('get_game_installs', { gameId: currentGame.id }),
          invoke<GameLink[]>('get_game_links', { gameId: currentGame.id }),
        ]);
        if (cancelled) return;
        setInstalls(installData);
        setActiveTab(installData.length > 0 ? 'local' : 'internet');

        const typedLink = linkData.find(
          l => l.source_type === 'itch' || l.source_type === 'steam'
        );
        if (typedLink && typedLink.source_type) {
          setUrlInput(typedLink.url);
          fetchExactMetadata(typedLink.source_type, typedLink.url);
        }
      } catch (err) {
        logger.error('Failed to load game context for metadata update:', err);
        setActiveTab('internet');
      }
    };
    load();
    return () => { cancelled = true; };
  }, [isOpen, isUpdateMode, currentGame?.id, fetchExactMetadata, logger]);

  // Update mode: scan local files when the Local tab is active.
  useEffect(() => {
    if (!isOpen || !isUpdateMode || activeTab !== 'local' || !currentGame) return;

    let cancelled = false;
    const load = async () => {
      setIsLocalLoading(true);
      setLocalError(null);
      try {
        const data = await invoke<ScannedGame | null>('scan_local_metadata', { gameId: currentGame.id });
        if (cancelled) return;
        setLocalScanned(data);
        if (!data) {
          setLocalError(t('metadataSearch.noLocalData'));
        }
      } catch (err) {
        if (!cancelled) {
          setLocalError(String(err));
          setLocalScanned(null);
        }
      } finally {
        if (!cancelled) setIsLocalLoading(false);
      }
    };
    load();
    return () => { cancelled = true; };
  }, [isOpen, isUpdateMode, activeTab, currentGame?.id, t]);

  const handleSelectResult = useCallback(async (result: MetadataSearchResult) => {
    setSelectedResult(result);
    setFields(defaultFields(result));

    // Fetch exact metadata from the selected source page so the preview and
    // the applied fields come from the real page, not the fuzzy search result.
    if (result.url) {
      try {
        const exact = await invoke<MetadataSearchResult | null>('fetch_metadata_by_url_command', {
          sourceType: result.source,
          url: result.url,
        });
        if (exact) {
          setSelectedResult(exact);
          setFields(defaultFields(exact));
        }
      } catch (err) {
        logger.warn('Exact metadata fetch failed for selected result:', err);
      }
    }
  }, [defaultFields, logger]);

  const handleToggleField = useCallback((field: keyof FieldSelection) => {
    setFields(prev => ({ ...prev, [field]: !prev[field] }));
  }, []);

  const saveCurrentGame = useCallback(async (): Promise<boolean> => {
    if (!currentGame || !selectedResult) return false;

    setIsApplying(true);
    setError(null);
    setDuplicateLink(null);
    let linkDuplicate: { game: Game; link: GameLink } | null = null;

    const resultToApply: MetadataSearchResult = {
      ...selectedResult,
      name: fields.title ? selectedResult.name : currentGame.title,
      description: fields.description ? selectedResult.description : currentGame.description,
      developer: fields.developer ? selectedResult.developer : currentGame.developer,
      publisher: fields.publisher ? selectedResult.publisher : currentGame.publisher,
      cover_url: fields.cover ? selectedResult.cover_url : currentGame.cover_image,
    };

    try {
      if (selectedResult.url) {
        try {
          const response = await invoke<AddGameLinkResponse>('add_game_link', {
            gameId: currentGame.id,
            url: selectedResult.url,
            title: selectedResult.name,
            sourceType: selectedResult.source,
            downloadStatus: null,
            queueSpace: null,
          });
          if (response.is_duplicate && response.existing_game) {
            linkDuplicate = { game: response.existing_game, link: response.link };
            setDuplicateLink(linkDuplicate);
          }
        } catch (linkErr) {
          logger.warn('Failed to add source link:', linkErr);
          // Do not fail the whole operation if only the link could not be added
        }
      }

      if (!linkDuplicate) {
        await invoke('apply_game_metadata', {
          gameId: currentGame.id,
          sourceType: selectedResult.source,
          meta: resultToApply,
        });
        onSave();
      }
      return true;
    } catch (err) {
      logger.error('Failed to apply metadata:', err);
      setError(String(err));
      return false;
    } finally {
      setIsApplying(false);
    }
  }, [currentGame, selectedResult, fields, onSave, logger]);

  const goNext = useCallback(() => {
    if (currentIndex < games.length - 1) {
      setCurrentIndex(prev => prev + 1);
    } else {
      onClose();
    }
  }, [currentIndex, games.length, onClose]);

  const handleApply = useCallback(async () => {
    if (isUpdateMode && activeTab === 'local') {
      const success = await handleApplyLocal();
      if (success) {
        onClose();
      }
      return;
    }
    const success = await saveCurrentGame();
    if (success) {
      goNext();
    }
  }, [isUpdateMode, activeTab, handleApplyLocal, saveCurrentGame, goNext, onClose]);

  const handleSkip = useCallback(() => {
    goNext();
  }, [goNext]);

  const goPrev = useCallback(() => {
    if (currentIndex > 0) {
      setCurrentIndex(prev => prev - 1);
    }
  }, [currentIndex]);

  const handleClose = useCallback(() => {
    onClose();
  }, [onClose]);

  const sourceLabel = (source: keyof SourceState) => t(`sources.${source}`);

  const statusText = (source: keyof SourceState) => {
    switch (sourceStatus[source]) {
      case 'loading':
        return t('metadataSearch.searching', { source: sourceLabel(source) });
      case 'done':
        return t('metadataSearch.found', {
          source: sourceLabel(source),
          count: results.filter(r => r.source === source).length,
        });
      case 'empty':
        return t('metadataSearch.empty', { source: sourceLabel(source) });
      case 'error':
        return t('metadataSearch.error', { source: sourceLabel(source), error: sourceErrors[source] || '' });
      default:
        return t('metadataSearch.waiting', { source: sourceLabel(source) });
    }
  };

  const statusDot = (status: SourceStatus) => {
    switch (status) {
      case 'loading':
        return 'bg-yellow-400 animate-pulse';
      case 'done':
        return 'bg-green-400';
      case 'empty':
        return 'bg-gray-400';
      case 'error':
        return 'bg-red-400';
      default:
        return 'bg-gray-600';
    }
  };

  const LocalMetadataPanel = () => {
    if (isLocalLoading) {
      return (
        <div className="flex-1 flex items-center justify-center text-gray-500">
          {t('common.loading')}
        </div>
      );
    }
    if (!installs.length) {
      return (
        <div className="flex-1 flex flex-col items-center justify-center text-gray-500 p-8">
          <div className="text-4xl mb-3">📁</div>
          <p>{t('metadataSearch.noInstall')}</p>
        </div>
      );
    }
    if (localError) {
      return (
        <div className="flex-1 flex items-center justify-center text-red-400 p-8">
          {localError}
        </div>
      );
    }
    const currentCover = coverUrl(currentGame.cover_image);
    const foundCover = localScanned?.cover_candidates[0] ? coverUrl(localScanned.cover_candidates[0]) : null;
    return (
      <div className="flex-1 overflow-y-auto p-6">
        <div className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-4">
          {t('metadataSearch.localHint')}
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-4">
            <div className="text-sm font-medium text-gray-300 border-b border-surface-100 pb-2">
              {t('metadataSearch.current')}
            </div>
            <ComparisonRow label={t('metadataSelect.title')} value={currentGame.title} />
            <ComparisonRow label={t('metadataSelect.developer')} value={currentGame.developer} />
            <ComparisonRow label={t('metadataSelect.description')} value={currentGame.description} />
            <ComparisonRow label={t('metadataSearch.executable')} value={currentGame.executable_path} />
            {currentCover && (
              <div className="aspect-[2/3] bg-surface-100 rounded-lg overflow-hidden max-w-[160px]">
                <img src={currentCover} alt="" className="w-full h-full object-cover" />
              </div>
            )}
          </div>
          <div className="space-y-4">
            <div className="text-sm font-medium text-accent border-b border-surface-100 pb-2">
              {t('metadataSearch.foundLocal')}
            </div>
            <ComparisonRow label={t('metadataSelect.title')} value={localScanned?.title} />
            <ComparisonRow label={t('metadataSelect.developer')} value={localScanned?.exe_metadata?.company_name} />
            <ComparisonRow label={t('metadataSelect.description')} value={localScanned?.exe_metadata?.file_description} />
            <ComparisonRow label={t('metadataSearch.executable')} value={localScanned?.executable} />
            {foundCover && (
              <div className="aspect-[2/3] bg-surface-100 rounded-lg overflow-hidden max-w-[160px]">
                <img src={foundCover} alt="" className="w-full h-full object-cover" />
              </div>
            )}
          </div>
        </div>
      </div>
    );
  };

  if (!isOpen || !currentGame) return null;

  return (
    <div className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-4">
      <div className="bg-surface-300 rounded-xl w-full max-w-5xl max-h-[90vh] flex flex-col shadow-2xl ring-1 ring-white/10">
        {/* Header */}
        <div className="p-4 border-b border-surface-100 flex items-center justify-between flex-shrink-0 bg-surface-400 rounded-t-xl">
          <div>
            <h2 className="text-lg font-semibold text-white">
              {isUpdateMode
                ? t('metadataSearch.updateTitle')
                : isBatch
                  ? t('metadataSearch.batchTitle', { current: currentIndex + 1, total: games.length })
                  : t('metadataSearch.title')}
            </h2>
            <p className="text-sm text-gray-400">
              {t('metadataSearch.gameName', { name: currentGame.title })}
            </p>
          </div>
          <button
            onClick={handleClose}
            className="text-gray-400 hover:text-white transition-colors w-8 h-8 flex items-center justify-center rounded-full hover:bg-white/10"
          >
            ✕
          </button>
        </div>

        {error && (
          <div className="mx-6 mt-4 p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm flex items-center gap-2">
            ⚠️ {error}
          </div>
        )}

        {duplicateLink && (
          <div className="mx-6 mt-4 p-3 bg-warning/20 border border-warning/50 rounded-lg text-warning text-sm">
            <div className="flex items-center justify-between gap-2">
              <span>{t('metadataSearch.duplicateLink', { title: duplicateLink.game.title })}</span>
              {onOpenGame && (
                <button
                  onClick={() => {
                    onOpenGame(duplicateLink.game, duplicateLink.link);
                    onClose();
                  }}
                  className="btn btn-sm btn-primary"
                >
                  {t('dialog.addLinkResult.openCard')}
                </button>
              )}
            </div>
          </div>
        )}

        {isUpdateMode && (
          <div className="flex border-b border-surface-100 flex-shrink-0">
            <button
              type="button"
              onClick={() => setActiveTab('local')}
              className={`flex-1 px-4 py-2 text-sm font-medium transition-colors ${activeTab === 'local' ? 'text-accent border-b-2 border-accent' : 'text-gray-400 hover:text-white'}`}
            >
              {t('metadataSearch.localTab')}
            </button>
            <button
              type="button"
              onClick={() => setActiveTab('internet')}
              className={`flex-1 px-4 py-2 text-sm font-medium transition-colors ${activeTab === 'internet' ? 'text-accent border-b-2 border-accent' : 'text-gray-400 hover:text-white'}`}
            >
              {t('metadataSearch.internetTab')}
            </button>
          </div>
        )}

        {/* Content */}
        <div className="flex flex-1 overflow-hidden min-h-0">
          {activeTab === 'internet' ? (
            <>
              {/* Left: Search & Results */}
          <div className="w-[420px] flex flex-col border-r border-surface-100">
            {/* Search controls */}
            <div className="p-4 border-b border-surface-100 space-y-3">
              <div className="flex gap-2">
                <input
                  type="text"
                  value={urlInput}
                  onChange={e => setUrlInput(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && handleFetchByUrl()}
                  placeholder={t('metadataSearch.urlPlaceholder')}
                  className="flex-1 px-3 py-2 bg-surface-200 rounded-lg text-sm focus:ring-1 focus:ring-accent outline-none"
                />
                <button
                  onClick={handleFetchByUrl}
                  disabled={!urlInput.trim() || isUrlFetching}
                  className="btn btn-sm btn-primary px-3"
                >
                  {isUrlFetching ? t('common.loading') : t('metadataSearch.fetchByUrl')}
                </button>
              </div>
              {urlError && (
                <div className="text-xs text-red-400">{urlError}</div>
              )}

              <div className="flex gap-2">
                <input
                  type="text"
                  value={query}
                  onChange={e => setQuery(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && runSearch()}
                  placeholder={t('metadataSearch.placeholder')}
                  className="flex-1 px-3 py-2 bg-surface-200 rounded-lg text-sm focus:ring-1 focus:ring-accent outline-none"
                />
                <button
                  onClick={runSearch}
                  disabled={!query.trim() || Object.values(sourceStatus).some(s => s === 'loading')}
                  className="btn btn-sm btn-primary px-3"
                >
                  {Object.values(sourceStatus).some(s => s === 'loading')
                    ? t('common.loading')
                    : t('metadataSearch.search')}
                </button>
              </div>

              <div className="flex gap-4 text-xs text-gray-400">
                {(Object.keys(includeSources) as (keyof SourceState)[]).map(source => (
                  <label key={source} className="flex items-center gap-1.5 cursor-pointer hover:text-white select-none">
                    <input
                      type="checkbox"
                      checked={includeSources[source]}
                      onChange={e => setIncludeSources(prev => ({ ...prev, [source]: e.target.checked }))}
                      className="rounded bg-surface-300 border-none text-accent focus:ring-0"
                    />
                    {sourceLabel(source)}
                  </label>
                ))}
              </div>

              {/* Source status */}
              <div className="space-y-1.5">
                {(Object.keys(sourceStatus) as (keyof SourceStatusMap)[]).map(source => (
                  <div key={source} className="flex items-center gap-2 text-xs">
                    <span className={`w-2 h-2 rounded-full ${statusDot(sourceStatus[source])}`} />
                    <span className="text-gray-300">{statusText(source)}</span>
                  </div>
                ))}
              </div>
            </div>

            {/* Results list */}
            <div className="flex-1 overflow-y-auto p-2 space-y-2">
              {results.length === 0 && (
                <div className="p-6 text-center text-gray-500 text-sm">
                  {Object.values(sourceStatus).some(s => s === 'loading')
                    ? t('metadataSearch.waitingForResults')
                    : t('metadataSearch.noResults')}
                </div>
              )}
              {results.map(result => (
                <div
                  key={`${result.source}-${result.id}`}
                  onClick={() => handleSelectResult(result)}
                  className={`flex gap-3 p-3 rounded-xl cursor-pointer transition-all border ${
                    selectedResult?.id === result.id && selectedResult?.source === result.source
                      ? 'bg-accent/20 border-accent/50'
                      : 'bg-surface-200/50 border-transparent hover:bg-surface-200 hover:border-surface-100'
                  }`}
                >
                  <div className="w-14 h-20 bg-black/20 rounded-lg overflow-hidden flex-shrink-0">
                    {result.cover_url ? (
                      <img
                        src={result.cover_url}
                        alt=""
                        className="w-full h-full object-cover"
                        onError={e => { e.currentTarget.style.display = 'none'; }}
                      />
                    ) : (
                      <div className="w-full h-full flex items-center justify-center text-xs opacity-30">?</div>
                    )}
                  </div>
                  <div className="flex-1 min-w-0 flex flex-col justify-center">
                    <div className="font-medium text-sm text-gray-100 truncate">{result.name}</div>
                    {result.url && (
                      <a
                        href={result.url}
                        target="_blank"
                        rel="noreferrer"
                        onClick={e => e.stopPropagation()}
                        className="text-xs text-accent truncate hover:underline"
                      >
                        {result.url}
                      </a>
                    )}
                    <div className="text-xs text-gray-500 line-clamp-1 mb-1">
                      {result.description || t('metadataSearch.noDescription')}
                    </div>
                    <div className="flex items-center gap-2 mt-auto">
                      <span
                        className={`px-1.5 py-0.5 rounded text-[10px] uppercase font-bold tracking-wider ${
                          result.source === 'steam'
                            ? 'bg-[#1b2838] text-[#66c0f4] border border-[#66c0f4]/30'
                            : 'bg-[#fa5c5c]/10 text-[#fa5c5c] border border-[#fa5c5c]/30'
                        }`}
                      >
                        {result.source}
                      </span>
                      {result.developer && (
                        <span className="text-xs text-gray-500 truncate">{result.developer}</span>
                      )}
                      {result.screenshots && result.screenshots.length > 0 && (
                        <img
                          src={result.screenshots[0]}
                          alt=""
                          className="w-8 h-6 rounded object-cover bg-black/20 flex-shrink-0"
                          onError={e => { e.currentTarget.style.display = 'none'; }}
                        />
                      )}

                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Right: Preview & field selection */}
          <div className="flex-1 flex flex-col min-w-0">
            <div className="flex-1 overflow-y-auto p-6">
              {!selectedResult ? (
                <div className="h-full flex flex-col items-center justify-center text-gray-500">
                  <div className="text-4xl mb-3">🔍</div>
                  <p>{t('metadataSearch.selectResult')}</p>
                </div>
              ) : (
                <div className="space-y-6">
                  {/* Result header */}
                  <div className="flex gap-4">
                    <div className="w-28 h-40 bg-black/20 rounded-xl overflow-hidden flex-shrink-0 shadow-lg">
                      {selectedResult.cover_url ? (
                        <img
                          src={selectedResult.cover_url}
                          alt=""
                          className="w-full h-full object-cover"
                          onError={e => { e.currentTarget.style.display = 'none'; }}
                        />
                      ) : (
                        <div className="w-full h-full flex items-center justify-center text-3xl opacity-30">?</div>
                      )}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-xs text-gray-400 mb-1">{t('metadataSearch.foundOn', { source: sourceLabel(selectedResult.source as keyof SourceState) })}</div>
                      <h3 className="text-xl font-bold text-white mb-1">{selectedResult.name}</h3>
                      {selectedResult.developer && (
                        <p className="text-sm text-gray-400 mb-2">{selectedResult.developer}</p>
                      )}
                      {selectedResult.url && (
                        <a
                          href={selectedResult.url}
                          target="_blank"
                          rel="noreferrer"
                          className="text-xs text-accent hover:underline inline-flex items-center gap-1"
                        >
                          {t('metadataSearch.openPage')} ↗
                        </a>
                      )}
                      <p className="text-xs text-gray-500 mt-2">
                        {t('metadataSearch.linkWillBeAdded')}
                      </p>
                    </div>
                  </div>

                  {/* Screenshots preview */}
                  {selectedResult.screenshots && selectedResult.screenshots.length > 0 && (
                    <div>
                      <div className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-2">
                        {t('details.screenshots')}
                      </div>
                      <div className="flex gap-2 overflow-x-auto pb-2">
                        {selectedResult.screenshots.map((url, idx) => (
                          <img
                            key={idx}
                            src={url}
                            alt=""
                            className="h-24 rounded-lg object-cover bg-black/20 flex-shrink-0"
                            onError={e => { e.currentTarget.style.display = 'none'; }}
                          />
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Fields to update */}
                  <div className="space-y-2">
                    <div className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-2">
                      {t('metadataSelect.fields')}
                    </div>

                    <FieldToggle
                      label={t('metadataSelect.title')}
                      value={selectedResult.name}
                      checked={fields.title}
                      onChange={() => handleToggleField('title')}
                    />

                    {selectedResult.developer && (
                      <FieldToggle
                        label={t('metadataSelect.developer')}
                        value={selectedResult.developer}
                        checked={fields.developer}
                        onChange={() => handleToggleField('developer')}
                      />
                    )}

                    {selectedResult.publisher && (
                      <FieldToggle
                        label={t('metadataSelect.publisher')}
                        value={selectedResult.publisher}
                        checked={fields.publisher}
                        onChange={() => handleToggleField('publisher')}
                      />
                    )}

                    {selectedResult.description && (
                      <FieldToggle
                        label={t('metadataSelect.description')}
                        value={selectedResult.description}
                        checked={fields.description}
                        onChange={() => handleToggleField('description')}
                      />
                    )}

                    {selectedResult.cover_url && (
                      <FieldToggle
                        label={t('metadataSelect.cover')}
                        value={selectedResult.cover_url}
                        checked={fields.cover}
                        onChange={() => handleToggleField('cover')}
                      />
                    )}
                  </div>

                  {((selectedResult.genres?.length ?? 0) + (selectedResult.tags?.length ?? 0)) > 0 && (
                    <div className="space-y-2">
                      <div className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-2">
                        {t('metadataSelect.classifications')}
                      </div>
                      {selectedResult.genres && selectedResult.genres.length > 0 && (
                        <div className="text-sm text-gray-300">
                          <span className="text-gray-500">{t('metadataSelect.genres')}:</span>{' '}
                          {selectedResult.genres.join(', ')}
                        </div>
                      )}
                      {selectedResult.tags && selectedResult.tags.length > 0 && (
                        <div className="text-sm text-gray-300">
                          <span className="text-gray-500">{t('metadataSelect.tags')}:</span>{' '}
                          {selectedResult.tags.join(', ')}
                        </div>
                      )}
                    </div>
                  )}

                  {selectedResult.external_links && selectedResult.external_links.length > 0 && (
                    <div className="space-y-2">
                      <div className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-2">
                        {t('metadataSelect.externalLinks')}
                      </div>
                      <div className="space-y-1">
                        {selectedResult.external_links.map(link => (
                          <div key={link.url} className="text-sm text-gray-300 truncate">
                            <a href={link.url} target="_blank" rel="noreferrer" className="text-accent hover:underline">{link.label || link.url}</a>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
            </>
          ) : (
            <LocalMetadataPanel />
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-surface-100 flex items-center justify-between bg-surface-400 rounded-b-xl">
          <div className="flex gap-2">
            {isBatch && (
              <>
                <button
                  onClick={goPrev}
                  disabled={currentIndex === 0}
                  className="btn btn-secondary btn-sm disabled:opacity-50"
                >
                  ← {t('metadataSearch.prev')}
                </button>
                <button
                  onClick={handleSkip}
                  className="btn btn-secondary btn-sm"
                >
                  {t('metadataSearch.skip')}
                </button>
              </>
            )}
          </div>

          <div className="flex gap-3">
            <button onClick={handleClose} className="btn btn-secondary">
              {t('common.cancel')}
            </button>
            <button
              onClick={handleApply}
              disabled={
                isApplying ||
                (isUpdateMode
                  ? activeTab === 'local'
                    ? !installs.length || !localScanned
                    : !selectedResult
                  : !selectedResult)
              }
              className="btn btn-primary disabled:opacity-50"
            >
              {isApplying
                ? t('common.loading')
                : isBatch
                  ? t('metadataSearch.applyAndNext')
                  : t('metadataSearch.apply')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

interface FieldToggleProps {
  label: string;
  value: string;
  checked: boolean;
  onChange: () => void;
}

function FieldToggle({ label, value, checked, onChange }: FieldToggleProps) {
  return (
    <label className="flex items-center gap-3 p-3 bg-surface-200 rounded-lg cursor-pointer hover:bg-surface-100 transition-colors">
      <input
        type="checkbox"
        checked={checked}
        onChange={onChange}
        className="rounded bg-surface-400 border-none text-accent w-5 h-5 focus:ring-0"
      />
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-gray-200">{label}</div>
        <div className="text-xs text-gray-500 truncate">{value}</div>
      </div>
    </label>
  );
}

interface ComparisonRowProps {
  label: string;
  value: string | null | undefined;
}

function ComparisonRow({ label, value }: ComparisonRowProps) {
  return (
    <div>
      <div className="text-xs text-gray-500 mb-0.5">{label}</div>
      <div className={`text-sm truncate ${value ? 'text-gray-200' : 'text-gray-600 italic'}`}>
        {value || '—'}
      </div>
    </div>
  );
}
