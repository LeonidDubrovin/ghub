# Plan: Fix archive extraction and unify metadata fetching by source URL

## Goal
- Fix the bug where downloaded itch.io archives are not extracted and the install folder ends up containing a raw `.download` file.
- Unify and fix metadata fetching so that:
  - Games with a source link get exact metadata from that page.
  - Games without a source link let the user search and pick the correct page, then get exact metadata from the chosen page.

## Constraints
- Keep existing compression dependencies (`zip`, `tar`, `flate2`) if possible.
- For metadata, prefer exact source-page parsing over any fuzzy search.
- Do not commit build artifacts (`release/`, `.kilocode/`, `.kilo/kilo.jsonc`).

## Current problems

### 1. Archives are not extracted
In `src-tauri/src/download_service.rs`:
- `download_itch_game` always downloads the file to `{title}.download`.
- `extract_archive` decides how to decompress based on the **file extension**.
- Because the extension is `.download`, the archive is treated as a non-archive and is simply copied into the target folder.

Result: install folder contains a `.download` file instead of extracted game files.

### 2. Metadata fetching is inconsistent and unreliable
There are two separate flows that currently use different query strategies:
- `create_game_from_link` (when adding a link) parses the URL into a fuzzy title/slug query and runs `metadata_aggregator.search_best`, which tries Steam first, then Itch.
- `fetch_and_update_game_metadata` (the "Fetch metadata" button) derives a query from exe metadata, game title, or directory name, then also runs `search_best`.

Both flows ignore the actual source URL stored on the game/link. For itch URLs, the fuzzy search often returns the wrong game (e.g., a Steam game with the same name or another itch game).

The exact itch/Steam page already contains reliable metadata: `og:title`, `og:image`, `og:description`, and the page title.

## Design decisions

1. **Exact source URL is the primary metadata source.** When a game has a source link with a known source type, metadata is fetched only from that exact page.
2. **No automatic fuzzy-search fallback.** If exact fetch fails, no wrong metadata is applied. It is better to have no metadata than wrong metadata.
3. **Games without a source link use manual search.** The user opens the metadata search dialog, picks a result, and the app stores that URL as the source link and then fetches exact metadata from it.
4. **The `search_game_metadata` command stays** only as a candidate finder for the manual search dialog. Selected candidates must be re-fetched by exact URL before applying.

## Plan

### 1. Archive extraction fix
- In `src-tauri/src/download_service.rs`:
  - Add a `sanitize_file_name` helper that preserves the file extension while removing unsafe characters (different from `sanitize_folder_name`).
  - Change `download_itch_game` to accept an optional `archive_filename` argument (e.g., the upload filename from the API like `TinyTowns_Windows.zip`).
  - If `archive_filename` is provided and has a recognized extension, use it as the download filename. Otherwise keep the safe fallback `{title}.download`.
  - Add a `detect_archive_format` helper that reads the file magic bytes:
    - `PK\x03\x04` → ZIP
    - `\x1f\x8b` → gzip (tar.gz)
    - `ustar` at offset 257 → tar
  - Update `extract_archive` to use the magic-detected format when the file extension is unrecognized or missing.
  - Keep existing ZIP and tar.gz extraction paths; if the format is unknown, copy the file as-is (e.g., a standalone installer).
- In `src-tauri/src/commands/downloads.rs`:
  - Pass `selectedUpload.filename` as `archive_filename` when calling `download_service::download_itch_game`.

### 2. Exact metadata fetch by source URL
- In `src-tauri/src/commands/metadata.rs`:
  - Add a helper `fetch_metadata_by_url(client, source_type, url) -> Result<Option<MetadataSearchResult>, String>`.
  - For `source_type == "itch"`: use `crate::metadata::itch::ItchStrategy::get_details(client, url)` to parse the exact page.
  - For `source_type == "steam"`: extract the Steam app ID from the URL, use `crate::metadata::steam::SteamStrategy::get_details(client, &app_id)`, and enrich the result with title/cover parsed from the Steam page.
  - For any other or missing source: return `Ok(None)`.
- In `src-tauri/src/metadata/steam.rs`:
  - Update `SteamStrategy::get_details` to also parse the page title and a cover URL from the Steam page so it can be used directly for source-URL metadata resolution.

### 3. Automatic metadata fetch (games with a source link)
- In `src-tauri/src/commands/downloads.rs`:
  - In `create_game_from_link`, when `source_type` is known (itch/steam), use `fetch_metadata_by_url` with the exact URL.
  - If it returns `None`, use the URL-derived title/slug as the fallback title only; do not call the aggregator.
