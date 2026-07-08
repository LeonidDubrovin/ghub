import { useTranslation } from 'react-i18next';
import type { Game, GameLink } from '../types';

interface AddLinkResultDialogProps {
  newGames: Game[];
  duplicateGames: Game[];
  duplicateLinks: GameLink[];
  errors: { url: string; error: string }[];
  onClose: () => void;
  onOpenGame: (game: Game, link?: GameLink) => void;
}

export default function AddLinkResultDialog({
  newGames,
  duplicateGames,
  duplicateLinks,
  errors,
  onClose,
  onOpenGame,
}: AddLinkResultDialogProps) {
  const { t } = useTranslation();

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
      <div className="bg-surface-300 rounded-xl p-6 shadow-lg max-w-lg w-full max-h-[90vh] flex flex-col">
        <h3 className="text-lg font-semibold mb-4">
          {t('dialog.addLinkResult.title')}
        </h3>

        <div className="space-y-6 overflow-y-auto pr-1 mb-4">
          {newGames.length > 0 && (
            <section>
              <h4 className="text-sm font-semibold text-accent mb-2">
                {t('dialog.addLinkResult.added', { count: newGames.length })}
              </h4>
              <ul className="space-y-1">
                {newGames.map((game) => (
                  <li key={game.id}>
                    <button
                      onClick={() => onOpenGame(game)}
                      className="text-left w-full px-3 py-2 rounded bg-surface-400 hover:bg-accent/20 text-sm text-white transition-colors"
                    >
                      {game.title}
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {duplicateGames.length > 0 && (
            <section>
              <h4 className="text-sm font-semibold text-warning mb-2">
                {t('dialog.addLinkResult.duplicates', { count: duplicateGames.length })}
              </h4>
              <ul className="space-y-2">
                {duplicateGames.map((game, index) => {
                  const link = duplicateLinks[index];
                  return (
                    <li
                      key={game.id}
                      className="flex items-center justify-between gap-3 px-3 py-2 rounded bg-surface-400 text-sm"
                    >
                      <div className="min-w-0">
                        <div className="font-medium text-white truncate">{game.title}</div>
                        {link && (
                          <div className="text-xs text-gray-400 truncate">{link.url}</div>
                        )}
                      </div>
                      <button
                        onClick={() => onOpenGame(game, link)}
                        className="btn btn-sm btn-primary shrink-0"
                      >
                        {t('dialog.addLinkResult.openCard')}
                      </button>
                    </li>
                  );
                })}
              </ul>
            </section>
          )}

          {errors.length > 0 && (
            <section>
              <h4 className="text-sm font-semibold text-danger mb-2">
                {t('dialog.addLinkResult.errors', { count: errors.length })}
              </h4>
              <ul className="space-y-2">
                {errors.map(({ url, error }) => (
                  <li
                    key={url}
                    className="px-3 py-2 rounded bg-danger/10 border border-danger/30 text-sm"
                  >
                    <div className="text-xs text-gray-400 break-all">{url}</div>
                    <div className="text-danger text-xs mt-1">{error}</div>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </div>

        <div className="flex justify-end pt-2 border-t border-surface-100">
          <button onClick={onClose} className="btn btn-secondary">
            {t('dialog.addLinkResult.close')}
          </button>
        </div>
      </div>
    </div>
  );
}
