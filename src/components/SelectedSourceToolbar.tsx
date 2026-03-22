import { useTranslation } from 'react-i18next';
import { useStartSourceScan, useCancelSourceScan, useSourceScanStatus } from '../hooks/useScanning';
import { useRemoveSpaceSource, useSpaceSources } from '../hooks/useSpaces';
import type { SelectedSource, SpaceSource } from '../types';

const PlayIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
  </svg>
);

const StopIcon = () => (
  <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
    <rect x="6" y="6" width="12" height="12" />
  </svg>
);

const TrashIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
  </svg>
);

interface SelectedSourceToolbarProps {
  selectedSource: SelectedSource;
  onClose: () => void;
}

export default function SelectedSourceToolbar({ selectedSource, onClose }: SelectedSourceToolbarProps) {
  const { t } = useTranslation();
  const { data: scanStatus } = useSourceScanStatus(selectedSource.spaceId, selectedSource.sourcePath);
  const { data: sources = [] } = useSpaceSources(selectedSource.spaceId);
  const startScan = useStartSourceScan();
  const cancelScan = useCancelSourceScan();
  const removeSource = useRemoveSpaceSource();

  // Find the specific source to check if it's active
  const sourceData: SpaceSource | undefined = sources.find(s => s.source_path === selectedSource.sourcePath);
  const isSourceActive = sourceData?.is_active ?? true;

  const isScanning = scanStatus?.scan_status === 'scanning';
  const isCompleted = scanStatus?.scan_status === 'completed';
  const isError = scanStatus?.scan_status === 'error';

  const handleStartScan = () => {
    if (!isSourceActive) {
      alert(t('space.sourceInactiveWarning') || 'This source is inactive and cannot be scanned.');
      return;
    }
    startScan.mutate({ spaceId: selectedSource.spaceId, sourcePath: selectedSource.sourcePath });
  };

  const handleCancelScan = async () => {
    if (!window.confirm(t('space.confirmCancelScan') || 'Cancel the current scan?')) {
      return;
    }
    try {
      await cancelScan.mutateAsync({
        spaceId: selectedSource.spaceId,
        sourcePath: selectedSource.sourcePath
      });
    } catch (err) {
      console.error('Failed to cancel scan:', err);
      alert(t('space.cancelScanError') || 'Failed to cancel scan');
    }
  };

  const handleRemove = () => {
    if (window.confirm(t('space.confirmRemoveSource', { path: selectedSource.sourcePath }))) {
      removeSource.mutate({
        space_id: selectedSource.spaceId,
        source_path: selectedSource.sourcePath,
      });
    }
  };

  // Get folder name for display
  const folderName = selectedSource.sourcePath.split(/[\\/]/).filter(Boolean).pop() || selectedSource.sourcePath;

  // Determine status message and progress
  const getStatusMessage = () => {
    if (isScanning && scanStatus?.scan_total) {
      const percent = Math.round((scanStatus.scan_progress! / scanStatus.scan_total!) * 100);
      return t('space.scanningProgress', { progress: scanStatus.scan_progress, total: scanStatus.scan_total, percent });
    } else if (isScanning) {
      return t('space.scanning');
    } else if (isError) {
      return t('space.scanError');
    } else if (isCompleted) {
      return t('space.scanCompleted');
    } else {
      return t('space.readyToScan');
    }
  };

  return (
    <div className="flex items-center gap-3 bg-surface-300 border-b border-surface-100 px-4 py-2">
      {/* Selected source info */}
      <div className="flex items-center gap-2 pr-2 border-r border-surface-100">
        <div className="w-8 h-8 rounded-lg bg-surface-200 flex items-center justify-center">
          <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
          </svg>
        </div>
        <div className="max-w-[200px]">
          <p className="text-sm font-medium truncate" title={folderName}>{folderName}</p>
          <p className="text-xs text-gray-500 truncate" title={selectedSource.sourcePath}>{selectedSource.sourcePath}</p>
        </div>
      </div>

      {/* Status message */}
      <div className="flex flex-col min-w-[120px]">
        <p className="text-xs font-medium text-gray-300">
          {getStatusMessage()}
        </p>
        {/* Progress bar when scanning */}
        {isScanning && scanStatus?.scan_total && (
          <div className="mt-1 h-1.5 bg-surface-100 rounded-full overflow-hidden">
            <div
              className="h-full bg-accent transition-all duration-300"
              style={{
                width: `${(scanStatus.scan_progress! / scanStatus.scan_total!) * 100}%`
              }}
            />
          </div>
        )}
        {/* Error message */}
        {isError && scanStatus?.scan_error && (
          <p className="text-xs text-danger mt-1 truncate" title={scanStatus.scan_error}>
            {scanStatus.scan_error}
          </p>
        )}
      </div>

      {/* Action buttons */}
      <div className="flex items-center gap-2">
        {isScanning ? (
          <button
            onClick={handleCancelScan}
            disabled={cancelScan.isPending}
            className="btn btn-secondary flex items-center gap-2 px-3 py-2 text-sm bg-yellow-500/20 text-yellow-500 border border-yellow-500/30 hover:bg-yellow-500/30 disabled:opacity-50"
            title={t('space.cancelScan')}
          >
            <StopIcon />
            <span>{t('space.cancelScan')}</span>
          </button>
        ) : (
          <button
            onClick={handleStartScan}
            disabled={startScan.isPending || !isSourceActive}
            className="btn btn-primary flex items-center gap-2 px-3 py-2 text-sm disabled:opacity-50"
            title={isSourceActive ? t('space.startScan') : t('space.sourceInactiveWarning')}
          >
            <PlayIcon />
            <span>{t('space.scan')}</span>
          </button>
        )}

        <button
          onClick={handleRemove}
          disabled={removeSource.isPending || isScanning}
          className="btn flex items-center gap-2 px-3 py-2 text-sm bg-danger/20 text-danger border border-danger/30 hover:bg-danger/30 disabled:opacity-50"
          title={t('space.removeSource')}
        >
          <TrashIcon />
          <span>{t('space.remove')}</span>
        </button>
      </div>

      {/* Close button */}
      <button
        onClick={onClose}
        className="p-1 hover:bg-surface-100 rounded transition-colors text-gray-400 hover:text-gray-200"
        title={t('common.close')}
      >
        ✕
      </button>
    </div>
  );
}
