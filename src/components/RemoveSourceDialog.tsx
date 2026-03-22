import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { SelectedSource } from '../types';

interface RemoveSourceDialogProps {
  selectedSource: SelectedSource;
  onClose: () => void;
  onRemove: (deleteGames: boolean) => Promise<void>;
  isPending: boolean;
}

export default function RemoveSourceDialog({ selectedSource, onClose, onRemove, isPending }: RemoveSourceDialogProps) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);

  const handleKeep = async () => {
    setError(null);
    try {
      await onRemove(false);
      // onClose will be called by parent after success
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleDelete = async () => {
    setError(null);
    try {
      await onRemove(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-surface-300 rounded-xl w-full max-w-md shadow-2xl">
        {/* Header */}
        <div className="p-4 border-b border-surface-100 flex items-center justify-between">
          <h2 className="text-lg font-semibold">{t('space.removeSourceDialogTitle')}</h2>
          <button 
            onClick={onClose}
            disabled={isPending}
            className="text-gray-500 hover:text-white disabled:opacity-50"
          >
            ✕
          </button>
        </div>
        
        {/* Content */}
        <div className="p-4 space-y-4">
          <p className="text-sm text-gray-300">
            {t('space.removeSourceDialogMessage')} <span className="font-mono text-gray-400">{selectedSource.sourcePath}</span>
          </p>
          
          <div className="flex gap-3">
            <button
              onClick={handleKeep}
              disabled={isPending}
              className="flex-1 btn btn-secondary flex flex-col items-center p-4 disabled:opacity-50"
            >
              <span className="text-lg mb-1">📁</span>
              <span className="font-medium">{t('space.removeSourceKeepGames')}</span>
            </button>
            <button
              onClick={handleDelete}
              disabled={isPending}
              className="flex-1 btn flex flex-col items-center p-4 bg-danger/20 text-danger border border-danger/30 hover:bg-danger/30 disabled:opacity-50"
            >
              <span className="text-lg mb-1">🗑️</span>
              <span className="font-medium">{t('space.removeSourceDeleteGames')}</span>
            </button>
          </div>
          
          {/* Error message */}
          {error && (
            <div className="p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm">
              ❌ {error}
            </div>
          )}
          
          <p className="text-xs text-gray-500 text-center">
            {t('common.cancel')} to close
          </p>
        </div>
        
        {/* Footer with Cancel button */}
        <div className="p-4 border-t border-surface-100 flex justify-end">
          <button
            onClick={onClose}
            disabled={isPending}
            className="btn btn-secondary"
          >
            {t('common.cancel')}
          </button>
        </div>
      </div>
    </div>
  );
}
