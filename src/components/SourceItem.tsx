import { useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useSourceScanStatus, useStartSourceScan, useCancelSourceScan } from '../hooks/useScanning';
import { useRemoveSpaceSource } from '../hooks/useSpaces';
import type { SpaceSource } from '../types';
import clsx from 'clsx';

interface SourceItemProps {
  spaceId: string;
  source: SpaceSource;
  isSourceSelected: boolean;
  onSelectSource: (path: string | null) => void;
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

const FolderIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
  </svg>
);

export default function SourceItem({
  spaceId,
  source,
  isSourceSelected,
  onSelectSource
}: SourceItemProps) {
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
   
  const handleStartScan = (e: React.MouseEvent) => {
    e.stopPropagation();
    startScan.mutate({ spaceId, sourcePath: source.source_path });
  };
   
  const handleCancelScan = (e: React.MouseEvent) => {
    e.stopPropagation();
    cancelScan.mutate({ spaceId, sourcePath: source.source_path });
  };
   
  const handleRemove = (e: React.MouseEvent) => {
    e.stopPropagation();
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

  const handleClick = () => {
    onSelectSource(isSourceSelected ? null : source.source_path);
  };

  // Get folder name from path
  const folderName = useMemo(() => {
    const parts = source.source_path.split(/[\\/]/).filter(Boolean);
    return parts.pop() || source.source_path;
  }, [source.source_path]);

  return (
    <div
      onClick={handleClick}
      className={clsx(
        'source-item flex items-center gap-3 p-2.5 rounded-lg transition-all cursor-pointer group border',
        isSourceSelected
          ? 'bg-accent/15 border-accent/40 shadow-sm'
          : source.is_active
            ? 'bg-surface-200 border-transparent hover:bg-surface-100 hover:border-surface-300'
            : 'bg-surface-400/50 border-transparent hover:bg-surface-400 opacity-70'
      )}
    >
      {/* Folder icon with status indicator */}
      <div className="relative flex-shrink-0">
        <div className={clsx(
          'w-8 h-8 rounded-lg flex items-center justify-center',
          isSourceSelected ? 'bg-accent/30' : 'bg-surface-300'
        )}>
          <FolderIcon />
        </div>
        {/* Status dot */}
        <div className={clsx(
          'absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2 border-surface-400',
          isScanning && 'bg-yellow-500',
          isError && 'bg-red-500',
          isCompleted && 'bg-green-500',
          isIdle && source.is_active && 'bg-green-500',
          !source.is_active && 'bg-gray-400'
        )} />
      </div>

      {/* Path info */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className={clsx(
            'text-sm font-medium truncate',
            isSourceSelected ? 'text-accent' : 'text-gray-200'
          )} title={source.source_path}>
            {folderName}
          </span>
        </div>
        
        {/* Full path hint on hover */}
        <div className="text-xs text-gray-500 truncate group-hover:text-gray-400 transition-colors">
          {source.source_path}
        </div>
        
        {/* Scan progress */}
        {isScanning && scanStatus?.scan_total && (
          <div className="mt-1.5">
            <div className="flex items-center gap-2 text-xs">
              <span className="text-yellow-500">{t('space.scanning')}</span>
              <span className="text-gray-400">{scanStatus.scan_progress}/{scanStatus.scan_total}</span>
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
        {error && (
          <div className="mt-1 text-xs text-danger">
            {error}
          </div>
        )}
      </div>

      {/* Action buttons */}
      <div className="flex items-center gap-1 flex-shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
        {isScanning ? (
          <button
            onClick={handleCancelScan}
            className="p-1.5 rounded hover:bg-surface-100 text-yellow-500 transition-colors"
            title={t('space.cancelScan')}
            disabled={cancelScan.isPending}
          >
            <StopIcon />
          </button>
        ) : (
          <button
            onClick={handleStartScan}
            className="p-1.5 rounded hover:bg-surface-100 text-green-500 transition-colors"
            title={t('space.startScan')}
            disabled={startScan.isPending || !source.is_active}
          >
            <PlayIcon />
          </button>
        )}
        
        <button
          onClick={handleRemove}
          className="p-1.5 rounded hover:bg-red-500/20 text-danger transition-colors"
          title={t('space.removeSource')}
          disabled={removeSource.isPending || isScanning}
        >
          <TrashIcon />
        </button>
      </div>
    </div>
  );
}