- In `src-tauri/src/commands/metadata.rs`:
  - In `fetch_and_update_game_metadata`, retrieve the game's primary source link (`external_link` or the first `game_links` row with a source type) and its `source_type`.
  - If a source link exists, use `fetch_metadata_by_url`.
  - If exact metadata is found, apply it and return the updated game.
  - If no source link exists or exact fetch fails, return a distinct response indicating that the user should pick a source manually. Do not fall back to `search_best`.

### 4. Manual metadata search (games without a source link)
- In `src-tauri/src/commands/metadata.rs`:
  - Keep `search_game_metadata` as a candidate search (fuzzy) used only by the manual dialog.
- In `src/components/MetadataSearchDialog.tsx`:
  - After the user selects a result, first add the selected URL as a source link for the game (`add_game_link`).
  - Then call `fetch_metadata_by_url` with the selected URL and source type to fetch exact metadata.
  - Apply the exact metadata to the game.
  - Do not apply the search result's metadata directly anymore.
- In `src/App.tsx` / `src/components/GameDetailsView.tsx`:
  - When the "Fetch metadata" action returns the "no source link" marker, open `MetadataSearchDialog` for the user to select a source.
  - After the dialog closes with a selected source, the game should already have metadata applied by the dialog.

### 5. Validation
- Run `cargo test`.
- Run `tsc --noEmit`.
- Run `npm run build:win64`.

### 6. Rebuild
- Output: `build/win64/ghub.exe`.

## Risks
- Magic-byte detection does not handle 7z archives without adding a new dependency. If an itch upload is a 7z file, it will still be copied as-is and the game will not launch.
- `sanitize_file_name` must preserve the extension but still prevent path traversal/invalid characters.
- If the itch/Steam page is blocked or malformed, exact metadata cannot be fetched. This is intentional.
- Manual search still relies on fuzzy results as candidates, but the final applied metadata comes from the exact page of the chosen candidate.
- The response type of `fetch_and_update_game_metadata` changes (or a new command is added), which requires frontend adjustments.

## Validation after implementation
1. Download `https://handcrafted.itch.io/runeshard` or any zip-based itch game and confirm the archive is extracted into a folder with the game executable, not a `.download` file.
2. Add `https://colorbomb.itch.io/trace` and confirm the cover, title, and developer match the itch page exactly.
3. Add a game with a Steam link and confirm exact Steam metadata is fetched by URL.
4. For a game that already has an itch link, press "Fetch metadata" and confirm the metadata is fetched from the exact itch page, not from a fuzzy search.
5. For a game without a source link, press "Fetch metadata", select a result in the search dialog, and confirm the chosen link is added and the exact metadata from that page is applied.

---

# Add-on: Delete installed variant with files and re-download support

## Goal
- Allow the user to delete an installed variant together with its files.
- Keep the existing "Download variant" flow for re-downloading the same or another version.

## Context
- The app already supports multiple installed variants per game (`installs` table).
- `delete_game_install` currently deletes only the DB record and leaves files on disk.
- The "Download variant" button is shown for any game with an itch source link, so downloading another version is already supported.

## Design decisions

1. **Variant-level deletion.** Delete one installed variant and its own folder only; other variants stay untouched.
2. **Ask every time.** Show a confirmation dialog with a checkbox "Delete files from disk" checked by default.
3. **Safety check.** Only delete files if the variant's `install_path` is inside the configured `space.source_path`.
4. **Re-download via existing flow.** Do not store `upload_id`; rely on the existing "Download variant" dialog to pick the same or a different upload.

## Plan

### 1. Backend: delete variant with optional file removal
- In `src-tauri/src/commands/games.rs`:
  - Add a helper `is_install_path_safe(install_path, source_path) -> bool` that canonicalizes both paths and checks `install_path.starts_with(source_path)`.
  - Add a helper `delete_install_files(install) -> Result<(), String>` that:
    - Loads the install's space via `db.get_space_by_id(&install.space_id)`.
    - Verifies `install.install_path` is under `space.source_path`.
    - Recursively deletes the directory using `std::fs::remove_dir_all`.
  - Update `delete_game_install` command signature to accept `delete_files: bool`.
    - If `delete_files` is true, call `delete_install_files` before removing the DB record.
    - If the directory is already gone, continue and delete the DB record.
