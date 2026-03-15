import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Game, MetadataSearchResult } from '../types';

interface BatchMetadataDialogProps {
  games: Game[];
  onClose: () => void;
  onSave: () => void;
}

interface GameMetadataEdit {
  game: Game;
  searchQuery: string;
  searchResults: MetadataSearchResult[];
  selectedResult: MetadataSearchResult | null;
  isSearching: boolean;
  editedTitle: string;
  editedDeveloper: string;
  editedPublisher: string;
  editedCover: string;
  editedDescription: string;
  status: 'pending' | 'searching' | 'ready' | 'saving' | 'saved' | 'error';
  error?: string;
}

type MetadataSource = 'itch' | 'steam' | 'manual';

export default function BatchMetadataDialog({ games, onClose, onSave }: BatchMetadataDialogProps) {
  const { t } = useTranslation();
  const [source, setSource] = useState<MetadataSource>('steam');
  const [currentIndex, setCurrentIndex] = useState(0);
  const [gameEdits, setGameEdits] = useState<GameMetadataEdit[]>([]);
  const [isSaving, setIsSaving] = useState(false);
  const autoSearch = true; // Could be made configurable later
  
  // Initialize game edits
  useEffect(() => {
    const edits = games.map(game => ({
      game,
      searchQuery: game.title,
      searchResults: [],
      selectedResult: null,
      isSearching: false,
      editedTitle: game.title,
      editedDeveloper: game.developer || '',
      editedPublisher: game.publisher || '',
      editedCover: game.cover_image || '',
      editedDescription: game.description || '',
      status: 'pending' as const,
    }));
    setGameEdits(edits);
    
    // Auto-search first game if enabled
    if (autoSearch && edits.length > 0) {
      searchMetadata(0, edits[0].searchQuery);
    }
  }, [games]);
  
  const searchMetadata = async (index: number, query: string) => {
    if (!query.trim()) return;
    
    setGameEdits(prev => {
      const updated = [...prev];
      updated[index] = { ...updated[index], isSearching: true, status: 'searching' };
      return updated;
    });
    
    try {
      const results = await invoke<MetadataSearchResult[]>('search_game_metadata', { 
        query, 
        sources: source === 'manual' ? [] : [source] 
      });
      
      setGameEdits(prev => {
        const updated = [...prev];
        updated[index] = { 
          ...updated[index], 
          isSearching: false, 
          searchResults: results,
          status: 'ready',
          // Auto-select first result if available
          selectedResult: results.length > 0 ? results[0] : null,
        };
        
        // Apply first result if available
        if (results.length > 0) {
          const r = results[0];
          updated[index].editedTitle = r.name;
          updated[index].editedDeveloper = r.developer || '';
          updated[index].editedPublisher = r.publisher || '';
          updated[index].editedCover = r.cover_url || '';
          updated[index].editedDescription = r.summary || '';
        }
        
        return updated;
      });
    } catch (err) {
      setGameEdits(prev => {
        const updated = [...prev];
        updated[index] = { 
          ...updated[index], 
          isSearching: false, 
          status: 'error',
          error: String(err),
        };
        return updated;
      });
    }
  };
  
  const applyResult = (index: number, result: MetadataSearchResult) => {
    setGameEdits(prev => {
      const updated = [...prev];
      updated[index] = {
        ...updated[index],
        selectedResult: result,
        editedTitle: result.name,
        editedDeveloper: result.developer || '',
        editedPublisher: result.publisher || '',
        editedCover: result.cover_url || '',
        editedDescription: result.summary || '',
      };
      return updated;
    });
  };
  
  const updateEdit = (index: number, updates: Partial<GameMetadataEdit>) => {
    setGameEdits(prev => {
      const updated = [...prev];
      updated[index] = { ...updated[index], ...updates };
      return updated;
    });
  };
  
  const handleSaveAll = async () => {
    setIsSaving(true);
    
    for (let i = 0; i < gameEdits.length; i++) {
      const edit = gameEdits[i];
      
      try {
        updateEdit(i, { status: 'saving' });
        
        await invoke('update_game', {
          request: {
            id: edit.game.id,
            title: edit.editedTitle,
            description: edit.editedDescription || null,
            developer: edit.editedDeveloper || null,
            publisher: edit.editedPublisher || null,
            cover_image: edit.editedCover || null,
          }
        });
        
        updateEdit(i, { status: 'saved' });
      } catch (err) {
        updateEdit(i, { status: 'error', error: String(err) });
      }
    }
    
    setIsSaving(false);
    onSave();
  };
  
  const currentEdit = gameEdits[currentIndex];
  
  const goNext = () => {
    if (currentIndex < gameEdits.length - 1) {
      const nextIndex = currentIndex + 1;
      setCurrentIndex(nextIndex);
      
      // Auto-search next if not already done
      if (autoSearch && gameEdits[nextIndex].status === 'pending') {
        searchMetadata(nextIndex, gameEdits[nextIndex].searchQuery);
      }
    }
  };
  
  const goPrev = () => {
    if (currentIndex > 0) {
      setCurrentIndex(currentIndex - 1);
    }
  };
  
  const getCoverPreview = (url: string | null) => {
    if (!url) return null;
    if (url.startsWith('http')) return url;
    try {
      return convertFileSrc(url);
    } catch {
      return null;
    }
  };
  
  if (gameEdits.length === 0) {
    return null;
  }
  
  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-surface-300 rounded-xl w-full max-w-4xl max-h-[90vh] flex flex-col shadow-2xl">
        {/* Header */}
        <div className="p-4 border-b border-surface-100 flex items-center justify-between flex-shrink-0">
          <div>
            <h2 className="text-lg font-semibold">{t('batch.title')}</h2>
            <p className="text-sm text-gray-400">
              {t('batch.progress', { current: currentIndex + 1, total: gameEdits.length })}
            </p>
          </div>
          
          <div className="flex items-center gap-4">
            {/* Source selector */}
            <div className="flex items-center gap-2">
              <span className="text-sm text-gray-400">{t('batch.source')}:</span>
              <select
                value={source}
                onChange={(e) => setSource(e.target.value as MetadataSource)}
                className="px-2 py-1 bg-surface-200 rounded text-sm focus:outline-none focus:ring-1 focus:ring-accent"
              >
                <option value="steam">Steam</option>
                <option value="itch">Itch.io</option>
                <option value="manual">{t('batch.manual')}</option>
              </select>
            </div>
            
            <button onClick={onClose} className="text-gray-500 hover:text-white">
              ✕
            </button>
          </div>
        </div>
        
        {/* Progress bar */}
        <div className="h-1 bg-surface-200">
          <div 
            className="h-full bg-accent transition-all"
            style={{ width: `${((currentIndex + 1) / gameEdits.length) * 100}%` }}
          />
        </div>
        
        {/* Content */}
        {currentEdit && (
          <div className="flex-1 overflow-auto p-4">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              {/* Left: Search & Results */}
              <div>
                <div className="mb-4">
                  <label className="block text-sm font-medium mb-2">
                    {t('batch.searchGame')}
                  </label>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={currentEdit.searchQuery}
                      onChange={(e) => updateEdit(currentIndex, { searchQuery: e.target.value })}
                      className="flex-1 px-3 py-2 bg-surface-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-accent"
                      placeholder={t('batch.searchPlaceholder')}
                      onKeyDown={(e) => e.key === 'Enter' && searchMetadata(currentIndex, currentEdit.searchQuery)}
                    />
                    <button
                      onClick={() => searchMetadata(currentIndex, currentEdit.searchQuery)}
                      disabled={currentEdit.isSearching}
                      className="btn btn-primary"
                    >
                      {currentEdit.isSearching ? '...' : '🔍'}
                    </button>
                  </div>
                </div>
                
                {/* Search results */}
                <div className="border border-surface-100 rounded-lg max-h-60 overflow-y-auto">
                  {currentEdit.searchResults.length > 0 ? (
                    currentEdit.searchResults.map((result) => (
                      <div
                        key={result.id}
                        onClick={() => applyResult(currentIndex, result)}
                        className={`flex items-center gap-3 p-3 cursor-pointer border-b border-surface-100 last:border-0 transition-colors
                          ${currentEdit.selectedResult?.id === result.id ? 'bg-accent/20' : 'hover:bg-surface-200'}
                        `}
                      >
                        {result.cover_url && (
                          <img 
                            src={result.cover_url} 
                            alt="" 
                            className="w-12 h-16 object-cover rounded"
                          />
                        )}
                        <div className="flex-1 min-w-0">
                          <p className="font-medium truncate">{result.name}</p>
                          <p className="text-xs text-gray-500">{result.developer}</p>
                          {result.release_date && (
                            <p className="text-xs text-gray-600">{result.release_date}</p>
                          )}
                        </div>
                        {currentEdit.selectedResult?.id === result.id && (
                          <span className="text-accent">✓</span>
                        )}
                      </div>
                    ))
                  ) : (
                    <div className="p-4 text-center text-gray-500">
                      {currentEdit.isSearching ? t('common.loading') : t('batch.noResults')}
                    </div>
                  )}
                </div>
              </div>
              
              {/* Right: Edit form */}
              <div>
                <div className="flex gap-4 mb-4">
                  {/* Cover preview */}
                  <div className="w-24 h-32 bg-surface-200 rounded-lg overflow-hidden flex-shrink-0">
                    {getCoverPreview(currentEdit.editedCover) ? (
                      <img 
                        src={getCoverPreview(currentEdit.editedCover)!} 
                        alt="" 
                        className="w-full h-full object-cover"
                      />
                    ) : (
                      <div className="w-full h-full flex items-center justify-center text-3xl opacity-30">
                        🎮
                      </div>
                    )}
                  </div>
                  
                  <div className="flex-1">
                    <p className="text-xs text-gray-400 mb-1">{t('batch.originalTitle')}:</p>
                    <p className="text-sm text-gray-300 mb-2">{currentEdit.game.title}</p>
                    
                    <input
                      type="text"
                      value={currentEdit.editedTitle}
                      onChange={(e) => updateEdit(currentIndex, { editedTitle: e.target.value })}
                      placeholder={t('edit.gameTitle')}
                      className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-accent"
                    />
                  </div>
                </div>
                
                <div className="grid grid-cols-2 gap-4 mb-4">
                  <div>
                    <label className="block text-xs text-gray-400 mb-1">{t('edit.developer')}</label>
                    <input
                      type="text"
                      value={currentEdit.editedDeveloper}
                      onChange={(e) => updateEdit(currentIndex, { editedDeveloper: e.target.value })}
                      className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-accent"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-gray-400 mb-1">{t('edit.publisher')}</label>
                    <input
                      type="text"
                      value={currentEdit.editedPublisher}
                      onChange={(e) => updateEdit(currentIndex, { editedPublisher: e.target.value })}
                      className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-accent"
                    />
                  </div>
                </div>
                
                <div className="mb-4">
                  <label className="block text-xs text-gray-400 mb-1">{t('edit.coverImage')}</label>
                  <input
                    type="text"
                    value={currentEdit.editedCover}
                    onChange={(e) => updateEdit(currentIndex, { editedCover: e.target.value })}
                    placeholder="URL"
                    className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-accent text-sm"
                  />
                </div>
                
                <div>
                  <label className="block text-xs text-gray-400 mb-1">{t('edit.description')}</label>
                  <textarea
                    value={currentEdit.editedDescription}
                    onChange={(e) => updateEdit(currentIndex, { editedDescription: e.target.value })}
                    rows={3}
                    className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-accent resize-none text-sm"
                  />
                </div>
                
                {/* Status */}
                {currentEdit.status === 'saved' && (
                  <div className="mt-3 p-2 bg-green-500/20 text-green-400 rounded text-sm text-center">
                    ✓ {t('batch.saved')}
                  </div>
                )}
                {currentEdit.status === 'error' && (
                  <div className="mt-3 p-2 bg-danger/20 text-danger rounded text-sm">
                    ❌ {currentEdit.error}
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
        
        {/* Navigation & Actions */}
        <div className="p-4 border-t border-surface-100 flex items-center justify-between flex-shrink-0">
          <div className="flex gap-2">
            <button
              onClick={goPrev}
              disabled={currentIndex === 0}
              className="btn btn-secondary disabled:opacity-50"
            >
              ← {t('common.previous')}
            </button>
            <button
              onClick={goNext}
              disabled={currentIndex === gameEdits.length - 1}
              className="btn btn-secondary disabled:opacity-50"
            >
              {t('common.next')} →
            </button>
          </div>
          
          <div className="text-sm text-gray-400">
            {gameEdits.filter(e => e.status === 'saved').length} / {gameEdits.length} {t('batch.saved')}
          </div>
          
          <div className="flex gap-2">
            <button onClick={onClose} className="btn btn-secondary">
              {t('common.cancel')}
            </button>
            <button
              onClick={handleSaveAll}
              disabled={isSaving}
              className="btn btn-primary disabled:opacity-50"
            >
              {isSaving ? t('common.loading') : t('batch.saveAll')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
