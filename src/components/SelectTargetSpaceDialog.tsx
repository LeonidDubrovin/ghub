import { useTranslation } from 'react-i18next';
import type { Space } from '../types';
import { useSpaceSources } from '../hooks/useSpaces';

interface SelectTargetSpaceDialogProps {
  spaces: Space[];
  onClose: () => void;
  onSelect: (spaceId: string, sourcePath: string) => void;
}

const FolderIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
  </svg>
);

export default function SelectTargetSpaceDialog({ spaces, onClose, onSelect }: SelectTargetSpaceDialogProps) {
  const { t } = useTranslation();
  const eligibleSpaces = spaces.filter(s => !s.is_system);

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-surface-300 rounded-xl p-6 shadow-lg max-w-md w-full">
        <h3 className="text-lg font-semibold mb-4">{t('dialog.selectTargetSpace.title')}</h3>
        <p className="text-sm text-gray-400 mb-4">{t('dialog.selectTargetSpace.description')}</p>

        {eligibleSpaces.length === 0 ? (
          <p className="text-sm text-gray-400 mb-4">{t('dialog.selectTargetSpace.noSpaces')}</p>
        ) : (
          <div className="space-y-3 max-h-80 overflow-y-auto mb-4">
            {eligibleSpaces.map(space => (
              <SpaceTargetItem
                key={space.id}
                space={space}
                onSelect={(sourcePath) => onSelect(space.id, sourcePath)}
              />
            ))}
          </div>
        )}

        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="btn btn-secondary">
            {t('common.cancel')}
          </button>
        </div>
      </div>
    </div>
  );
}

function SpaceTargetItem({
  space,
  onSelect,
}: {
  space: Space;
  onSelect: (sourcePath: string) => void;
}) {
  const { t } = useTranslation();
  const { data: sources = [] } = useSpaceSources(space.id);
  const activeSources = sources.filter(s => s.is_active);

  return (
    <div className="border border-surface-100 rounded-lg p-3">
      <div className="flex items-center gap-2 mb-2 text-gray-200">
        <FolderIcon />
        <span className="font-medium">{space.name}</span>
      </div>
      {activeSources.length === 0 ? (
        <p className="text-xs text-gray-500">{t('dialog.selectTargetSpace.noSources')}</p>
      ) : (
        <div className="space-y-1">
          {activeSources.map(source => (
            <button
              key={source.source_path}
              onClick={() => onSelect(source.source_path)}
              className="w-full text-left px-3 py-2 rounded bg-surface-400 hover:bg-accent/20 text-sm text-gray-300 truncate"
              title={source.source_path}
            >
              {source.source_path}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