- In `src-tauri/src/lib.rs`:
  - The command is already registered; no new registration is needed because the existing `delete_game_install` signature changes.

### 2. Frontend: delete confirmation dialog
- Create `src/components/DeleteInstallDialog.tsx`:
  - Props: `install`, `onClose`, `onConfirm(deleteFiles)`.
  - Show the install path (and `version` if present).
  - Checkbox "Delete files from disk" (`checked` by default).
  - Buttons: "Cancel" and "Delete".
- In `src/components/GameDetailsView.tsx`:
  - Replace the `confirm(...)` call in `handleDeleteInstall` with a state-driven dialog.
  - Add `showDeleteDialog` state and `installToDelete` state.
  - On confirm, call `delete_game_install({ installId, deleteFiles: ... })`.
  - After success, refresh the install list via `get_game_installs`.
- In `src/App.tsx` (if needed):
  - No changes; the dialog is local to `GameDetailsView`.

### 3. Translations
- Add keys to `src/locales/en.json` and `src/locales/ru.json`:
  - `details.deleteInstallTitle`
  - `details.deleteInstallMessage`
  - `details.deleteInstallFiles`
  - `details.deleteInstallConfirm`
  - `details.deleteInstallCancel`

### 4. Validation
- Run `cargo check`.
- Run `tsc --noEmit`.
- Run `npm run build:win64`.

## Risks
- `remove_dir_all` can fail on Windows if files are read-only. If this happens, we may need to clear read-only attributes recursively.
- If the space source path was changed after installation, the safety check will refuse to delete files. The user must manually delete the folder.
- If the install path was moved outside the source path, deletion is refused. This is intentional.

## Validation after implementation
1. Install an itch game variant. Click delete on the variant, choose "Delete files from disk" — confirm the DB record is gone and the folder is removed.
2. Install another variant, click delete, uncheck "Delete files from disk" — confirm the DB record is gone but the folder remains.
3. After deleting all variants, click "Download variant" and select an upload — confirm a new install is created.
4. With two installed variants, delete one — confirm the other variant folder still exists and is playable.

---

# Add-on: Clean install folder names for itch downloads

## Goal
- Remove archive extensions and redundant game-title prefixes from install folder names.
- Use platform names from the itch API as a fallback when the upload filename/display name does not contain a useful variant.

## Context
- `download_itch_game` creates the install folder as `{game_title} - {upload_name}`.
- `upload_name` is `display_name || filename`. Filenames often contain the game title and archive extension (e.g., `TinyTowns_Windows.zip`), producing folders like `TinyTowns - TinyTowns_Windows.zip`.
- The archive filename used for the temporary download is separate and should keep its original extension.

## Design decisions

1. **Clean the variant name in the backend.** `download_itch_game` will derive the folder name from the game title, the raw upload name, and the upload platforms.
2. **Use `display_name` when available.** Otherwise use `filename`.
3. **Remove archive extensions.** Strip `.zip`, `.tar.gz`, `.tar`, `.gz`, `.tgz`, `.rar`, `.7z`.
4. **Remove the game title prefix.** Match case-insensitively while ignoring spaces, hyphens, and underscores.
5. **Trim separators.** Remove leading/trailing spaces, hyphens, underscores, and dots.
6. **Platform fallback.** If the cleaned variant is empty, use the platform names returned by the itch API, joined by spaces.
7. **Title-only fallback.** If no platforms are available, use only the game title.

## Plan

### 1. Backend: clean variant name helpers
- In `src-tauri/src/download_service.rs`:
  - Add `remove_archive_extension(name: &str) -> &str` that strips known archive extensions.
  - Add `strip_title_prefix(title: &str, raw: &str) -> String` that removes the game title from the start of the raw name (case-insensitive, ignoring separators).
  - Add `platform_label(platform: &str) -> String` that maps `windows`/`linux`/`osx`/`android` to `Windows`/`Linux`/`macOS`/`Android`.
  - Add `clean_variant_name(title: &str, raw: &str, platforms: Option<&[String]>) -> Option<String>` that:
    - Removes the archive extension.
    - Strips the title prefix.
    - Trims separators.
    - Returns the cleaned variant if non-empty.
    - Falls back to joined platform labels if cleaned variant is empty and platforms are provided.
  - Update `download_itch_game`:
    - Add parameter `upload_platforms: Option<Vec<String>>`.
    - Use `clean_variant_name(title, variant_name.unwrap_or(""), upload_platforms.as_deref())` to build the final folder-name suffix.
    - Keep the final folder name as `{title} - {clean_variant}` when a clean variant exists, otherwise `{title}`.
    - Keep the `archive_filename` parameter unchanged for the temporary download file.

