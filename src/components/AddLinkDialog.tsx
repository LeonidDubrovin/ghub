import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { createLoggerForComponent } from '../lib/logger';

interface AddLinkDialogProps {
  onClose: () => void;
  onAdd: () => void;
}

export default function AddLinkDialog({ onClose, onAdd }: AddLinkDialogProps) {
  const logger = createLoggerForComponent('AddLinkDialog');
  const { t } = useTranslation();
  const [urls, setUrls] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!urls.trim()) return;

    setIsSubmitting(true);
    setError(null);

    const urlList = urls.split('\n').map(u => u.trim()).filter(u => u);
    let successCount = 0;
    let errors: string[] = [];

    try {
      // Process sequentially to avoid overwhelming backend/network
      for (const url of urlList) {
        try {
          await invoke('create_download_link', { url });
          successCount++;
        } catch (err) {
          logger.error(`Failed to add link ${url}:`, err);
          errors.push(`${url}: ${err}`);
        }
      }
      
      if (successCount > 0) {
        onAdd();
        onClose();
      }
      
      if (errors.length > 0) {
        setError(`${t('dialog.addLink.partialError')} ${errors.join('; ')}`);
        // If some failed, keep dialog open with failed URLs? 
        // For simplicity, just close if at least one succeeded, or show error if all failed.
        if (successCount === 0) {
           // Keep open
        }
      }
    } catch (err) {
      logger.error('Failed to add links:', err);
      setError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4">
      <div className="bg-surface-300 rounded-xl w-full max-w-md shadow-2xl ring-1 ring-white/10 p-6">
        <h2 className="text-xl font-bold mb-4 flex items-center gap-2">
          🔗 {t('dialog.addLink.title')}
        </h2>

        {error && (
          <div className="mb-4 p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm">
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium mb-1 text-gray-300">
              {t('dialog.addLink.url')}
            </label>
            <textarea
              value={urls}
              onChange={(e) => setUrls(e.target.value)}
              placeholder={t('dialog.addLink.placeholderMulti')}
              className="w-full px-3 py-2 bg-surface-200 rounded-lg focus:ring-1 focus:ring-accent outline-none min-h-[100px]"
              autoFocus
            />
            <p className="text-xs text-gray-500 mt-1">
              {t('dialog.addLink.autoMetadataHint')}
            </p>
          </div>

          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="btn btn-secondary"
              disabled={isSubmitting}
            >
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              className="btn btn-primary"
              disabled={!urls.trim() || isSubmitting}
            >
              {isSubmitting ? t('common.loading') : t('common.add')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
