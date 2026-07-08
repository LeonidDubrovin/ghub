import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { createLoggerForComponent } from '../lib/logger';
import AddLinkResultDialog from './AddLinkResultDialog';
import type { Game, GameLink, CreateGameFromLinkResponse } from '../types';

interface AddLinkDialogProps {
  onClose: () => void;
  onAdd: () => void;
  onOpenGame: (game: Game, link?: GameLink) => void;
}

export default function AddLinkDialog({ onClose, onAdd, onOpenGame }: AddLinkDialogProps) {
  const logger = createLoggerForComponent('AddLinkDialog');
  const { t } = useTranslation();
  const [urls, setUrls] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<{
    newGames: Game[];
    duplicateGames: Game[];
    duplicateLinks: GameLink[];
    errors: { url: string; error: string }[];
  } | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!urls.trim()) return;

    setIsSubmitting(true);
    setError(null);

    const urlList = urls.split('\n').map(u => u.trim()).filter(u => u);
    const newGames: Game[] = [];
    const duplicateGames: Game[] = [];
    const duplicateLinks: GameLink[] = [];
    const errors: { url: string; error: string }[] = [];

    try {
      // Process sequentially to avoid overwhelming backend/network
      for (const url of urlList) {
        try {
          const response = await invoke<CreateGameFromLinkResponse>('create_game_from_link', { url });
          if (response.is_duplicate) {
            duplicateGames.push(response.game);
            duplicateLinks.push(response.existing_link as GameLink);
          } else {
            newGames.push(response.game);
          }
        } catch (err) {
          logger.error(`Failed to add link ${url}:`, err);
          errors.push({ url, error: String(err) });
        }
      }

      const singleUrl = urlList.length === 1;
      const singleDuplicate = singleUrl && duplicateGames.length === 1 && newGames.length === 0 && errors.length === 0;

      if (singleDuplicate) {
        onOpenGame(duplicateGames[0], duplicateLinks[0]);
        onAdd();
        onClose();
        return;
      }

      if (newGames.length > 0) {
        onAdd();
      }

      if (newGames.length > 0 || duplicateGames.length > 0 || errors.length > 0) {
        setResult({ newGames, duplicateGames, duplicateLinks, errors });
      } else {
        onClose();
      }
    } catch (err) {
      logger.error('Failed to add links:', err);
      setError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  if (result) {
    return (
      <AddLinkResultDialog
        newGames={result.newGames}
        duplicateGames={result.duplicateGames}
        duplicateLinks={result.duplicateLinks}
        errors={result.errors}
        onClose={onClose}
        onOpenGame={onOpenGame}
      />
    );
  }

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
