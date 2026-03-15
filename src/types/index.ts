export interface Space {
  id: string;
  name: string;
  path: string | null;
  type: 'local' | 'steam' | 'itch' | 'virtual';
  icon: string | null;
  color: string | null;
  sort_order: number;
  is_active: boolean;
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
}

export interface Install {
  id: string;
  game_id: string;
  space_id: string;
  install_path: string;
  executable_path: string | null;
  launch_arguments: string | null;
  working_directory: string | null;
  status: 'installed' | 'installing' | 'broken';
  version: string | null;
  install_size_bytes: number | null;
  installed_at: string;
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
  fetch_metadata?: boolean;
}

export interface CreateGameLinkRequest {
  url: string;
  title?: string;
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

export interface DownloadLink {
  id: string;
  url: string;
  title: string;
  cover_url: string | null;
  description: string | null;
  status: string;
  added_at: string;
}

export interface MetadataSearchResult {
  id: string;
  name: string;
  cover_url?: string;
  release_date?: string;
  summary?: string;
  developer?: string;
  publisher?: string;
  source?: string;
  url?: string;
  tags?: string[];
  genres?: string[];
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