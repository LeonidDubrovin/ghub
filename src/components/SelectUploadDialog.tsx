import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-shell';
import type { ItchUpload } from '../types';

interface SelectUploadDialogProps {
  uploads: ItchUpload[];
  isLoading?: boolean;
  linkUrl?: string;
  onClose: () => void;
  onSelect: (upload: ItchUpload) => void;
}

export default function SelectUploadDialog({ uploads, isLoading, linkUrl, onClose, onSelect }: SelectUploadDialogProps) {
  const { t } = useTranslation();

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
  };

  const formatDate = (value: string | null) => {
    if (!value) return null;
    const d = new Date(value);
    if (isNaN(d.getTime())) return null;
    return d.toLocaleDateString();
  };

  const platformTags = (upload: ItchUpload) => {
    const p = upload.platforms || {};
    const tags: string[] = [];
    if (p.windows) tags.push('Windows');
    if (p.osx) tags.push('macOS');
    if (p.linux) tags.push('Linux');
    if (p.android) tags.push('Android');
    return tags;
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-surface-300 rounded-xl p-6 shadow-lg max-w-lg w-full">
        <h3 className="text-lg font-semibold mb-2">{t('dialog.selectUpload.title')}</h3>
        <p className="text-sm text-gray-400 mb-4">{t('dialog.selectUpload.description')}</p>

        {isLoading ? (
          <div className="flex flex-col items-center justify-center py-10 mb-4 text-gray-400">
            <div className="w-8 h-8 border-2 border-accent/30 border-t-accent rounded-full animate-spin mb-3" />
            <p className="text-sm">{t('dialog.selectUpload.loading')}</p>
          </div>
        ) : uploads.length === 0 ? (
          <div className="mb-4 space-y-2">
            <p className="text-sm text-gray-400">{t('dialog.selectUpload.noUploads')}</p>
            <p className="text-sm text-gray-500">{t('dialog.selectUpload.noUploadsDescription')}</p>
            {linkUrl && (
              <button
                onClick={() => open(linkUrl)}
                className="btn btn-primary"
              >
                {t('dialog.selectUpload.openPage')}
              </button>
            )}
          </div>
        ) : (
          <div className="space-y-2 max-h-80 overflow-y-auto mb-4">
            {uploads.map(upload => {
              const date = formatDate(upload.created_at);
              return (
                <button
                  key={upload.id}
                  onClick={() => onSelect(upload)}
                  className="w-full text-left px-4 py-3 rounded-lg bg-surface-400 hover:bg-accent/20 border border-surface-100 transition-colors"
                >
                  <div className="flex items-center justify-between">
                    <span className="font-medium text-white">
                      {upload.display_name || upload.filename}
                    </span>
                    <span className="text-xs text-gray-400">{formatBytes(upload.size)}</span>
                  </div>
                  <div className="flex items-center gap-2 mt-1 flex-wrap">
                    {platformTags(upload).map(tag => (
                      <span key={tag} className="text-xs px-2 py-0.5 bg-surface-200 rounded text-gray-300">
                        {tag}
                      </span>
                    ))}
                    <span className="text-xs text-gray-500">{upload.filename}</span>
                    {date && (
                      <span className="text-xs text-gray-500 ml-auto">
                        {t('dialog.selectUpload.addedAt', { date })}
                      </span>
                    )}
                  </div>
                </button>
              );
            })}
          </div>
        )}

        <div className="flex justify-end">
          <button onClick={onClose} className="btn btn-secondary">
            {t('common.cancel')}
          </button>
        </div>
      </div>
    </div>
  );
}
