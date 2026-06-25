import type { Game } from '../types';
import MetadataSearchDialog from './MetadataSearchDialog';

interface BatchMetadataDialogProps {
  games: Game[];
  onClose: () => void;
  onSave: () => void;
}

export default function BatchMetadataDialog({ games, onClose, onSave }: BatchMetadataDialogProps) {
  return (
    <MetadataSearchDialog
      isOpen={true}
      games={games}
      onClose={onClose}
      onSave={onSave}
    />
  );
}
