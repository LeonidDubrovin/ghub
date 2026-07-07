import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import type { Game, MetadataSearchResult, ScannedGame, Install, GameLink } from '../types';
import { createLoggerForComponent } from '../lib/logger';

interface MetadataUpdateDialogProps {
  game: Game;
  isOpen: boolean;
  onClose: () => void;
  onSave: () => void;
}

type Section = 'local' | 'current' | 'find';
type SourceType = 'steam' | 'itch';
type SourceStatus = 'idle' | 'loading' | 'done' | 'empty' | 'error';

interface FieldSelection {
  title: boolean;
  description: boolean;
  developer: boolean;
  publisher: boolean;
  cover: boolean;
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

const defaultFields = (result: MetadataSearchResult): FieldSelection => ({
  title: true,
  description: !!result.description,
  developer: !!result.developer,
  publisher: !!result.publisher,
  cover: !!result.cover_url,
});

export default function MetadataUpdateDialog({
  game,
  isOpen,
  onClose,
  onSave,
}: MetadataUpdateDialogProps) {
  const logger = useMemo(() => createLoggerForComponent('MetadataUpdateDialog'), []);
  const { t } = useTranslation();

  const [isInitializing, setIsInitializing] = useState(true);
  const [activeSection, setActiveSection] = useState<Section>('find');
  const [installs, setInstalls] = useState<Install[]>([]);
  const [gameLinks, setGameLinks] = useState<GameLink[]>([]);

  // Local section
  const [localScanned, setLocalScanned] = useState<ScannedGame | null>(null);
  const [isLocalLoading, setIsLocalLoading] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [isApplyingLocal, setIsApplyingLocal] = useState(false);

  // Current source link section
  const [currentLink, setCurrentLink] = useState<GameLink | null>(null);
  const [currentLinkResult, setCurrentLinkResult] = useState<MetadataSearchResult | null>(null);
  const [currentLinkFields, setCurrentLinkFields] = useState<FieldSelection>({
    title: true,
    description: true,
    developer: true,
    publisher: true,
    cover: true,
  });
  const [isCurrentLinkLoading, setIsCurrentLinkLoading] = useState(false);
  const [currentLinkError, setCurrentLinkError] = useState<string | null>(null);
  const [isApplyingCurrentLink, setIsApplyingCurrentLink] = useState(false);

  // Add by URL section
  const [urlInput, setUrlInput] = useState('');
  const [urlResult, setUrlResult] = useState<MetadataSearchResult | null>(null);
  const [urlFields, setUrlFields] = useState<FieldSelection>({
    title: true,
    description: true,
    developer: true,
    publisher: true,
    cover: true,
  });
  const [isUrlLoading, setIsUrlLoading] = useState(false);
  const [urlError, setUrlError] = useState<string | null>(null);
  const [isApplyingUrl, setIsApplyingUrl] = useState(false);

  // Search by title section
  const [query, setQuery] = useState(game.title);
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
  const [searchFields, setSearchFields] = useState<FieldSelection>({
    title: true,
    description: true,
    developer: true,
    publisher: true,
    cover: true,
  });
  const [isApplyingSearch, setIsApplyingSearch] = useState(false);

  const inferSourceType = useCallback((url: string): SourceType | null => {
    const lower = url.toLowerCase();
    if (lower.includes('itch.io')) return 'itch';
    if (lower.includes('steampowered.com/app/') || lower.includes('store.steampowered.com/app/')) return 'steam';
    return null;
  }, []);

  const coverUrl = useCallback((path: string | null) => {
    if (!path) return null;
    if (path.startsWith('http')) return path;
    try { return convertFileSrc(path); } catch { return null; }
  }, []);

  const applyMetadata = useCallback(async (result: MetadataSearchResult, fields: FieldSelection) => {
    await invoke('update_game', {
      request: {
        id: game.id,
        title: fields.title ? result.name : null,
        description: fields.description && result.description ? result.description : null,
        developer: fields.developer && result.developer ? result.developer : null,
        publisher: fields.publisher && result.publisher ? result.publisher : null,
        cover_image: fields.cover && result.cover_url ? result.cover_url : null,
      },
    });

    if (result.url) {
      try {
        await invoke('add_game_link', {
          gameId: game.id,
          url: result.url,
          title: result.name,
          sourceType: result.source,
          downloadStatus: null,
          queueSpace: null,
        });
      } catch (linkErr) {
        logger.warn('Failed to add source link:', linkErr);
      }
    }
  }, [game.id, logger]);

  // Local section handlers
  const scanLocal = useCallback(async (installList?: Install[]) => {
    const list = installList || installs;
    if (!list.length) return;
    setIsLocalLoading(true);
    setLocalError(null);
    try {
      const data = await invoke<ScannedGame | null>('scan_local_metadata', { gameId: game.id });
      setLocalScanned(data);
      if (!data) {
        setLocalError(t('metadataUpdate.noLocalData'));
      }
    } catch (err) {
      logger.error('Local scan failed:', err);
      setLocalError(String(err));
      setLocalScanned(null);
    } finally {
      setIsLocalLoading(false);
    }
  }, [game.id, installs, logger, t]);

  const applyLocal = useCallback(async () => {
    setIsApplyingLocal(true);
    setLocalError(null);
    try {
      await invoke('refresh_game_from_local', { gameId: game.id });
      onSave();
      onClose();
    } catch (err) {
      logger.error('Apply local metadata failed:', err);
      setLocalError(String(err));
    } finally {
      setIsApplyingLocal(false);
    }
  }, [game.id, onClose, onSave, logger]);

  // Current link section handlers
  const fetchCurrentLink = useCallback(async (link: GameLink) => {
    if (!link.source_type) return;
    setIsCurrentLinkLoading(true);
    setCurrentLinkError(null);
    setCurrentLinkResult(null);
    try {
      const result = await invoke<MetadataSearchResult | null>('fetch_metadata_by_url_command', {
        sourceType: link.source_type,
        url: link.url,
      });
      if (result) {
        setCurrentLinkResult(result);
        setCurrentLinkFields(defaultFields(result));
      } else {
        setCurrentLinkError(t('metadataUpdate.noExactMetadata'));
      }
    } catch (err) {
      logger.error('Fetch current link metadata failed:', err);
      setCurrentLinkError(String(err));
    } finally {
      setIsCurrentLinkLoading(false);
    }
  }, [logger, t]);

  // Stable refs to avoid re-running the initial load effect when callbacks change.
  const scanLocalRef = useRef(scanLocal);
  scanLocalRef.current = scanLocal;
  const fetchCurrentLinkRef = useRef(fetchCurrentLink);
  fetchCurrentLinkRef.current = fetchCurrentLink;

  const applyCurrentLink = useCallback(async () => {
    if (!currentLinkResult) return;
    setIsApplyingCurrentLink(true);
    setCurrentLinkError(null);
    try {
      await applyMetadata(currentLinkResult, currentLinkFields);
      onSave();
      onClose();
    } catch (err) {
      logger.error('Apply current link metadata failed:', err);
      setCurrentLinkError(String(err));
    } finally {
      setIsApplyingCurrentLink(false);
    }
  }, [currentLinkFields, currentLinkResult, onClose, onSave, applyMetadata, logger]);

  // URL section handlers
  const handleFetchUrl = useCallback(async () => {
    const url = urlInput.trim();
    if (!url) return;
    const sourceType = inferSourceType(url);
    if (!sourceType) {
      setUrlError(t('metadataUpdate.unknownUrl'));
      return;
    }
    setIsUrlLoading(true);
    setUrlError(null);
    setUrlResult(null);
    try {
      const result = await invoke<MetadataSearchResult | null>('fetch_metadata_by_url_command', {
        sourceType,
        url,
      });
      if (result) {
        setUrlResult(result);
        setUrlFields(defaultFields(result));
      } else {
        setUrlError(t('metadataUpdate.noExactMetadata'));
      }
    } catch (err) {
      logger.error('Fetch URL metadata failed:', err);
      setUrlError(String(err));
    } finally {
      setIsUrlLoading(false);
    }
  }, [urlInput, inferSourceType, logger, t]);

  const applyUrl = useCallback(async () => {
    if (!urlResult) return;
    setIsApplyingUrl(true);
    setUrlError(null);
    try {
      await applyMetadata(urlResult, urlFields);
      onSave();
      onClose();
    } catch (err) {
      logger.error('Apply URL metadata failed:', err);
      setUrlError(String(err));
    } finally {
      setIsApplyingUrl(false);
    }
  }, [urlFields, urlResult, onClose, onSave, applyMetadata, logger]);

  // Search section handlers
  const searchSource = useCallback(async (source: SourceType) => {
    const trimmed = query.trim();
    if (!trimmed) return;
    setSourceStatus(prev => ({ ...prev, [source]: 'loading' }));
    setSourceErrors(prev => ({ ...prev, [source]: null }));
    try {
      const sourceResults = await invoke<MetadataSearchResult[]>('search_game_metadata', {
        query: trimmed,
        sources: [source],
      });
      setResults(prev => {
        const without = prev.filter(r => r.source !== source);
        return [...without, ...sourceResults].sort((a, b) => {
          if (a.source === b.source) return a.name.localeCompare(b.name);
          return a.source === 'steam' ? -1 : 1;
        });
      });
      setSourceStatus(prev => ({ ...prev, [source]: sourceResults.length > 0 ? 'done' : 'empty' }));
    } catch (err) {
      logger.error(`${source} search failed:`, err);
      setSourceStatus(prev => ({ ...prev, [source]: 'error' }));
      setSourceErrors(prev => ({ ...prev, [source]: String(err) }));
    }
  }, [query, logger]);

  const runSearch = useCallback(() => {
    setResults([]);
    setSelectedResult(null);
    setSourceStatus({ steam: 'idle', itch: 'idle' });
    setSourceErrors({ steam: null, itch: null });
    (Object.keys(includeSources) as SourceType[]).forEach(source => {
      if (includeSources[source]) searchSource(source);
    });
  }, [includeSources, searchSource]);

  const handleSelectResult = useCallback(async (result: MetadataSearchResult) => {
    setSelectedResult(result);
    setSearchFields(defaultFields(result));
    if (result.url) {
      try {
        const exact = await invoke<MetadataSearchResult | null>('fetch_metadata_by_url_command', {
          sourceType: result.source,
          url: result.url,
        });
        if (exact) {
          setSelectedResult(exact);
          setSearchFields(defaultFields(exact));
        }
      } catch (err) {
        logger.warn('Exact metadata fetch for selected result failed:', err);
      }
    }
  }, [logger]);

  const applySearchResult = useCallback(async () => {
    if (!selectedResult) return;
    setIsApplyingSearch(true);
    try {
      await applyMetadata(selectedResult, searchFields);
      onSave();
      onClose();
    } catch (err) {
      logger.error('Apply search result failed:', err);
      setSourceErrors(prev => ({ ...prev, [selectedResult.source as SourceType]: String(err) }));
    } finally {
      setIsApplyingSearch(false);
    }
  }, [selectedResult, searchFields, onClose, onSave, applyMetadata, logger]);

  // Initial load: determine which section to open and trigger the first scan/fetch.
  // Uses refs for the callbacks so changing them does not restart this effect.
  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    setIsInitializing(true);
    const load = async () => {
      try {
        const [installData, linkData] = await Promise.all([
          invoke<Install[]>('get_game_installs', { gameId: game.id }),
          invoke<GameLink[]>('get_game_links', { gameId: game.id }),
        ]);
        if (cancelled) return;
        setInstalls(installData);
        setGameLinks(linkData);
        const typedLink = linkData.find(l => l.source_type === 'itch' || l.source_type === 'steam');
        if (installData.length > 0) {
          setActiveSection('local');
          scanLocalRef.current(installData);
        } else if (typedLink) {
          setActiveSection('current');
          setCurrentLink(typedLink);
          fetchCurrentLinkRef.current(typedLink);
        } else {
          setActiveSection('find');
          setQuery(game.title);
        }
      } catch (err) {
        if (!cancelled) {
          logger.error('Failed to load game context:', err);
        }
      } finally {
        if (!cancelled) {
          setIsInitializing(false);
        }
      }
    };
    load();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, game.id, game.title]);

  // Helpers
  const sourceLabel = (source: SourceType) => t(`sources.${source}`);
  const sourceIcon = (source: string | null) => {
    switch (source) {
      case 'steam': return '🎮';
      case 'itch': return '🎨';
      default: return '🔗';
    }
  };

  const statusText = (source: SourceType) => {
    switch (sourceStatus[source]) {
      case 'loading': return t('metadataSearch.searching', { source: sourceLabel(source) });
      case 'done': return t('metadataSearch.found', { source: sourceLabel(source), count: results.filter(r => r.source === source).length });
      case 'empty': return t('metadataSearch.empty', { source: sourceLabel(source) });
      case 'error': return t('metadataSearch.error', { source: sourceLabel(source), error: sourceErrors[source] || '' });
      default: return t('metadataSearch.waiting', { source: sourceLabel(source) });
    }
  };

  const statusDot = (status: SourceStatus) => {
    switch (status) {
      case 'loading': return 'bg-yellow-400 animate-pulse';
      case 'done': return 'bg-green-400';
      case 'empty': return 'bg-gray-400';
      case 'error': return 'bg-red-400';
      default: return 'bg-gray-600';
    }
  };

  const handleToggleSearchField = (field: keyof FieldSelection) => {
    setSearchFields(prev => ({ ...prev, [field]: !prev[field] }));
  };
  const handleToggleUrlField = (field: keyof FieldSelection) => {
    setUrlFields(prev => ({ ...prev, [field]: !prev[field] }));
  };
  const handleToggleCurrentLinkField = (field: keyof FieldSelection) => {
    setCurrentLinkFields(prev => ({ ...prev, [field]: !prev[field] }));
  };

  const typedLink = gameLinks.find(l => l.source_type === 'itch' || l.source_type === 'steam');

  if (!isOpen) return null;

  // Sub-sections
  const SidebarButton = ({
    active,
    onClick,
    disabled,
    icon,
    label,
    description,
  }: {
    active: boolean;
    onClick: () => void;
    disabled?: boolean;
    icon: string;
    label: string;
    description: string;
  }) => (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`text-left w-full p-3 rounded-xl transition-colors border ${
        active
          ? 'bg-accent/20 border-accent/50'
          : disabled
            ? 'opacity-50 cursor-not-allowed border-transparent'
            : 'hover:bg-surface-300 border-transparent'
      }`}
    >
      <div className="flex items-center gap-2">
        <span className="text-lg">{icon}</span>
        <span className={`text-sm font-medium ${active ? 'text-white' : 'text-gray-200'}`}>{label}</span>
      </div>
      <div className="text-xs text-gray-500 mt-1 truncate pl-7">{description}</div>
    </button>
  );

  const ResultPreview = ({ result, fields, onToggle }: {
    result: MetadataSearchResult;
    fields: FieldSelection;
    onToggle: (field: keyof FieldSelection) => void;
  }) => (
    <div className="space-y-6">
      <div className="flex gap-4">
        <div className="w-28 h-40 bg-black/20 rounded-xl overflow-hidden flex-shrink-0 shadow-lg">
          {result.cover_url ? (
            <img
              src={result.cover_url}
              alt=""
              className="w-full h-full object-cover"
              onError={e => { e.currentTarget.style.display = 'none'; }}
            />
          ) : (
            <div className="w-full h-full flex items-center justify-center text-3xl opacity-30">?</div>
          )}
        </div>
        <div className="flex-1 min-w-0">
          <div className="text-xs text-gray-400 mb-1">{t('metadataSearch.foundOn', { source: sourceLabel(result.source as SourceType) })}</div>
          <h3 className="text-xl font-bold text-white mb-1">{result.name}</h3>
          {result.developer && <p className="text-sm text-gray-400 mb-2">{result.developer}</p>}
          {result.url && (
            <a href={result.url} target="_blank" rel="noreferrer" className="text-xs text-accent hover:underline inline-flex items-center gap-1">
              {t('metadataSearch.openPage')} ↗
            </a>
          )}
          <p className="text-xs text-gray-500 mt-2">{t('metadataSearch.linkWillBeAdded')}</p>
        </div>
      </div>

      <div className="space-y-2">
        <div className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-2">{t('metadataSelect.fields')}</div>
        <FieldToggle label={t('metadataSelect.title')} value={result.name} checked={fields.title} onChange={() => onToggle('title')} />
        {result.developer && <FieldToggle label={t('metadataSelect.developer')} value={result.developer} checked={fields.developer} onChange={() => onToggle('developer')} />}
        {result.publisher && <FieldToggle label={t('metadataSelect.publisher')} value={result.publisher} checked={fields.publisher} onChange={() => onToggle('publisher')} />}
        {result.description && <FieldToggle label={t('metadataSelect.description')} value={result.description} checked={fields.description} onChange={() => onToggle('description')} />}
        {result.cover_url && <FieldToggle label={t('metadataSelect.cover')} value={result.cover_url} checked={fields.cover} onChange={() => onToggle('cover')} />}
      </div>
    </div>
  );

  const LocalSection = () => {
    if (!installs.length) {
      return (
        <div className="h-full flex flex-col items-center justify-center text-gray-500 p-8">
          <div className="text-4xl mb-3">📁</div>
          <p className="text-center">{t('metadataUpdate.noInstall')}</p>
        </div>
      );
    }
    const install = installs[0];
    return (
      <div className="space-y-6">
        <div>
          <h3 className="text-lg font-semibold text-white mb-1">{t('metadataUpdate.localTitle')}</h3>
          <p className="text-sm text-gray-400">{t('metadataUpdate.localDescription')}</p>
        </div>
        <div className="p-3 bg-surface-200 rounded-lg text-sm text-gray-300">
          <span className="text-gray-500">{t('metadataUpdate.installPath')}:</span>{' '}
          <span className="font-mono text-xs break-all">{install.install_path}</span>
        </div>
        <button
          onClick={() => scanLocal()}
          disabled={isLocalLoading}
          className="btn btn-primary"
        >
          {isLocalLoading ? t('common.loading') : t('metadataUpdate.scanLocal')}
        </button>

        {localError && (
          <div className="p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm">
            {localError}
          </div>
        )}

        {localScanned && (
          <div className="space-y-4">
            <div className="text-xs font-bold text-gray-500 uppercase tracking-wider">{t('metadataUpdate.scanResult')}</div>
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-3">
                <div className="text-sm font-medium text-gray-300 border-b border-surface-100 pb-2">{t('metadataUpdate.current')}</div>
                <ComparisonRow label={t('metadataSelect.title')} value={game.title} />
                <ComparisonRow label={t('metadataSelect.developer')} value={game.developer} />
                <ComparisonRow label={t('metadataSelect.description')} value={game.description} />
                <ComparisonRow label={t('metadataUpdate.executable')} value={game.executable_path} />
                {coverUrl(game.cover_image) && (
                  <div className="aspect-[2/3] bg-surface-100 rounded-lg overflow-hidden max-w-[140px]">
                    <img src={coverUrl(game.cover_image)!} alt="" className="w-full h-full object-cover" />
                  </div>
                )}
              </div>
              <div className="space-y-3">
                <div className="text-sm font-medium text-accent border-b border-surface-100 pb-2">{t('metadataUpdate.found')}</div>
                <ComparisonRow label={t('metadataSelect.title')} value={localScanned.title} />
                <ComparisonRow label={t('metadataSelect.developer')} value={localScanned.exe_metadata?.company_name} />
                <ComparisonRow label={t('metadataSelect.description')} value={localScanned.exe_metadata?.file_description} />
                <ComparisonRow label={t('metadataUpdate.executable')} value={localScanned.executable} />
                {localScanned.cover_candidates[0] && coverUrl(localScanned.cover_candidates[0]) && (
                  <div className="aspect-[2/3] bg-surface-100 rounded-lg overflow-hidden max-w-[140px]">
                    <img src={coverUrl(localScanned.cover_candidates[0])!} alt="" className="w-full h-full object-cover" />
                  </div>
                )}
              </div>
            </div>
            <button
              onClick={applyLocal}
              disabled={isApplyingLocal}
              className="btn btn-primary"
            >
              {isApplyingLocal ? t('common.loading') : t('metadataUpdate.applyLocal')}
            </button>
          </div>
        )}
      </div>
    );
  };

  const CurrentSection = () => {
    if (!typedLink) {
      return (
        <div className="h-full flex flex-col items-center justify-center text-gray-500 p-8">
          <div className="text-4xl mb-3">🔗</div>
          <p className="text-center">{t('metadataUpdate.noCurrentLink')}</p>
        </div>
      );
    }
    return (
      <div className="space-y-6">
        <div>
          <h3 className="text-lg font-semibold text-white mb-1">{t('metadataUpdate.currentTitle')}</h3>
          <p className="text-sm text-gray-400">{t('metadataUpdate.currentDescription')}</p>
        </div>
        <div className="p-4 bg-surface-200 rounded-xl border border-surface-100">
          <div className="flex items-center gap-3 mb-2">
            <span className="text-2xl">{sourceIcon(typedLink.source_type)}</span>
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium text-white truncate">{typedLink.title || typedLink.url}</div>
              <a href={typedLink.url} target="_blank" rel="noreferrer" className="text-xs text-accent hover:underline truncate block">
                {typedLink.url}
              </a>
            </div>
          </div>
          <button
            onClick={currentLinkResult ? applyCurrentLink : () => fetchCurrentLink(typedLink)}
            disabled={isCurrentLinkLoading || isApplyingCurrentLink}
            className="btn btn-primary w-full"
          >
            {isCurrentLinkLoading
              ? t('common.loading')
              : currentLinkResult
                ? t('metadataUpdate.applyCurrentLink')
                : t('metadataUpdate.refreshCurrentLink')}
          </button>
        </div>

        {currentLinkError && (
          <div className="p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm">
            {currentLinkError}
          </div>
        )}

        {isCurrentLinkLoading && (
          <div className="text-sm text-gray-400">{t('common.loading')}</div>
        )}

        {currentLinkResult && (
          <div className="border-t border-surface-100 pt-4">
            <ResultPreview result={currentLinkResult} fields={currentLinkFields} onToggle={handleToggleCurrentLinkField} />
          </div>
        )}
      </div>
    );
  };

  const FindSection = () => (
    <div className="space-y-8">
      {/* Add by URL */}
      <div className="space-y-4">
        <div>
          <h3 className="text-lg font-semibold text-white mb-1">{t('metadataUpdate.addByUrlTitle')}</h3>
          <p className="text-sm text-gray-400">{t('metadataUpdate.addByUrlDescription')}</p>
        </div>
        <div className="flex gap-2">
          <input
            type="text"
            value={urlInput}
            onChange={e => setUrlInput(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleFetchUrl()}
            placeholder={t('metadataUpdate.urlPlaceholder')}
            className="flex-1 px-3 py-2 bg-surface-200 rounded-lg text-sm focus:ring-1 focus:ring-accent outline-none"
          />
          <button
            onClick={handleFetchUrl}
            disabled={!urlInput.trim() || isUrlLoading}
            className="btn btn-primary btn-sm px-3"
          >
            {isUrlLoading ? t('common.loading') : t('metadataUpdate.fetchUrl')}
          </button>
        </div>
        {urlError && (
          <div className="p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm">
            {urlError}
          </div>
        )}
        {urlResult && (
          <div className="border-t border-surface-100 pt-4">
            <ResultPreview result={urlResult} fields={urlFields} onToggle={handleToggleUrlField} />
            <div className="mt-4">
              <button
                onClick={applyUrl}
                disabled={isApplyingUrl}
                className="btn btn-primary"
              >
                {isApplyingUrl ? t('common.loading') : t('metadataUpdate.applyUrl')}
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="border-t border-surface-100" />

      {/* Search by title */}
      <div className="space-y-4">
        <div>
          <h3 className="text-lg font-semibold text-white mb-1">{t('metadataUpdate.searchTitle')}</h3>
          <p className="text-sm text-gray-400">{t('metadataUpdate.searchDescription')}</p>
        </div>
        <div className="flex gap-2">
          <input
            type="text"
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && runSearch()}
            placeholder={t('metadataUpdate.searchPlaceholder')}
            className="flex-1 px-3 py-2 bg-surface-200 rounded-lg text-sm focus:ring-1 focus:ring-accent outline-none"
          />
          <button
            onClick={runSearch}
            disabled={!query.trim() || Object.values(sourceStatus).some(s => s === 'loading')}
            className="btn btn-primary btn-sm px-3"
          >
            {Object.values(sourceStatus).some(s => s === 'loading') ? t('common.loading') : t('metadataSearch.search')}
          </button>
        </div>

        <div className="flex gap-4 text-xs text-gray-400">
          {(Object.keys(includeSources) as SourceType[]).map(source => (
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

        <div className="space-y-1.5">
          {(Object.keys(sourceStatus) as SourceType[]).map(source => (
            <div key={source} className="flex items-center gap-2 text-xs">
              <span className={`w-2 h-2 rounded-full ${statusDot(sourceStatus[source])}`} />
              <span className="text-gray-300">{statusText(source)}</span>
            </div>
          ))}
        </div>

        <div className="space-y-2">
          {results.length === 0 && (
            <div className="p-6 text-center text-gray-500 text-sm bg-surface-200/50 rounded-xl">
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
                  <img src={result.cover_url} alt="" className="w-full h-full object-cover" onError={e => { e.currentTarget.style.display = 'none'; }} />
                ) : (
                  <div className="w-full h-full flex items-center justify-center text-xs opacity-30">?</div>
                )}
              </div>
              <div className="flex-1 min-w-0 flex flex-col justify-center">
                <div className="font-medium text-sm text-gray-100 truncate">{result.name}</div>
                {result.url && (
                  <a href={result.url} target="_blank" rel="noreferrer" onClick={e => e.stopPropagation()} className="text-xs text-accent truncate hover:underline">
                    {result.url}
                  </a>
                )}
                <div className="text-xs text-gray-500 line-clamp-1 mb-1">{result.description || t('metadataSearch.noDescription')}</div>
                <div className="flex items-center gap-2 mt-auto">
                  <span className={`px-1.5 py-0.5 rounded text-[10px] uppercase font-bold tracking-wider ${
                    result.source === 'steam'
                      ? 'bg-[#1b2838] text-[#66c0f4] border border-[#66c0f4]/30'
                      : 'bg-[#fa5c5c]/10 text-[#fa5c5c] border border-[#fa5c5c]/30'
                  }`}>
                    {result.source}
                  </span>
                  {result.developer && <span className="text-xs text-gray-500 truncate">{result.developer}</span>}
                </div>
              </div>
            </div>
          ))}
        </div>

        {selectedResult && (
          <div className="border-t border-surface-100 pt-4">
            <ResultPreview result={selectedResult} fields={searchFields} onToggle={handleToggleSearchField} />
            <div className="mt-4">
              <button
                onClick={applySearchResult}
                disabled={isApplyingSearch}
                className="btn btn-primary"
              >
                {isApplyingSearch ? t('common.loading') : t('metadataUpdate.applySearch')}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );

  return (
    <div className="fixed inset-0 bg-black/70 backdrop-blur-sm flex items-center justify-center z-50 p-4">
      <div className="bg-surface-300 rounded-xl w-full max-w-5xl max-h-[90vh] flex flex-col shadow-2xl ring-1 ring-white/10">
        {/* Header */}
        <div className="p-4 border-b border-surface-100 flex items-center justify-between flex-shrink-0 bg-surface-400 rounded-t-xl">
          <div>
            <h2 className="text-lg font-semibold text-white">{t('metadataUpdate.title')}</h2>
            <p className="text-sm text-gray-400">{t('metadataUpdate.gameName', { name: game.title })}</p>
          </div>
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-white transition-colors w-8 h-8 flex items-center justify-center rounded-full hover:bg-white/10"
          >
            ✕
          </button>
        </div>

        {/* Body */}
        <div className="flex flex-1 overflow-hidden min-h-0">
          {/* Sidebar */}
          <div className="w-64 flex flex-col border-r border-surface-100 bg-surface-400/50 p-3 gap-2 flex-shrink-0 overflow-y-auto">
            <SidebarButton
              active={activeSection === 'local'}
              onClick={() => setActiveSection('local')}
              disabled={isInitializing || !installs.length}
              icon="📁"
              label={t('metadataUpdate.localSection')}
              description={installs.length ? t('metadataUpdate.localSectionDesc') : t('metadataUpdate.noInstall')}
            />
            <SidebarButton
              active={activeSection === 'current'}
              onClick={() => {
                setActiveSection('current');
                if (typedLink && !currentLink) {
                  setCurrentLink(typedLink);
                  fetchCurrentLink(typedLink);
                }
              }}
              disabled={isInitializing || !typedLink}
              icon="🔗"
              label={t('metadataUpdate.currentSection')}
              description={typedLink ? typedLink.title || typedLink.url : t('metadataUpdate.noCurrentLink')}
            />
            <SidebarButton
              active={activeSection === 'find'}
              onClick={() => setActiveSection('find')}
              disabled={isInitializing}
              icon="🔍"
              label={t('metadataUpdate.findSection')}
              description={t('metadataUpdate.findSectionDesc')}
            />
          </div>

          {/* Content */}
          <div className="flex-1 overflow-y-auto p-6">
            {isInitializing ? (
              <div className="h-full flex flex-col items-center justify-center text-gray-500">
                <div className="w-8 h-8 border-2 border-accent/30 border-t-accent rounded-full animate-spin mb-3" />
                <p className="text-sm">{t('common.loading')}</p>
              </div>
            ) : (
              <>
                {activeSection === 'local' && <LocalSection />}
                {activeSection === 'current' && <CurrentSection />}
                {activeSection === 'find' && <FindSection />}
              </>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-surface-100 flex justify-end bg-surface-400 rounded-b-xl">
          <button onClick={onClose} className="btn btn-secondary">
            {t('common.cancel')}
          </button>
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
