import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import type { Game, CreateGameRequest, ScannedGame } from '../types';

export function useGames(spaceId: string | null, sourcePath?: string) {
  return useQuery({
    queryKey: ['games', spaceId, sourcePath],
    queryFn: async () => {
      if (spaceId && sourcePath) {
        return await invoke<Game[]>('get_games_by_source', { spaceId, sourcePath });
      }
      if (spaceId) {
        return await invoke<Game[]>('get_games_by_space', { spaceId });
      }
      return await invoke<Game[]>('get_all_games');
    },
  });
}

export function useCreateGame() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (request: CreateGameRequest) => {
      return await invoke<Game>('create_game', { request });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['games'] });
    },
  });
}

export function useDeleteGame() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (id: string) => {
      return await invoke('delete_game', { id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['games'] });
    },
  });
}

export function useLaunchGame() {
  return useMutation({
    mutationFn: async ({ gameId, spaceId }: { gameId: string; spaceId: string }) => {
      return await invoke('launch_game', { gameId, spaceId });
    },
  });
}

export function useScanDirectory() {
  return useMutation({
    mutationFn: async (path: string) => {
      return await invoke<ScannedGame[]>('scan_directory', { path });
    },
  });
}
