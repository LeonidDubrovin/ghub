import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useCreateSpace, useAddSpaceSource } from '../hooks/useSpaces';
import { open } from '@tauri-apps/plugin-dialog';

interface AddSpaceDialogProps {
  onClose: () => void;
}

const COLORS = [
  '#ef4444', '#f97316', '#eab308', '#22c55e', 
  '#14b8a6', '#3b82f6', '#8b5cf6', '#ec4899'
];

const ICONS = ['📁', '🎮', '🎯', '⭐', '🎲', '🕹️', '💾', '📚'];

export default function AddSpaceDialog({ onClose }: AddSpaceDialogProps) {
  const { t } = useTranslation();
  const createSpace = useCreateSpace();
  const addSpaceSource = useAddSpaceSource();
  
  const [name, setName] = useState('');
  const [color, setColor] = useState(COLORS[0]);
  const [icon, setIcon] = useState(ICONS[0]);
  const [selectedPaths, setSelectedPaths] = useState<string[]>([]);
  const [isAddingSources, setIsAddingSources] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const handleSelectFolders = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: true,
        title: t('space.selectFolders'),
      });
      
      if (selected && Array.isArray(selected)) {
        // Filter out already added paths
        const newPaths = selected.filter(path => !selectedPaths.includes(path));
        setSelectedPaths(prev => [...prev, ...newPaths]);
      }
    } catch (err) {
      console.error('Failed to select folders:', err);
    }
  };
  
  const removePath = (path: string) => {
    setSelectedPaths(prev => prev.filter(p => p !== path));
  };
  
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!name.trim()) return;
    
    setError(null);
    setIsAddingSources(true);
    
    try {
      // Create space first
      const space = await createSpace.mutateAsync({
        name: name.trim(),
        type: 'virtual',
        color,
        icon,
        initial_sources: selectedPaths.length > 0 ? selectedPaths : undefined,
      });
      
      // Add sources if any
      if (selectedPaths.length > 0) {
        // Track which sources failed
        const failedSources: string[] = [];
        await Promise.all(
          selectedPaths.map(path =>
            addSpaceSource.mutateAsync({
              space_id: space.id,
              source_path: path,
              scan_recursively: true,
            }).catch(err => {
              failedSources.push(path);
              console.error(`Failed to add source ${path}:`, err);
              return null;
            })
          )
        );
        
        // If any sources failed, show error but don't close dialog
        if (failedSources.length > 0) {
          setError(t('space.addSourcesPartialError', {
            count: failedSources.length,
            total: selectedPaths.length
          }) || `Failed to add ${failedSources.length} of ${selectedPaths.length} sources`);
          setIsAddingSources(false);
          return;
        }
      }
      
      onClose();
    } catch (error) {
      console.error('Failed to create space:', error);
      const message = error instanceof Error ? error.message : String(error);
      setError(t('space.createSpaceError', { message }) || `Failed to create space: ${message}`);
      setIsAddingSources(false);
    }
  };
  
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-surface-300 rounded-xl w-full max-w-lg shadow-2xl">
        {/* Header */}
        <div className="p-4 border-b border-surface-100 flex items-center justify-between">
          <h2 className="text-lg font-semibold">{t('space.addTitle')}</h2>
          <button 
            onClick={onClose}
            className="text-gray-500 hover:text-white"
          >
            ✕
          </button>
        </div>
        
        {/* Form */}
        <form onSubmit={handleSubmit} className="p-4 space-y-4">
          {/* Name */}
          <div>
            <label className="block text-sm font-medium mb-1">
              {t('space.name')}
            </label>
            <input
              type="text"
              value={name}
              onChange={e => setName(e.target.value)}
              placeholder={t('space.namePlaceholder')}
              className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-accent"
              autoFocus
            />
            <p className="text-xs text-gray-500 mt-1">
              {t('space.description')}
            </p>
          </div>
          
          {/* Icon */}
          <div>
            <label className="block text-sm font-medium mb-2">
              {t('space.icon')}
            </label>
            <div className="flex gap-2 flex-wrap">
              {ICONS.map(i => (
                <button
                  key={i}
                  type="button"
                  onClick={() => setIcon(i)}
                  className={`w-10 h-10 rounded-lg text-xl flex items-center justify-center transition-all ${
                    icon === i 
                      ? 'bg-accent/30 ring-2 ring-accent' 
                      : 'bg-surface-200 hover:bg-surface-100'
                  }`}
                >
                  {i}
                </button>
              ))}
            </div>
          </div>
          
          {/* Color */}
          <div>
            <label className="block text-sm font-medium mb-2">
              {t('space.color')}
            </label>
            <div className="flex gap-2">
              {COLORS.map(c => (
                <button
                  key={c}
                  type="button"
                  onClick={() => setColor(c)}
                  className={`w-8 h-8 rounded-full transition-transform ${
                    color === c 
                      ? 'ring-2 ring-white ring-offset-2 ring-offset-surface-300 scale-110' 
                      : ''
                  }`}
                  style={{ backgroundColor: c }}
                />
              ))}
            </div>
          </div>
          
          {/* Source directories */}
          <div>
            <label className="block text-sm font-medium mb-2">
              {t('space.sourceDirectories')}
            </label>
            <p className="text-xs text-gray-500 mb-2">
              {t('space.sourceDirectoriesDescription')}
            </p>
            
            {/* Selected paths list */}
            {selectedPaths.length > 0 && (
              <div className="mb-2 max-h-32 overflow-auto border border-surface-100 rounded-lg">
                {selectedPaths.map((path, index) => (
                  <div key={index} className="flex items-center justify-between px-3 py-2 border-b border-surface-100 last:border-0">
                    <span className="text-sm truncate">{path}</span>
                    <button
                      type="button"
                      onClick={() => removePath(path)}
                      className="text-danger hover:text-red-700 ml-2"
                    >
                      ✕
                    </button>
                  </div>
                ))}
              </div>
            )}
            
            {/* Buttons */}
            <div className="flex gap-2">
              <button
                type="button"
                onClick={handleSelectFolders}
                disabled={isAddingSources}
                className="btn btn-secondary flex-1"
              >
                📁 {t('space.addFolders')}
              </button>
            </div>
          </div>
          
          {/* Error message */}
          {error && (
            <div className="mb-4 p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm">
              ❌ {error}
            </div>
          )}
          
          {/* Preview */}
          <div className="p-3 bg-surface-400 rounded-lg">
            <p className="text-xs text-gray-500 mb-2">{t('space.preview')}</p>
            <div className="flex items-center gap-3">
              <span 
                className="w-8 h-8 rounded flex items-center justify-center text-lg"
                style={{ backgroundColor: color }}
              >
                {icon}
              </span>
              <span className="font-medium">{name || t('space.namePlaceholder')}</span>
            </div>
            {selectedPaths.length > 0 && (
              <p className="text-xs text-gray-400 mt-2">
                {t('space.sourceCount', { count: selectedPaths.length })}
              </p>
            )}
          </div>
          
          {/* Actions */}
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="btn btn-secondary"
            >
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              disabled={!name.trim() || createSpace.isPending || isAddingSources}
              className="btn btn-primary disabled:opacity-50"
            >
              {createSpace.isPending || isAddingSources ? t('common.loading') : t('common.create')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}