export type SortField = 'title' | 'last_played' | 'playtime' | 'added_at' | 'developer';
export type SortOrder = 'asc' | 'desc';

export interface Space {
  id: string;
  name: string;
  path: string | null;
  type: 'local' | 'steam' | 'itch' | 'virtual';
  icon: string | null;
  color: string | null;
  sort_order: number;
  is_active: boolean;
  is_system: boolean;
  created_at: string;
  updated_at: string;
  watch_directories?: SpaceSource[];
}

export interface SpaceSource {
  space_id: string;
  source_path: string;
  is_active: boolean;
  scan_recursively: boolean;
  last_scanned_at?: string;
  exclude_patterns?: string[];
  // Scan status - always set explicitly, never undefined
  scan_status: 'idle' | 'scanning' | 'completed' | 'error';
  scan_progress?: number;
  scan_total?: number;
  scan_error?: string;
  scan_started_at?: string;
  scan_completed_at?: string;
}

export interface Game {
  id: string;
  title: string;
  sort_title: string | null;
  description: string | null;
  release_date: string | null;
  developer: string | null;
  publisher: string | null;
  cover_image: string | null;
  background_image: string | null;
  total_playtime_seconds: number;
  last_played_at: string | null;
  times_launched: number;
  is_favorite: boolean;
  is_hidden: boolean;
  completion_status: 'not_played' | 'playing' | 'completed' | 'abandoned' | 'on_hold';
  user_rating: number | null;
  added_at: string;
  updated_at: string;
  external_link?: string;
  // Optional fields for UI display (populated when joining with installs/spaces)
  space_id?: string;
  space_name?: string;
  space_type?: string;
  install_path?: string;
  executable_path?: string;
}

export interface Install {
  id: string;
  game_id: string;
  space_id: string;
  install_path: string;
  executable_path: string | null;
  launch_arguments: string | null;
  working_directory: string | null;
  status: 'installed' | 'missing' | 'modified' | 'installing' | 'broken';
  version: string | null;
  install_size_bytes: number | null;
  installed_at: string;
  fingerprint?: string;
}

export interface ScannedGame {
  path: string;
  title: string;
  executable: string | null;
  all_executables: string[];
  size_bytes: number;
  icon_path: string | null;
  cover_candidates: string[];
  exe_metadata: ExeMetadata | null;
}

export interface ExeMetadata {
  product_name: string | null;
  company_name: string | null;
  file_description: string | null;
  file_version: string | null;
}

export interface Setting {
  key: string;
  value: string;
}

export interface CreateSpaceRequest {
  name: string;
  type: string;
  icon?: string;
  color?: string;
  initial_sources?: string[];
}

export interface CreateGameRequest {
  title: string;
  space_id: string;
  install_path: string;
  executable_path?: string;
  description?: string;
  developer?: string;
  cover_image?: string;
}

export interface UpdateGameRequest {
  id: string;
  title?: string;
  description?: string | null;
  developer?: string | null;
  publisher?: string | null;
  cover_image?: string | null;
  is_favorite?: boolean;
  completion_status?: string;
  user_rating?: number | null;
}

export interface MetadataSearchResult {
  id: string;
  name: string;
  cover_url: string | null;
  release_date: string | null;
  developer: string | null;
  publisher: string | null;
  description: string | null;
  rating: number | null;
  source: string;
  url: string | null;
  tags: string[] | null;
  genres: string[] | null;
}

export interface AddSpaceSourceRequest {
  space_id: string;
  source_path: string;
  scan_recursively?: boolean;
}

export interface UpdateSpaceSourceRequest {
  space_id: string;
  source_path: string;
  is_active: boolean;
  scan_recursively?: boolean;
}

export interface SpaceWithSources {
  space: Space;
  sources: SpaceSource[];
}

export type DownloadStatus = 'pending' | 'external' | 'browser' | 'downloaded' | 'error';

export interface GameLink {
  id: string;
  game_id: string;
  url: string;
  canonical_url?: string;
  title: string | null;
  source_type: string | null;
  download_status: DownloadStatus | null;
  queue_space: 'incoming' | 'online' | null;
  created_at: string;
}

export interface CreateGameFromLinkRequest {
  url: string;
}

export interface CreateGameFromLinkResponse {
  game: Game;
  is_duplicate: boolean;
  existing_link?: GameLink;
}

export interface AddGameLinkResponse {
  link: GameLink;
  is_duplicate: boolean;
  existing_game?: Game;
}

export interface DownloadGameLinkRequest {
  game_id: string;
  link_id: string;
  upload_id: number;
  upload_name: string;
  space_id: string;
  source_path: string;
}

export interface DownloadGameLinkResponse {
  game: Game;
  status: 'downloaded' | 'browser';
}

export interface ItchUpload {
  id: number;
  filename: string;
  display_name: string | null;
  size: number;
  created_at: string | null;
  platforms: {
    windows?: boolean;
    linux?: boolean;
    osx?: boolean;
    android?: boolean;
  } | null;
}

export interface MoveGameLinkRequest {
  link_id: string;
  queue_space: 'incoming' | 'online' | null;
}

export interface OpenGameLinkRequest {
  url: string;
  source_type?: string;
}

export interface SelectedSource {
  spaceId: string;
  sourcePath: string;
}