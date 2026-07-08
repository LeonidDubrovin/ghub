import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import type { Game, GameLink, AddGameLinkResponse } from '../types';
import { createLoggerForComponent } from '../lib/logger';

interface EditGameDialogProps {
  game: Game;
  onClose: () => void;
  onSave: () => void;
  onDelete?: () => void;
  onOpenGame?: (game: Game, link?: GameLink) => void;
}

import MetadataSearchDialog from './MetadataSearchDialog';

export default function EditGameDialog({ game, onClose, onSave, onDelete, onOpenGame }: EditGameDialogProps) {
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
   
   // Game links management
   const [gameLinks, setGameLinks] = useState<GameLink[]>([]);
   const [newLinkUrl, setNewLinkUrl] = useState('');
   const [newLinkTitle, setNewLinkTitle] = useState('');
   const [newLinkSource, setNewLinkSource] = useState<string>('other');
    const [isAddingLink, setIsAddingLink] = useState(false);
    const [isDeletingLink, setIsDeletingLink] = useState<string | null>(null);
    const [duplicateLink, setDuplicateLink] = useState<{ game: Game; link: GameLink } | null>(null);
    const [isSearchOpen, setIsSearchOpen] = useState(false);

  
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
   
   const handleSearchSave = () => {
     onSave();
     setIsSearchOpen(false);
   };

    // Fetch game links on mount
   useEffect(() => {
     const fetchLinks = async () => {
       try {
         const links = await invoke<GameLink[]>('get_game_links', { gameId: game.id });
         setGameLinks(links);
       } catch (error) {
         logger.error('Failed to fetch game links:', error);
       }
     };
     fetchLinks();
   }, [game.id]);

    const handleAddLink = async () => {
      if (!newLinkUrl.trim()) return;
      
      setIsAddingLink(true);
      setError(null);
      setDuplicateLink(null);
      
      try {
        const response = await invoke<AddGameLinkResponse>('add_game_link', {
          gameId: game.id,
          url: newLinkUrl.trim(),
          title: newLinkTitle.trim() || null,
          sourceType: newLinkSource === 'other' ? null : newLinkSource,
          downloadStatus: null,
          queueSpace: null,
        });
        if (response.is_duplicate && response.existing_game) {
          setDuplicateLink({ game: response.existing_game, link: response.link });
        } else {
          setNewLinkUrl('');
          setNewLinkTitle('');
          setNewLinkSource('other');
          // Refresh links
          const links = await invoke<GameLink[]>('get_game_links', { gameId: game.id });
          setGameLinks(links);
        }
      } catch (err) {
        logger.error('Add link failed:', err);
        setError(String(err));
      } finally {
        setIsAddingLink(false);
      }
    };


   const handleDeleteLink = async (linkId: string) => {
     setIsDeletingLink(linkId);
     setError(null);
     
     try {
       await invoke('remove_game_link', { linkId });
       setGameLinks(prev => prev.filter(l => l.id !== linkId));
     } catch (err) {
       logger.error('Delete link failed:', err);
       setError(String(err));
     } finally {
       setIsDeletingLink(null);
     }
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
        
        <div className="flex-1 overflow-hidden">
          {/* Main Form */}
          <div className="flex-1 overflow-y-auto p-6">
             {error && (
              <div className="mb-4 p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm flex items-center gap-2">
                ⚠️ {error}
              </div>
            )}
            
            {duplicateLink && (
              <div className="mb-4 p-3 bg-warning/20 border border-warning/50 rounded-lg text-warning text-sm">
                <div className="flex items-center justify-between gap-2">
                  <span>
                    {t('edit.duplicateLink', { title: duplicateLink.game.title })}
                  </span>
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

                 {/* External Links Section */}
                 <div className="pt-4 border-t border-surface-100">
                   <label className="block text-sm font-medium mb-2 text-gray-400">{t('details.sourceLinks')}</label>
                   
                   {/* Existing links list */}
                   {gameLinks.length > 0 && (
                     <div className="space-y-2 mb-3">
                       {gameLinks.map(link => (
                         <div key={link.id} className="flex items-center gap-2 p-2 bg-surface-200 rounded-lg">
                           <span className="text-sm">
                             {link.source_type === 'steam' ? '🎮' : 
                              link.source_type === 'itch' ? '🎨' : 
                              link.source_type === 'gog' ? '🛡️' : 
                              link.source_type === 'epic' ? '⚔️' : '🔗'}
                           </span>
                           <div className="flex-1 min-w-0">
                             <div className="text-sm text-gray-200 truncate">{link.title || link.url}</div>
                             <div className="text-xs text-gray-500 truncate">{link.url}</div>
                           </div>
                           <button
                             onClick={() => handleDeleteLink(link.id)}
                             disabled={isDeletingLink === link.id}
                             className="text-danger hover:text-red-400 text-sm px-2 py-1"
                             title={t('common.delete')}
                           >
                             {isDeletingLink === link.id ? '...' : '🗑️'}
                           </button>
                         </div>
                       ))}
                     </div>
                   )}

                   {/* Add new link form */}
                   <div className="space-y-2">
                     <input
                       type="text"
                       value={newLinkUrl}
                       onChange={e => setNewLinkUrl(e.target.value)}
                       placeholder="https://..."
                       className="w-full px-3 py-2 bg-surface-200 rounded-lg text-sm focus:ring-1 focus:ring-accent outline-none"
                     />
                     <div className="flex gap-2">
                       <input
                         type="text"
                         value={newLinkTitle}
                         onChange={e => setNewLinkTitle(e.target.value)}
                         placeholder={t('edit.gameTitle') + ' (optional)'}
                         className="flex-1 px-3 py-2 bg-surface-200 rounded-lg text-sm focus:ring-1 focus:ring-accent outline-none"
                       />
                       <select
                         value={newLinkSource}
                         onChange={e => setNewLinkSource(e.target.value)}
                         className="px-3 py-2 bg-surface-200 rounded-lg text-sm focus:ring-1 focus:ring-accent outline-none"
                       >
                         <option value="steam">Steam</option>
                         <option value="itch">itch.io</option>
                         <option value="gog">GOG</option>
                         <option value="epic">Epic</option>
                         <option value="other">{t('sources.other')}</option>
                       </select>
                       <button
                         onClick={handleAddLink}
                         disabled={isAddingLink || !newLinkUrl.trim()}
                         className="btn btn-sm btn-primary"
                       >
                         {isAddingLink ? '...' : t('common.add')}
                       </button>
                     </div>
                   </div>
                 </div>
               </div>
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
              <button
                onClick={() => setIsSearchOpen(true)}
                className="btn btn-secondary"
              >
                🌐 {t('edit.searchMetadata')}
              </button>
              <button onClick={onClose} className="btn btn-secondary">{t('common.cancel')}</button>
              <button onClick={handleSave} disabled={isSaving} className="btn btn-primary px-6">{isSaving ? t('common.loading') : t('common.save')}</button>
            </div>
         </div>
       </div>

       <MetadataSearchDialog
         isOpen={isSearchOpen}
         games={[game]}
         onClose={() => setIsSearchOpen(false)}
         onSave={handleSearchSave}
         onOpenGame={onOpenGame}
       />
     </div>
   );
 }
