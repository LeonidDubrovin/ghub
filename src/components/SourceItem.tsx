import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useSourceScanStatus, useStartSourceScan, useCancelSourceScan } from '../hooks/useScanning';
import { useRemoveSpaceSource } from '../hooks/useSpaces';
import type { SpaceSource } from '../types';
import clsx from 'clsx';

interface SourceItemProps {
  spaceId: string;
  source: SpaceSource;
}

// Icons
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

const CheckIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
  </svg>
);

const FolderIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
  </svg>
);

const AlertIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
  </svg>
);

export default function SourceItem({ spaceId, source }: SourceItemProps) {
  const { t } = useTranslation();
  const { data: scanStatus } = useSourceScanStatus(spaceId, source.source_path);
  const startScan = useStartSourceScan();
  const cancelScan = useCancelSourceScan();
  const removeSource = useRemoveSpaceSource();
  
  const [error, setError] = useState<string | null>(null);
  
  const isScanning = scanStatus?.scan_status === 'scanning';
  const isCompleted = scanStatus?.scan_status === 'completed';
  const isError = scanStatus?.scan_status === 'error';
  const isIdle = !scanStatus?.scan_status || scanStatus?.scan_status === 'idle';
  
  const handleStartScan = () => {
    startScan.mutate({ spaceId, sourcePath: source.source_path });
  };
  
  const handleCancelScan = () => {
    cancelScan.mutate({ spaceId, sourcePath: source.source_path });
  };
  
  const handleRemove = () => {
    if (window.confirm(t('space.confirmRemoveSource', { path: source.source_path }))) {
      setError(null);
      removeSource.mutate(
        { space_id: spaceId, source_path: source.source_path },
        {
          onError: (err: unknown) => {
            const message = err instanceof Error ? err.message : String(err);
            setError(t('space.removeSourceError', { message }) || `Failed to remove source: ${message}`);
          }
        }
      );
    }
  };

  return (
    <div className={clsx(
      'source-item flex items-center gap-2 p-2 rounded-lg transition-colors',
      source.is_active ? 'bg-surface-200' : 'bg-surface-400 opacity-60'
    )}>
      {/* Status icon */}
      <span className="flex-shrink-0">
        {isScanning && <AlertIcon />}
        {isError && <AlertIcon />}
        {isCompleted && <CheckIcon />}
        {isIdle && source.is_active && <CheckIcon />}
        {!source.is_active && <span className="text-gray-500">○</span>}
      </span>
      
      {/* Path */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1">
          <FolderIcon />
          <span className="text-sm font-medium truncate" title={source.source_path}>
            {source.source_path.split(/[\\/]/).pop()}
          </span>
        </div>
        
        {/* Scan progress */}
        {isScanning && scanStatus?.scan_total && (
          <div className="mt-1">
            <div className="flex items-center gap-2 text-xs text-gray-400">
              <span>{t('space.scanning')}</span>
              <span>{scanStatus.scan_progress}/{scanStatus.scan_total}</span>
            </div>
            <div className="mt-1 h-1.5 bg-surface-100 rounded-full overflow-hidden">
              <div
                className="h-full bg-accent transition-all duration-300"
                style={{
                  width: `${(scanStatus.scan_progress! / scanStatus.scan_total!) * 100}%`
                }}
              />
            </div>
          </div>
        )}
        
        {/* Error message */}
        {isError && scanStatus?.scan_error && (
          <div className="mt-1 text-xs text-danger">
            {scanStatus.scan_error}
          </div>
        )}
      {/* Error message */}
      {error && (
        <div className="mt-1 text-xs text-danger">
          {error}
        </div>
      )}
      </div> {/* Close flex-1 min-w-0 */}
      
      {/* Action buttons */}
      <div className="flex items-center gap-1 flex-shrink-0">
        {isScanning ? (
          <button
            onClick={handleCancelScan}
            className="p-1.5 rounded hover:bg-surface-100 text-yellow-500"
            title={t('space.cancelScan')}
            disabled={cancelScan.isPending}
          >
            <StopIcon />
          </button>
        ) : (
          <button
            onClick={handleStartScan}
            className="p-1.5 rounded hover:bg-surface-100 text-green-500"
            title={t('space.startScan')}
            disabled={startScan.isPending || !source.is_active}
          >
            <PlayIcon />
          </button>
        )}
        
        <button
          onClick={handleRemove}
          className="p-1.5 rounded hover:bg-surface-100 text-danger"
          title={t('space.removeSource')}
          disabled={removeSource.isPending || isScanning}
        >
          <TrashIcon />
        </button>
      </div>
    </div>
  );
}