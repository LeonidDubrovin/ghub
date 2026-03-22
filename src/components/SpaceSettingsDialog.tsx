import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useSpaceSources, useAddSpaceSource, useRemoveSpaceSource, useUpdateSpaceSource } from '../hooks/useSpaces';
import { open } from '@tauri-apps/plugin-dialog';
import type { Space, SpaceSource } from '../types';

interface SpaceSettingsDialogProps {
  space: Space;
  onClose: () => void;
}

export default function SpaceSettingsDialog({ space, onClose }: SpaceSettingsDialogProps) {
  const { t } = useTranslation();
  const { data: sources = [], refetch: refetchSources } = useSpaceSources(space.id);
  const addSpaceSource = useAddSpaceSource();
  const removeSpaceSource = useRemoveSpaceSource();
  const updateSpaceSource = useUpdateSpaceSource();
  
  const [isSelectingFolder, setIsSelectingFolder] = useState(false);
  
  const handleSelectFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('space.selectFolder'),
      });
      
      if (selected && typeof selected === 'string') {
        // Check if already added
        if (sources.some(s => s.source_path === selected)) {
          alert(t('space.folderAlreadyAdded'));
          return;
        }
        
        await addSpaceSource.mutateAsync({
          space_id: space.id,
          source_path: selected,
          scan_recursively: true,
        });
        refetchSources();
      }
    } catch (err) {
      console.error('Failed to select folder:', err);
    } finally {
      setIsSelectingFolder(false);
    }
  };
  
  const handleRemoveSource = async (sourcePath: string) => {
    if (!confirm(t('space.confirmRemoveSource'))) return;
    
    await removeSpaceSource.mutateAsync({
      space_id: space.id,
      source_path: sourcePath,
    });
    refetchSources();
  };
  
  const handleToggleSource = async (source: SpaceSource, isActive: boolean) => {
    await updateSpaceSource.mutateAsync({
      space_id: space.id,
      source_path: source.source_path,
      is_active: isActive,
    });
    refetchSources();
  };
  
  
  return (
    <>
      <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
        <div className="bg-surface-300 rounded-xl w-full max-w-2xl shadow-2xl">
          {/* Header */}
          <div className="p-4 border-b border-surface-100 flex items-center justify-between">
            <div>
              <h2 className="text-lg font-semibold">{t('space.settingsTitle')}</h2>
              <p className="text-sm text-gray-400">{space.name}</p>
            </div>
            <button 
              onClick={onClose}
              className="text-gray-500 hover:text-white"
            >
              ✕
            </button>
          </div>
          
          {/* Content */}
          <div className="p-4">
            {/* Sources section */}
            <div className="mb-6">
              <div className="flex items-center justify-between mb-3">
                <h3 className="text-md font-medium">{t('space.sourceDirectories')}</h3>
                <button
                  onClick={() => setIsSelectingFolder(true)}
                  disabled={isSelectingFolder || addSpaceSource.isPending}
                  className="btn btn-primary text-sm"
                >
                  ➕ {t('space.addFolder')}
                </button>
              </div>
              
              {isSelectingFolder && (
                <div className="mb-4 p-4 bg-surface-400 rounded-lg text-center">
                  <p className="text-sm text-gray-400 mb-2">{t('space.selectFolderHint')}</p>
                  <button
                    onClick={handleSelectFolder}
                    disabled={addSpaceSource.isPending}
                    className="btn btn-primary"
                  >
                    {addSpaceSource.isPending ? t('common.loading') : t('space.browse')}
                  </button>
                  <button
                    onClick={() => setIsSelectingFolder(false)}
                    className="btn btn-secondary ml-2"
                  >
                    {t('common.cancel')}
                  </button>
                </div>
              )}
              
              {sources.length === 0 ? (
                <div className="p-8 text-center bg-surface-400 rounded-lg">
                  <span className="text-4xl mb-2 block">📁</span>
                  <p className="text-gray-400">{t('space.noSources')}</p>
                  <p className="text-sm text-gray-500 mt-1">
                    {t('space.noSourcesDescription')}
                  </p>
                </div>
              ) : (
                <div className="space-y-2">
                  {sources.map(source => (
                    <div 
                      key={source.source_path}
                      className={`p-3 rounded-lg border ${source.is_active ? 'bg-surface-200 border-surface-100' : 'bg-surface-400 border-surface-100 opacity-60'}`}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 mb-1">
                            <input
                              type="checkbox"
                              checked={source.is_active}
                              onChange={e => handleToggleSource(source, e.target.checked)}
                              className="w-4 h-4 rounded accent-accent"
                            />
                            <span className="font-medium truncate">{source.source_path.split('\\').pop()}</span>
                          </div>
                          <p className="text-xs text-gray-500 truncate">{source.source_path}</p>
                          <div className="flex gap-3 mt-2 text-xs text-gray-400">
                            <span>{t('space.recursive', { enabled: source.scan_recursively })}</span>
                            {source.last_scanned_at && (
                              <span>{t('space.lastScanned')}: {new Date(source.last_scanned_at).toLocaleDateString()}</span>
                            )}
                          </div>
                        </div>
                        
                        <button
                          onClick={() => handleRemoveSource(source.source_path)}
                          disabled={removeSpaceSource.isPending}
                          className="text-danger hover:text-red-700 p-1"
                          title={t('space.removeSource')}
                        >
                          🗑️
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
            
            {/* Actions */}
            <div className="flex justify-end items-center pt-4 border-t border-surface-100">
              <div className="text-sm text-gray-400 mr-auto">
                {sources.filter(s => s.is_active).length} {t('space.activeSources')}
              </div>
              <button onClick={onClose} className="btn btn-secondary">
                {t('common.close')}
              </button>
            </div>
          </div>
        </div>
      </div>
      
    </>
  );
}