import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useSourceScanStatus } from '../hooks/useScanning';
import type { SpaceSource } from '../types';
import clsx from 'clsx';

interface SourceItemProps {
  spaceId: string;
  source: SpaceSource;
  isSourceSelected: boolean;
  onSelectSource: (path: string | null) => void;
}

// Icons removed - scanning is controlled from toolbar

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
  
  const isScanning = scanStatus?.scan_status === 'scanning';
  const isCompleted = scanStatus?.scan_status === 'completed';
  const isError = scanStatus?.scan_status === 'error';
  const isIdle = !scanStatus?.scan_status || scanStatus?.scan_status === 'idle';
 
  // Scanning is controlled from the SelectedSourceToolbar when this source is selected

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
        'source-item flex items-center gap-3 p-2.5 rounded-lg transition-all cursor-pointer group border select-none',
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
        {isScanning && (
          <div className="mt-1.5">
            <div className="flex items-center gap-2 text-xs">
              <span className="text-yellow-500">{t('space.scanning')}</span>
              {scanStatus?.scan_total && scanStatus.scan_total > 0 ? (
                <span className="text-gray-400">{scanStatus.scan_progress}/{scanStatus.scan_total}</span>
              ) : scanStatus?.scan_progress && scanStatus.scan_progress > 0 ? (
                <span className="text-gray-400">{t('space.scanningWithCount', { count: scanStatus.scan_progress })}</span>
              ) : null}
            </div>
            <div className="mt-1 h-1.5 bg-surface-100 rounded-full overflow-hidden">
              {scanStatus?.scan_total && scanStatus.scan_total > 0 ? (
                <div
                  className="h-full bg-accent transition-all duration-300"
                  style={{
                    width: `${(scanStatus.scan_progress! / scanStatus.scan_total!) * 100}%`
                  }}
                />
              ) : (
                <div className="h-full bg-accent animate-pulse" style={{ width: '100%' }} />
              )}
            </div>
          </div>
        )}
        
        {/* Error message */}
        {isError && scanStatus?.scan_error && (
          <div className="mt-1 text-xs text-danger">
            {scanStatus.scan_error}
          </div>
        )}
        {/* Error from scan status is shown above */}
      </div>

      {/* Scanning is controlled from the toolbar when source is selected */}
    </div>
  );
}