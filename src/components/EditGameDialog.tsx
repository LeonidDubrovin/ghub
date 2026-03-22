import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import type { Game } from '../types';
import { createLoggerForComponent } from '../lib/logger';

interface EditGameDialogProps {
  game: Game;
  onClose: () => void;
  onSave: () => void;
  onDelete?: () => void;
}

interface MetadataSearchResult {
  id: string;
  name: string;
  cover_url: string | null;
  release_date: string | null;
  developer: string | null;
  publisher: string | null;
  description: string | null;
  rating: number | null;
  source: string;
  url: string | null;
}

export default function EditGameDialog({ game, onClose, onSave, onDelete }: EditGameDialogProps) {
  const logger = createLoggerForComponent('EditGameDialog');
  const { t } = useTranslation();
   
  const [title, setTitle] = useState(game.title);
  const [description, setDescription] = useState(game.description || '');
  const [developer, setDeveloper] = useState(game.developer || '');
  const [publisher, setPublisher] = useState(game.publisher || '');
  const [coverImage, setCoverImage] = useState(game.cover_image || '');
  const [isFavorite, setIsFavorite] = useState(game.is_favorite);
  const [completionStatus, setCompletionStatus] = useState(game.completion_status);
  const [userRating, setUserRating] = useState(game.user_rating || 0);
   
  const [isSaving, setIsSaving] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  // Metadata search
  const [searchQuery, setSearchQuery] = useState(game.title);
  const [isSearching, setIsSearching] = useState(false);
  const [searchResults, setSearchResults] = useState<MetadataSearchResult[]>([]);
  const [showSearchResults, setShowSearchResults] = useState(false);
  const [activeSources, setActiveSources] = useState({ steam: true, itch: true });
  
  // Metadata preview & selection
  const [previewResult, setPreviewResult] = useState<MetadataSearchResult | null>(null);
  const [fieldsToUpdate, setFieldsToUpdate] = useState({
    title: true,
    description: true,
    developer: true,
    publisher: true,
    cover: true,
  });
  
  const handleSave = async () => {
    setIsSaving(true);
    setError(null);
    
    try {
      await invoke('update_game', {
        request: {
          id: game.id,
          title: title || null,
          description: description || null,
          developer: developer || null,
          publisher: publisher || null,
          cover_image: coverImage || null,
          is_favorite: isFavorite,
          completion_status: completionStatus,
          user_rating: userRating > 0 ? userRating : null,
        }
      });
      onSave();
      onClose();
    } catch (err) {
      logger.error('Save failed:', err);
      setError(String(err));
    } finally {
      setIsSaving(false);
    }
  };
   
  const handleDelete = async () => {
    setIsDeleting(true);
    setError(null);
    
    try {
      await invoke('delete_game', { id: game.id });
      onDelete?.();
      onSave(); // Refresh list
      onClose();
    } catch (err) {
      logger.error('Delete failed:', err);
      setError(String(err));
    } finally {
      setIsDeleting(false);
      setShowDeleteConfirm(false);
    }
  };
   
  const handleSearch = async () => {
    if (!searchQuery.trim()) return;
    
    setIsSearching(true);
    setError(null);
    setShowSearchResults(true);
    setSearchResults([]);
    
    const sources = [];
    if (activeSources.steam) sources.push('steam');
    if (activeSources.itch) sources.push('itch');
    
    try {
      const results = await invoke<MetadataSearchResult[]>('search_game_metadata', {
        query: searchQuery.trim(),
        sources
      });
      setSearchResults(results);
    } catch (err) {
      logger.error('Search failed:', err);
      setError(String(err));
    } finally {
      setIsSearching(false);
    }
  };
  
  const handleResultClick = (result: MetadataSearchResult) => {
    setPreviewResult(result);
    // Reset fields selection default
    setFieldsToUpdate({
      title: true,
      description: !!result.description,
      developer: !!result.developer,
      publisher: !!result.publisher,
      cover: !!result.cover_url,
    });
  };

  const applySelectedMetadata = () => {
    if (!previewResult) return;
    
    if (fieldsToUpdate.title) setTitle(previewResult.name);
    if (fieldsToUpdate.description && previewResult.description) setDescription(previewResult.description);
    if (fieldsToUpdate.developer && previewResult.developer) setDeveloper(previewResult.developer);
    if (fieldsToUpdate.publisher && previewResult.publisher) setPublisher(previewResult.publisher);
    if (fieldsToUpdate.cover && previewResult.cover_url) setCoverImage(previewResult.cover_url);
    
    setPreviewResult(null);
    setShowSearchResults(false);
  };
  
  const statusOptions = [
    { value: 'not_played', label: t('status.notPlayed') },
    { value: 'playing', label: t('status.playing') },
    { value: 'completed', label: t('status.completed') },
    { value: 'abandoned', label: t('status.abandoned') },
    { value: 'on_hold', label: t('status.onHold') },
  ];
  
  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4">
      <div className="bg-surface-300 rounded-xl w-full max-w-4xl max-h-[90vh] flex flex-col shadow-2xl ring-1 ring-white/10">
        {/* Header */}
        <div className="p-4 border-b border-surface-100 flex items-center justify-between flex-shrink-0 bg-surface-400 rounded-t-xl">
          <h2 className="text-lg font-semibold flex items-center gap-2">
            ✏️ {t('edit.title')}
          </h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white transition-colors w-8 h-8 flex items-center justify-center rounded-full hover:bg-white/10">✕</button>
        </div>
        
        <div className="flex flex-1 overflow-hidden">
          {/* Main Form */}
          <div className="flex-1 overflow-y-auto p-6 border-r border-surface-100">
             {error && (
              <div className="mb-4 p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm flex items-center gap-2">
                ⚠️ {error}
              </div>
            )}
            
            <div className="grid grid-cols-[200px_1fr] gap-6">
              {/* Cover Column */}
              <div className="space-y-3">
                <label className="block text-sm font-medium text-gray-400">{t('edit.cover')}</label>
                <div className="aspect-[2/3] bg-surface-100 rounded-lg overflow-hidden relative group border border-surface-100">
                  {coverImage ? (
                    <img src={coverImage} alt="" className="w-full h-full object-cover" onError={(e) => { e.currentTarget.style.display = 'none'; }} />
                  ) : (
                    <div className="w-full h-full flex items-center justify-center text-4xl opacity-20">🖼️</div>
                  )}
                  <div className="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                    <button onClick={() => setCoverImage('')} className="text-white text-xs bg-red-500 px-2 py-1 rounded">Remove</button>
                  </div>
                </div>
                <input
                  type="text"
                  value={coverImage}
                  onChange={e => setCoverImage(e.target.value)}
                  placeholder="https://..."
                  className="w-full px-3 py-2 bg-surface-200 rounded-lg text-xs focus:ring-1 focus:ring-accent outline-none"
                />
                
                {/* Favorite Toggle */}
                <div className="pt-2">
                  <button
                    onClick={() => setIsFavorite(!isFavorite)}
                    className={`w-full py-2 rounded-lg transition-colors flex items-center justify-center gap-2 text-sm ${
                      isFavorite ? 'bg-yellow-500/20 text-yellow-400 border border-yellow-500/50' : 'bg-surface-200 text-gray-400 hover:bg-surface-100'
                    }`}
                  >
                    {isFavorite ? '⭐ ' + t('edit.favoriteOn') : '☆ ' + t('edit.favoriteOff')}
                  </button>
                </div>
              </div>
              
              {/* Fields Column */}
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium mb-1 text-gray-400">{t('edit.gameTitle')}</label>
                  <input
                    type="text"
                    value={title}
                    onChange={e => setTitle(e.target.value)}
                    className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:ring-1 focus:ring-accent outline-none font-medium text-lg"
                  />
                </div>
                
                <div className="grid grid-cols-2 gap-4">
                   <div>
                    <label className="block text-sm font-medium mb-1 text-gray-400">{t('edit.developer')}</label>
                    <input type="text" value={developer} onChange={e => setDeveloper(e.target.value)} className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:ring-1 focus:ring-accent outline-none" />
                  </div>
                  <div>
                    <label className="block text-sm font-medium mb-1 text-gray-400">{t('edit.publisher')}</label>
                    <input type="text" value={publisher} onChange={e => setPublisher(e.target.value)} className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:ring-1 focus:ring-accent outline-none" />
                  </div>
                </div>
                
                <div className="grid grid-cols-2 gap-4">
                   <div>
                    <label className="block text-sm font-medium mb-1 text-gray-400">{t('edit.status')}</label>
                    <select value={completionStatus} onChange={e => setCompletionStatus(e.target.value as any)} className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:ring-1 focus:ring-accent outline-none appearance-none">
                      {statusOptions.map(opt => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium mb-1 text-gray-400">{t('edit.rating')}</label>
                    <div className="flex gap-1 h-[38px] items-center">
                      {[1, 2, 3, 4, 5].map(star => (
                        <button key={star} onClick={() => setUserRating(userRating === star ? 0 : star)} className={`text-2xl transition-colors ${star <= userRating ? 'text-yellow-400' : 'text-gray-600 hover:text-gray-400'}`}>★</button>
                      ))}
                    </div>
                  </div>
                </div>

                <div>
                   <label className="block text-sm font-medium mb-1 text-gray-400">{t('edit.description')}</label>
                   <textarea value={description} onChange={e => setDescription(e.target.value)} rows={6} className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:ring-1 focus:ring-accent outline-none resize-none text-sm leading-relaxed" />
                </div>
              </div>
            </div>
          </div>
          
          {/* Metadata Sidebar */}
          <div className="w-[350px] bg-surface-200 flex flex-col border-l border-surface-100">
             <div className="p-4 border-b border-surface-100">
               <label className="block text-sm font-medium mb-2 text-accent">🌐 {t('edit.searchMetadata')}</label>
               <div className="flex gap-2 mb-2">
                 <input type="text" value={searchQuery} onChange={e => setSearchQuery(e.target.value)} onKeyDown={e => e.key === 'Enter' && handleSearch()} className="flex-1 px-3 py-2 bg-surface-300 rounded-lg text-sm focus:ring-1 focus:ring-accent outline-none" placeholder={t('edit.searchPlaceholder')} />
                 <button onClick={handleSearch} disabled={isSearching || !searchQuery.trim()} className="btn btn-sm btn-primary px-3">{isSearching ? '...' : 'Go'}</button>
               </div>
               
               {/* Source toggles */}
               <div className="flex gap-3 text-xs text-gray-400">
                 <label className="flex items-center gap-1 cursor-pointer hover:text-white">
                   <input type="checkbox" checked={activeSources.steam} onChange={e => setActiveSources({...activeSources, steam: e.target.checked})} className="rounded bg-surface-300 border-none text-accent focus:ring-0" /> Steam
                 </label>
                 <label className="flex items-center gap-1 cursor-pointer hover:text-white">
                   <input type="checkbox" checked={activeSources.itch} onChange={e => setActiveSources({...activeSources, itch: e.target.checked})} className="rounded bg-surface-300 border-none text-accent focus:ring-0" /> Itch.io
                 </label>
               </div>
             </div>
             
             <div className="flex-1 overflow-y-auto p-2 space-y-2">
               {isSearching && <div className="p-4 text-center text-gray-500 italic">{t('edit.searching')}</div>}
               
               {!isSearching && searchResults.length === 0 && showSearchResults && (
                 <div className="p-4 text-center text-gray-500">{t('edit.noResults')}</div>
               )}
               
               {searchResults.map(result => (
                 <div key={result.id} onClick={() => handleResultClick(result)} 
                   className="flex gap-4 p-3 rounded-xl hover:bg-surface-300 cursor-pointer transition-all group border border-transparent hover:border-surface-100 hover:shadow-lg bg-surface-200/50 mb-2">
                   {/* Cover */}
                   <div className="w-16 h-24 bg-black/20 rounded-lg overflow-hidden flex-shrink-0 shadow-md">
                     {result.cover_url ? (
                       <img src={result.cover_url} className="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500" alt="" />
                     ) : (
                       <div className="w-full h-full flex items-center justify-center text-xs opacity-30">?</div>
                     )}
                   </div>
                   
                   {/* Info */}
                   <div className="flex-1 min-w-0 flex flex-col justify-center">
                     <div className="font-bold text-base text-gray-100 group-hover:text-white truncate mb-1">{result.name}</div>
                     
                     <div className="text-xs text-gray-400 mb-2 line-clamp-2 leading-relaxed">
                        {result.description || t('edit.noDescription')}
                     </div>

                     <div className="flex items-center gap-2 mt-auto">
                        <span className={`px-1.5 py-0.5 rounded text-[10px] uppercase font-bold tracking-wider ${result.source === 'steam' ? 'bg-[#1b2838] text-[#66c0f4] border border-[#66c0f4]/30' : 'bg-[#fa5c5c]/10 text-[#fa5c5c] border border-[#fa5c5c]/30'}`}>
                          {result.source}
                        </span>
                        {result.developer && (
                          <span className="text-xs text-gray-500 truncate max-w-[120px]" title={result.developer}>
                            👤 {result.developer}
                          </span>
                        )}
                        {result.release_date && (
                          <span className="text-xs text-gray-500 truncate ml-auto">
                            📅 {result.release_date}
                          </span>
                        )}
                     </div>
                   </div>
                 </div>
               ))}
             </div>
          </div>
        </div>
        
        {/* Footer */}
        <div className="p-4 border-t border-surface-100 flex justify-between bg-surface-400 rounded-b-xl">
           <button onClick={() => setShowDeleteConfirm(!showDeleteConfirm)} className="text-danger hover:underline text-sm px-2">
             {showDeleteConfirm ? t('edit.confirmDelete') : t('actions.delete')}
           </button>
           {showDeleteConfirm && (
             <button onClick={handleDelete} disabled={isDeleting} className="btn btn-sm bg-danger text-white ml-2">{isDeleting ? '...' : t('common.delete')}</button>
           )}
           
           <div className="flex gap-3 ml-auto">
             <button onClick={onClose} className="btn btn-secondary">{t('common.cancel')}</button>
             <button onClick={handleSave} disabled={isSaving} className="btn btn-primary px-6">{isSaving ? t('common.loading') : t('common.save')}</button>
           </div>
        </div>
      </div>
      
      {/* Preview Dialog */}
      {previewResult && (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
          <div className="bg-surface-300 rounded-xl max-w-lg w-full shadow-2xl ring-1 ring-white/10 flex flex-col max-h-[90vh]">
            <div className="p-4 border-b border-surface-100 bg-surface-400 rounded-t-xl flex justify-between">
              <h3 className="font-semibold text-white">{t('actions.updateMetadata')}</h3>
              <button onClick={() => setPreviewResult(null)} className="text-gray-400 hover:text-white">✕</button>
            </div>
            
            <div className="p-6 overflow-y-auto">
              <div className="flex gap-4 mb-6">
                <div className="w-24 h-32 bg-black/20 rounded overflow-hidden flex-shrink-0 shadow-lg">
                   {previewResult.cover_url ? <img src={previewResult.cover_url} className="w-full h-full object-cover" alt="" /> : <div className="flex items-center justify-center h-full text-2xl opacity-30">?</div>}
                </div>
                <div>
                  <h4 className="font-bold text-lg text-white mb-1">{previewResult.name}</h4>
                  <p className="text-sm text-gray-400 mb-2">{previewResult.developer}</p>
                  <a href={previewResult.url || '#'} target="_blank" rel="noreferrer" className="text-xs text-accent hover:underline flex items-center gap-1">
                    Open in {previewResult.source} ↗
                  </a>
                </div>
              </div>
              
              <div className="space-y-3">
                <div className="text-xs font-bold text-gray-500 uppercase tracking-wider mb-2">Select fields to update:</div>
                
                <label className="flex items-center gap-3 p-3 bg-surface-200 rounded-lg cursor-pointer hover:bg-surface-100 transition-colors">
                  <input type="checkbox" checked={fieldsToUpdate.title} onChange={e => setFieldsToUpdate({...fieldsToUpdate, title: e.target.checked})} className="rounded bg-surface-400 border-none text-accent w-5 h-5" />
                  <div className="flex-1">
                    <div className="text-sm font-medium text-gray-200">Title</div>
                    <div className="text-xs text-gray-500 truncate">{previewResult.name}</div>
                  </div>
                </label>
                
                {previewResult.developer && (
                  <label className="flex items-center gap-3 p-3 bg-surface-200 rounded-lg cursor-pointer hover:bg-surface-100 transition-colors">
                    <input type="checkbox" checked={fieldsToUpdate.developer} onChange={e => setFieldsToUpdate({...fieldsToUpdate, developer: e.target.checked})} className="rounded bg-surface-400 border-none text-accent w-5 h-5" />
                    <div className="flex-1">
                      <div className="text-sm font-medium text-gray-200">Developer</div>
                      <div className="text-xs text-gray-500 truncate">{previewResult.developer}</div>
                    </div>
                  </label>
                )}
                
                {previewResult.publisher && (
                  <label className="flex items-center gap-3 p-3 bg-surface-200 rounded-lg cursor-pointer hover:bg-surface-100 transition-colors">
                    <input type="checkbox" checked={fieldsToUpdate.publisher} onChange={e => setFieldsToUpdate({...fieldsToUpdate, publisher: e.target.checked})} className="rounded bg-surface-400 border-none text-accent w-5 h-5" />
                    <div className="flex-1">
                      <div className="text-sm font-medium text-gray-200">Publisher</div>
                      <div className="text-xs text-gray-500 truncate">{previewResult.publisher}</div>
                    </div>
                  </label>
                )}
                
                {previewResult.description && (
                  <label className="flex items-center gap-3 p-3 bg-surface-200 rounded-lg cursor-pointer hover:bg-surface-100 transition-colors">
                    <input type="checkbox" checked={fieldsToUpdate.description} onChange={e => setFieldsToUpdate({...fieldsToUpdate, description: e.target.checked})} className="rounded bg-surface-400 border-none text-accent w-5 h-5" />
                    <div className="flex-1">
                      <div className="text-sm font-medium text-gray-200">Description</div>
                      <div className="text-xs text-gray-500 line-clamp-1">{previewResult.description}</div>
                    </div>
                  </label>
                )}
                
                {previewResult.cover_url && (
                  <label className="flex items-center gap-3 p-3 bg-surface-200 rounded-lg cursor-pointer hover:bg-surface-100 transition-colors">
                    <input type="checkbox" checked={fieldsToUpdate.cover} onChange={e => setFieldsToUpdate({...fieldsToUpdate, cover: e.target.checked})} className="rounded bg-surface-400 border-none text-accent w-5 h-5" />
                    <div className="flex-1">
                      <div className="text-sm font-medium text-gray-200">Cover Image</div>
                      <div className="text-xs text-gray-500 truncate">{previewResult.cover_url}</div>
                    </div>
                  </label>
                )}
              </div>
            </div>
            
            <div className="p-4 border-t border-surface-100 bg-surface-400 rounded-b-xl flex justify-end gap-3">
              <button onClick={() => setPreviewResult(null)} className="btn btn-secondary">Cancel</button>
              <button onClick={applySelectedMetadata} className="btn btn-primary">Apply Selected</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