### 2. Backend command: pass platforms to download service
- In `src-tauri/src/commands/downloads.rs`:
  - Update `download_game_link` to accept `upload_platforms: Option<Vec<String>>`.
  - Pass `upload_platforms` to `download_service::download_itch_game`.

### 3. Frontend: send selected platforms
- In `src/components/GameDetailsView.tsx`:
  - In the `download_game_link` invoke call, add:
    ```typescript
    uploadPlatforms: selectedUpload.platforms
      ? Object.entries(selectedUpload.platforms)
          .filter(([, enabled]) => enabled)
          .map(([platform]) => platform)
      : null,
    ```

### 4. Validation
- Run `cargo check`.
- Run `tsc --noEmit`.
- Run `npm run build:win64`.

## Risks
- If the upload filename contains the game title in a different form than the stored game title (e.g., `Tiny-Towns` vs `TinyTowns`), the prefix may not be removed and the folder will still contain redundancy.
- If the cleaned variant is empty and platforms are missing, multiple variants of the same game will get names like `TinyTowns`, `TinyTowns-1`, etc.
- Some itch uploads may use non-standard archive extensions; the list must be maintained.

## Validation after implementation
1. Download a game with upload filename `TinyTowns_Windows.zip` — confirm the install folder is `TinyTowns - Windows`.
2. Download a game with `display_name` "Linux" — confirm the folder is `{title} - Linux`.
3. Download a game with filename `TinyTowns.zip` and platforms `{windows: true}` — confirm the folder is `{title} - Windows`.
4. Download a game with filename `TinyTowns.zip` and platforms `{windows: true, linux: true}` — confirm the folder is `{title} - Windows Linux`.
5. Confirm the temporary archive file still has its original filename and is removed after extraction.

---

# Add-on: Fix "No executable path set" after itch download

## Goal
- Fix the bug where a downloaded itch install that contains an `.exe` at its root is recorded with no executable path.
- Verify whether the generated folder name for "Last Minute Escape" is still weird after the fix and correct it if needed.

## Root cause
- `download_service::process_downloaded_archive` calls `scan_directory_internal(target_dir)` after moving the extracted game files into the install folder.
- `scan_directory_internal` -> `scanner::scan_directory` treats the passed directory as a **library/source container** and skips the base directory itself, only looking in subdirectories.
- After extraction, the game files (including the `.exe`) live directly in `target_dir`, not in a subdirectory. So the scanner returns no games, `executable_path` becomes `None`, and launching fails with "No executable path set".

## Design decisions
1. For single install directories, use a direct executable scan that includes the base directory.
2. Add logging to the download flow so we can diagnose the folder name if it still looks wrong after the fix.
3. Keep the existing container scan for source/library directories.

## Plan

### 1. Backend: add single-directory executable scan
- In `src-tauri/src/scanner.rs`:
  - Add a public function `find_executable_in_directory(dir: &Path) -> Option<String>`:
    - Create a `ScanConfig::default()`.
    - Call `find_all_executables(dir, &config)` (the existing helper that scans the directory and its descendants).
    - Call `pick_best_executable(dir, &executables)` (the existing helper with priority rules).
    - Return the best executable path relative to `dir`, or `None`.
- This function does **not** skip the base directory; it scans `dir` itself and its descendants up to `max_exe_search_depth` (currently 4).

### 2. Backend: use the new scan for downloads
- In `src-tauri/src/download_service.rs`:
  - In `process_downloaded_archive`, replace:
    ```rust
    let scanned = crate::commands::scan_directory_internal(target_dir)
        .map_err(|e| format!("Failed to scan installed directory: {}", e))?;
    let executable_path = scanned.into_iter().next().and_then(|g| g.executable);
    ```
    with:
    ```rust
    let executable_path = crate::scanner::find_executable_in_directory(target_dir);
    ```
  - Add logging around the folder-name logic in `download_itch_game`:
    - `info!("Download folder: title='{}' raw_variant='{}' clean_variant='{:?}' platforms='{:?}' final_folder='{}'", ...)`.
  - Add logging in `process_downloaded_archive`:
    - `info!("Scanned install directory '{}' for executable: {:?}", target_dir.display(), executable_path)`.

