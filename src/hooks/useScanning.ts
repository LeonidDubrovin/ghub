import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import type { SpaceSource } from '../types';

export interface SourceScanStatus {
  space_id: string;
  source_path: string;
  scan_status?: 'idle' | 'scanning' | 'completed' | 'error';
  scan_progress?: number;
  scan_total?: number;
  scan_error?: string;
  scan_started_at?: string;
  scan_completed_at?: string;
}

/**
 * Hook to get scan status for a specific source
 * Polls every 2 seconds when status is 'scanning'
 */
export function useSourceScanStatus(spaceId: string, sourcePath: string) {
  return useQuery({
    queryKey: ['source_scan_status', spaceId, sourcePath],
    queryFn: async () => {
      return await invoke<SpaceSource>('get_source_scan_status', { spaceId, sourcePath });
    },
    enabled: !!spaceId && !!sourcePath,
    refetchInterval: (query) => {
      const data = query.state.data;
      // Poll every 2 seconds if scanning, otherwise every 10 seconds
      if (data?.scan_status === 'scanning') {
        return 2000;
      }
      return 10000;
    },
    staleTime: 1000,
  });
}

/**
 * Hook to start scanning a source
 */
export function useStartSourceScan() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async ({ spaceId, sourcePath }: { spaceId: string; sourcePath: string }) => {
      return await invoke('start_source_scan', { spaceId, sourcePath });
    },
    onSuccess: (_, variables) => {
      // Invalidate status queries to immediately show "scanning"
      queryClient.invalidateQueries({ queryKey: ['source_scan_status', variables.spaceId, variables.sourcePath] });
      queryClient.invalidateQueries({ queryKey: ['space_sources', variables.spaceId] });
    },
  });
}

/**
 * Hook to cancel a running scan
 */
export function useCancelSourceScan() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async ({ spaceId, sourcePath }: { spaceId: string; sourcePath: string }) => {
      return await invoke('cancel_source_scan', { spaceId, sourcePath });
    },
    onSuccess: (_, variables) => {
      // Invalidate scan status to update UI immediately
      queryClient.invalidateQueries({ queryKey: ['source_scan_status', variables.spaceId, variables.sourcePath] });
      // Also invalidate space_sources to update the source's scan_status field
      queryClient.invalidateQueries({ queryKey: ['space_sources', variables.spaceId] });
    },
  });
}