import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import type { Space, SpaceSource, CreateSpaceRequest, AddSpaceSourceRequest, UpdateSpaceSourceRequest, ScannedGame } from '../types';

export function useSpaces() {
  return useQuery({
    queryKey: ['spaces'],
    queryFn: async () => {
      return await invoke<Space[]>('get_all_spaces');
    },
  });
}

export function useCreateSpace() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (request: CreateSpaceRequest) => {
      return await invoke<Space>('create_space', { request });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['spaces'] });
    },
  });
}

export function useDeleteSpace() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (id: string) => {
      return await invoke('delete_space', { id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['spaces'] });
    },
  });
}

// ============ SPACE SOURCES HOOKS ============

export function useSpaceSources(spaceId: string) {
  return useQuery({
    queryKey: ['space_sources', spaceId],
    queryFn: async () => {
      return await invoke<SpaceSource[]>('get_space_sources', { spaceId });
    },
    enabled: !!spaceId,
  });
}

export function useAddSpaceSource() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (request: AddSpaceSourceRequest) => {
      return await invoke('add_space_source', {
        spaceId: request.space_id,
        sourcePath: request.source_path,
        scanRecursively: request.scan_recursively ?? true,
      });
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ['space_sources', variables.space_id] });
      queryClient.invalidateQueries({ queryKey: ['spaces'] });
    },
  });
}

export function useRemoveSpaceSource() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async ({ space_id, source_path }: { space_id: string; source_path: string }) => {
      return await invoke('remove_space_source', { space_id, source_path });
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ['space_sources', variables.space_id] });
      queryClient.invalidateQueries({ queryKey: ['spaces'] });
    },
  });
}

export function useUpdateSpaceSource() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: async (request: UpdateSpaceSourceRequest) => {
      return await invoke('update_space_source', {
        spaceId: request.space_id,
        sourcePath: request.source_path,
        isActive: request.is_active,
        scanRecursively: request.scan_recursively,
      });
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ['space_sources', variables.space_id] });
    },
  });
}

export function useScanSpaceSources() {
  return useMutation({
    mutationFn: async (spaceId: string) => {
      return await invoke<ScannedGame[]>('scan_space_sources', { spaceId });
    },
  });
}