### 3. Folder name verification
- After the executable fix is built, ask the user to re-download the same game.
- If the folder is still named `Last Minute Escape - Minute Escape`, inspect the new log entries to see:
  - The game title from the database.
  - The raw `variant_name` (upload `display_name` or `filename`).
  - The cleaned variant produced by `clean_variant_name`.
  - The platforms from the itch API.
- Based on the logs, decide whether the cleaning logic needs adjustment. Possible adjustments:
  - If the cleaned variant is a suffix/substring of the title (e.g., title is "Last Minute Escape" and variant is "Minute Escape"), consider it non-useful and fall back to platforms or title-only.
  - If the raw upload name is already a clean platform label, keep it.

### 4. Validation
- Run `cargo check`.
- Run `cargo test`.
- Run `npm run build:win64`.
- Manual test: re-download the same itch game and confirm:
  - The install folder contains the `.exe`.
  - The database `installs.executable_path` is set.
  - The game launches without "No executable path set".

## Risks
- If the install directory has multiple `.exe` files (e.g., installer + game), `pick_best_executable` may still pick the wrong one. Existing priority rules apply.
- If the `.exe` is deeper than `max_exe_search_depth` (4), it still won't be found. This is unlikely for simple itch games.
- Web-only games (HTML/JS) will continue to have no executable. Handling browser games is out of scope for this fix.
- `refresh_game_from_local` uses the same `scan_directory_internal` on an install path and likely has the same bug. It should be fixed in the same way if the user reports issues with "Refresh from local".

---

# Add-on: Unify refresh and search into a single "Update metadata" dialog

## Goal
- Replace the two separate actions/buttons for metadata ("Fetch metadata" and "Refresh from local") with a single **"Update metadata"** entry point.
- Open a dialog where the user always sees **exactly one clearly labeled action per section**, with a preview before applying.
- Make sure every path actually works and is easy to diagnose.

## Context
- The previous unified dialog used tabs for "Local files" and "Internet" and mixed several actions in one tab, which confused users and appeared to do nothing.
- `MetadataSearchDialog` is still used by `BatchMetadataDialog` and `EditGameDialog` for pure search, so it must stay unchanged.

## Design decisions (final)
1. **Single entry point.** One "Update metadata" button in `GameDetailsView`, one context-menu item, and one batch item.
2. **Separate dedicated dialog.** A new `MetadataUpdateDialog` component is used for the update entry point; `MetadataSearchDialog` remains search-only for batch/edit.
3. **Sidebar with three distinct sections.** The dialog is split into three independent cards that never mix their state:
   - **Local files** — scan the install directory, show a current vs. found comparison, and apply when the user confirms.
   - **Current source link** — if the game already has a Steam/itch link, fetch exact metadata from that page and apply it.
   - **Find / add link** — paste a new URL (exact fetch) or search by title, pick a result, and apply.
4. **One primary action per section.** Each section has a single button that does exactly what its label says ("Сканировать", "Загрузить по ссылке", "Загрузить и показать", "Применить выбранный результат").
5. **Preview before apply.** For every non-local source the user first sees the fetched metadata, can toggle which fields to update, and only then applies.
6. **Default section.** Open the section that is most useful for the game:
   - "Local files" if the game has an install.
   - "Current source link" if there is an existing Steam/itch link but no install.
   - "Find / add link" if there is neither.
7. **Preserve existing metadata.** Local refresh only fills missing fields and updates the executable path.
8. **Authenticated itch search.** `search_game_metadata` uses the stored API key for `https://itch.io/api/1/{key}/search/games`; falls back to public search if no key or the call fails.
9. **Backend logging.** Every metadata command now logs the inputs and result count so we can diagnose "nothing happens" in real builds.

## Implemented changes

### 1. Backend: logging and local scan fixes
- In `src-tauri/src/commands/metadata.rs`:
  - Added `info!` logs to `search_game_metadata`, `fetch_metadata_by_url_command`, `scan_local_metadata`, and `refresh_game_from_local`.
  - `scan_local_metadata` now returns absolute paths for `executable` and `cover_candidates` so the preview can display them.
  - `refresh_game_from_local` logs which fields are updated and the executable path.

### 2. Frontend: new `MetadataUpdateDialog`
- Created `src/components/MetadataUpdateDialog.tsx`:
  - Left sidebar with three big buttons: Local files, Current source link, Find / add link.
  - Each section is a self-contained card with its own state, action button, error area, and preview.
  - Local section loads the install path and a current-vs-found comparison.
  - Current-link section auto-fetches the existing link and lets the user refetch/apply.
  - Find section has two subsections: URL input (exact fetch) and title search (result list + preview + field toggles).
  - Uses `fetch_metadata_by_url_command` for exact source-page metadata and `search_game_metadata` for title search.
  - Applying adds/updates the source link and calls `update_game` with the selected fields.

