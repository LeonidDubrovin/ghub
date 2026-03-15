import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import type { DownloadLink } from '../types';

export default function DownloadLinksView() {
  const { t } = useTranslation();
  const [links, setLinks] = useState<DownloadLink[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  const loadLinks = async () => {
    setIsLoading(true);
    try {
      const data = await invoke<DownloadLink[]>('get_download_links');
      setLinks(data);
    } catch (err) {
      console.error('Failed to load links:', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadLinks();
  }, []);

  const handleDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm(t('links.confirmDelete'))) return;
    try {
      await invoke('delete_download_link', { id });
      loadLinks();
    } catch (err) {
      console.error('Failed to delete link:', err);
    }
  };

  if (isLoading) {
    return <div className="flex-1 flex items-center justify-center text-gray-500">{t('common.loading')}</div>;
  }

  if (links.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-gray-500">
        <span className="text-4xl mb-4">🔗</span>
        <p>{t('links.noLinks')}</p>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-4">
        {links.map(link => (
          <div key={link.id} className="bg-surface-300 rounded-xl overflow-hidden shadow-lg group hover:ring-2 hover:ring-accent transition-all">
            <div className="flex h-32">
              {/* Cover */}
              <div className="w-24 bg-black/20 flex-shrink-0 relative">
                {link.cover_url ? (
                  <img src={link.cover_url} alt="" className="w-full h-full object-cover" />
                ) : (
                  <div className="w-full h-full flex items-center justify-center text-2xl opacity-20">?</div>
                )}
                <div className="absolute top-1 left-1 bg-black/60 text-white text-[10px] px-1 rounded uppercase">
                  {link.status}
                </div>
              </div>
              
              {/* Info */}
              <div className="flex-1 p-3 flex flex-col min-w-0">
                <div className="flex justify-between items-start gap-2">
                  <h3 className="font-bold text-white truncate" title={link.title}>{link.title}</h3>
                  <button onClick={(e) => handleDelete(link.id, e)} className="text-gray-500 hover:text-red-500 transition-colors">✕</button>
                </div>
                
                <div className="text-xs text-gray-400 line-clamp-2 mb-auto mt-1">
                  {link.description || t('links.noDescription')}
                </div>
                
                <div className="mt-2 flex items-center justify-between">
                  <span className="text-[10px] text-gray-500">{link.added_at}</span>
                  <a href={link.url} target="_blank" rel="noreferrer" className="text-xs text-accent hover:underline flex items-center gap-1">
                    Open ↗
                  </a>
                </div>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
