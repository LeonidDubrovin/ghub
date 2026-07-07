import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Install } from '../types';

interface DeleteInstallDialogProps {
  install: Install | null;
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (deleteFiles: boolean) => Promise<void>;
  isPending?: boolean;
}

export default function DeleteInstallDialog({
  install,
  isOpen,
  onClose,
  onConfirm,
  isPending = false,
}: DeleteInstallDialogProps) {
  const { t } = useTranslation();
  const [deleteFiles, setDeleteFiles] = useState(true);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen || !install) return null;

  const handleConfirm = async () => {
    setError(null);
    try {
      await onConfirm(deleteFiles);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-surface-300 rounded-xl w-full max-w-md shadow-2xl">
        {/* Header */}
        <div className="p-4 border-b border-surface-100 flex items-center justify-between">
          <h2 className="text-lg font-semibold">{t('details.deleteInstallTitle')}</h2>
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
            {t('details.deleteInstallMessage', {
              version: install.version || install.install_path,
            })}
          </p>

          <label className="flex items-start gap-3 p-3 bg-surface-200 rounded-lg cursor-pointer select-none">
            <input
              type="checkbox"
              checked={deleteFiles}
              onChange={e => setDeleteFiles(e.target.checked)}
              disabled={isPending}
              className="mt-0.5 rounded bg-surface-400 border-none text-accent focus:ring-0"
            />
            <span className="text-sm text-gray-200">{t('details.deleteInstallFiles')}</span>
          </label>

          {error && (
            <div className="p-3 bg-danger/20 border border-danger/50 rounded-lg text-danger text-sm">
              ❌ {error}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-surface-100 flex justify-end gap-3">
          <button
            onClick={onClose}
            disabled={isPending}
            className="btn btn-secondary disabled:opacity-50"
          >
            {t('details.deleteInstallCancel')}
          </button>
          <button
            onClick={handleConfirm}
            disabled={isPending}
            className="btn bg-danger/20 text-danger border border-danger/30 hover:bg-danger/30 disabled:opacity-50"
          >
            {isPending ? t('common.loading') : t('details.deleteInstallConfirm')}
          </button>
        </div>
      </div>
    </div>
  );
}
