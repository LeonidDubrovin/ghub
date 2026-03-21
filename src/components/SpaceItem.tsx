import { useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { Space, SpaceSource } from '../types';
import clsx from 'clsx';
import { useSpaceSources } from '../hooks/useSpaces';
import { useAddSpaceSource } from '../hooks/useSpaces';
import { open } from '@tauri-apps/plugin-dialog';
import SourceItem from './SourceItem';

// Icon components
const FolderIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
  </svg>
);

const ChevronRightIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
  </svg>
);

const ChevronDownIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
  </svg>
);

const CheckIcon = () => (
  <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
  </svg>
);

const SteamIcon = () => (
  <svg className="w-4 h-4" viewBox="0 0 24 24" fill="currentColor">
    <path d="M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.004.105.004.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 11.999-5.373 11.999-12S18.605 0 11.979 0z"/>
  </svg>
);

const ItchIcon = () => (
  <svg className="w-4 h-4" viewBox="0 0 24 24" fill="currentColor">
    <path d="M3.13 1.338C2.08 1.96.02 4.328 0 4.95v1.03c0 1.303 1.22 2.45 2.325 2.45 1.33 0 2.436-1.102 2.436-2.41 0 1.308 1.07 2.41 2.4 2.41 1.328 0 2.362-1.102 2.362-2.41 0 1.308 1.137 2.41 2.466 2.41h.024c1.33 0 2.466-1.102 2.466-2.41 0 1.308 1.034 2.41 2.363 2.41 1.33 0 2.4-1.102 2.4-2.41 0 1.308 1.106 2.41 2.435 2.41C22.78 8.43 24 7.282 24 5.98V4.95c-.02-.62-2.08-2.99-3.13-3.612-.542-.332-1.37-.603-2.166-.603H5.296c-.797 0-1.624.27-2.166.603z"/>
  </svg>
);

const SettingsIcon = () => (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
  </svg>
);

interface SpaceItemProps {
  space: Space;
  isSelected: boolean;
  onSelect: (id: string) => void;
  onSettings: (space: Space, e: React.MouseEvent) => void;
}

export default function SpaceItem({ space, isSelected, onSelect, onSettings }: SpaceItemProps) {
  const { t } = useTranslation();
  const { data: sources = [] } = useSpaceSources(space.id);
  const [isExpanded, setIsExpanded] = useState(false);
  const addSpaceSource = useAddSpaceSource();

  const activeSourceCount = useMemo(() =>
    sources.filter(s => s.is_active).length,
    [sources]
  );
  
  const handleAddSource = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: true,
        title: t('space.selectFolders'),
      });
      
      if (selected && Array.isArray(selected)) {
        // Add each selected path as a source
        for (const path of selected) {
          await addSpaceSource.mutateAsync({
            space_id: space.id,
            source_path: path,
            scan_recursively: true,
          });
        }
      }
    } catch (err) {
      console.error('Failed to select folder:', err);
    }
  };
  
  const handleSourceRemoved = () => {
    // Optional: refresh or update UI
  };

  const getSpaceIcon = (type: string) => {
    switch (type) {
      case 'steam': return <SteamIcon />;
      case 'itch': return <ItchIcon />;
      default: return <FolderIcon />;
    }
  };

  const toggleExpand = (e: React.MouseEvent) => {
    e.stopPropagation();
    setIsExpanded(prev => !prev);
  };

  const truncatePath = (path: string, maxLength: number = 30): string => {
    if (path.length <= maxLength) return path;
    const parts = path.split(/[\\/]/);
    if (parts.length <= 2) {
      return '...' + path.slice(-maxLength);
    }
    return '.../' + parts.slice(-2).join('/');
  };

  return (
    <div className="space-item-container">
      <button
        onClick={() => onSelect(space.id)}
        className={clsx('sidebar-item w-full group relative', isSelected && 'active')}
      >
        <span
          className="w-6 h-6 flex items-center justify-center rounded"
          style={{ backgroundColor: space.color || undefined }}
        >
          {space.icon || getSpaceIcon(space.type)}
        </span>
        <span className="truncate flex-1 text-left">{space.name}</span>
        
        {/* Source count badge */}
        {activeSourceCount > 0 && (
          <span className="text-xs bg-accent/20 text-accent px-1.5 py-0.5 rounded ml-1">
            {activeSourceCount}
          </span>
        )}
        
        {/* Expand button */}
        {sources.length > 0 && (
          <button
            onClick={toggleExpand}
            className="p-1 opacity-0 group-hover:opacity-100 transition-opacity"
            title={isExpanded ? t('space.collapseSources') : t('space.expandSources')}
          >
            {isExpanded ? <ChevronDownIcon /> : <ChevronRightIcon />}
          </button>
        )}
        
        {/* Settings button */}
        <button
          onClick={(e) => onSettings(space, e)}
          className="p-1 opacity-0 group-hover:opacity-100 transition-opacity"
          title={t('space.settings')}
        >
          <SettingsIcon />
        </button>
      </button>

      {/* Sources list (expanded) */}
      {isExpanded && (
        <div className="ml-8 mt-2 space-y-2 border-l-2 border-surface-100 pl-2">
          {sources.map(source => (
            <SourceItem
              key={source.source_path}
              spaceId={space.id}
              source={source}
              onRemoved={handleSourceRemoved}
            />
          ))}
          
          {/* Add source button */}
          <button
            onClick={handleAddSource}
            className="w-full p-2 text-sm text-accent hover:bg-surface-200 rounded-lg border border-dashed border-accent/50 transition-colors"
            title={t('space.addSource')}
          >
            + {t('space.addSource')}
          </button>
        </div>
      )}
    </div>
  );
}