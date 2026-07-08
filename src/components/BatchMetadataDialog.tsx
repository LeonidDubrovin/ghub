import type { Game, GameLink } from '../types';
import MetadataSearchDialog from './MetadataSearchDialog';

interface BatchMetadataDialogProps {
  games: Game[];
  onClose: () => void;
  onSave: () => void;
  onOpenGame?: (game: Game, link?: GameLink) => void;
}

export default function BatchMetadataDialog({ games, onClose, onSave, onOpenGame }: BatchMetadataDialogProps) {
  return (
    <MetadataSearchDialog
      isOpen={true}
      games={games}
      onClose={onClose}
      onSave={onSave}
      onOpenGame={onOpenGame}
    />
  );
}
