import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

interface SettingsDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function SettingsDialog({ isOpen, onClose }: SettingsDialogProps) {
  const { t } = useTranslation();
  const [apiKey, setApiKey] = useState('');
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setApiKey('');
    setMessage(null);
    let cancelled = false;
    const load = async () => {
      try {
        const key = await invoke<string | null>('get_itch_api_key');
        if (!cancelled) setApiKey(key || '');
      } catch (e) {
        console.error('Failed to load API key:', e);
      }
    };
    load();
    return () => { cancelled = true; };
  }, [isOpen]);

  if (!isOpen) return null;

  const handleSave = async () => {
    setLoading(true);
    setMessage(null);
    try {
      if (apiKey.trim()) {
        await invoke('set_itch_api_key', { apiKey: apiKey.trim() });
      } else {
        await invoke('delete_itch_api_key');
      }
      setMessage(t('settings.saved'));
    } catch (e) {
      console.error('Failed to save API key:', e);
      alert(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async () => {
    setLoading(true);
    setMessage(null);
    try {
      await invoke('delete_itch_api_key');
      setApiKey('');
      setMessage(t('settings.deleted'));
    } catch (e) {
      console.error('Failed to delete API key:', e);
      alert(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-surface-300 rounded-xl p-6 shadow-lg max-w-md w-full">
        <h3 className="text-lg font-semibold mb-4">{t('settings.title')}</h3>

        <div className="mb-4">
          <label className="block text-sm text-gray-400 mb-1">{t('settings.itchApiKey')}</label>
          <input
            type="password"
            value={apiKey}
            onChange={e => setApiKey(e.target.value)}
            placeholder={t('settings.itchApiKeyPlaceholder')}
            className="w-full px-3 py-2 bg-surface-200 rounded-lg text-sm text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-accent"
          />
          <p className="text-xs text-gray-500 mt-1">{t('settings.itchApiKeyHint')}</p>
        </div>

        {message && (
          <div className="mb-4 text-sm text-green-400 bg-green-500/10 rounded-lg px-3 py-2">
            {message}
          </div>
        )}

        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="btn btn-secondary">
            {t('common.close')}
          </button>
          <button
            onClick={handleDelete}
            disabled={loading || !apiKey}
            className="btn btn-secondary text-red-400"
          >
            {t('common.delete')}
          </button>
          <button
            onClick={handleSave}
            disabled={loading}
            className="btn btn-primary"
          >
            {loading ? t('common.saving') : t('common.save')}
          </button>
        </div>
      </div>
    </div>
  );
}