### 3. Frontend: wire the new dialog
- In `src/components/GameDetailsView.tsx`:
  - Replaced the `MetadataSearchDialog` import with `MetadataUpdateDialog` for the update dialog.
  - The single "Update metadata" button opens `MetadataUpdateDialog`.
- In `src/App.tsx`:
  - Replaced the context-menu `MetadataSearchDialog` with `MetadataUpdateDialog`.

### 4. Translations
- In `src/locales/ru.json` and `src/locales/en.json`:
  - Added the full `metadataUpdate` namespace with clear labels for every section and button.
  - Added `metadataSearch.noDescription`.

### 5. Validation
- `npm run build` (tsc + vite) passed.
- `npm run build:win64` passed.
- `cargo test` still has 16 pre-existing failures unrelated to this change.

## Risks
- The `ItchApiClient::search_games` endpoint is undocumented and may change or be rate-limited.
- `scan_single_directory` may pick the wrong subfolder if the install contains multiple nested executables.
- The local section looks at the first install only; multiple variants are not handled in this iteration.
- Exact page fetching still depends on the itch/Steam page being reachable and parseable.

## Follow-up fixes
- Fixed dialog opening flicker/jitter caused by the initial-load effect depending on the scan/fetch callbacks, which themselves depended on state set by the effect. The callbacks are now accessed through stable refs and the effect only depends on `isOpen`, `game.id`, and `game.title`. An `isInitializing` loading screen is shown while the initial section is chosen, preventing a content flash.

---

# Add-on: Refresh UI after download completion

## Goal
- After a game finishes downloading, immediately switch the view to the target source/space, show the game there, and update the game card so the button is no longer "Download".
- Refresh the sidebar/source counts so the new location is visible.

## Context
- `download_game_link` returns a `DownloadGameLinkResponse` with the updated `Game`, but `GameDetailsView` ignored the returned game and only refetched `installs`.
- `GameDetailsView` did not refetch `gameLinks`, so `activeLink` still had the old `download_status` and `queue_space`, leaving the button as "Download".
- `App.onSave` only called `refetchGames()` for the current space. When the game moved from `incoming` to a target space, it left the current list, but the selected detail card was not cleared/updated.
- React Query caches for `games`, `space_sources`, and `spaces` were not invalidated, so the sidebar did not reflect the move.

## Design decisions
1. Use the returned game from `download_game_link` to immediately update the selected game state.
2. Switch the view to the target space and source where the game was installed.
3. Keep the downloaded game selected so the game list scrolls to it and highlights it.
4. Invalidate the relevant React Query caches so the sidebar and game lists refresh.
5. Refetch `gameLinks` in `GameDetailsView` after download so the button logic is updated.

## Implemented changes

### 1. Frontend: `GameDetailsView.tsx`
- Added `onGameDownloaded?: (game: Game, spaceId: string, sourcePath: string) => void` prop.
- Captured the `DownloadGameLinkResponse` from `download_game_link`.
- After success, refetched `installs` and `gameLinks` for the selected game.
- If the status is `downloaded`, called `onGameDownloaded(response.game, spaceId, sourcePath)` instead of the generic `onSave`.
- Kept `onSave` for browser-status downloads or errors.

### 2. Frontend: `App.tsx`
- Imported `useQueryClient` from `@tanstack/react-query`.
- Added `handleGameDownloaded`:
  - Sets `selectedSpaceId` and `selectedSource` to the target.
  - Sets `selectedGameForDetails` to the updated game object.
  - Clears bulk selection state.
  - Invalidates `['games']`, `['space_sources']`, and `['spaces']` queries.
- Passed `onGameDownloaded={handleGameDownloaded}` to `GameDetailsView`.

### 3. Validation
- `npm run build` passed.
- `npm run build:win64` passed (after stopping the running old binary to unlock the output file).
- Updated `build/win64/ghub.exe`.

## Risks
- If the target space/source query is slow, the game list may briefly show the old/new state before the new game appears. The selected game is set immediately, but the highlight depends on the new list loading.
- The returned `game` object from `get_game_by_id` reflects the first install. If multiple installs exist, the displayed source might not match the downloaded variant.
