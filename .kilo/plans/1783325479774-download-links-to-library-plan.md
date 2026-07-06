# Plan: Download links → game cards with installation

**Goal:** turn external links (Steam/itch) into real game cards in the library, with the ability to download itch games into a chosen space/source. Steam links open the store; browser-only itch games stay in a "Downloads" list until moved or opened.

**Target file:** `E:\dev\GameLauncher\ghub\.kilo\plans\1783325479774-download-links-to-library-plan.md`

---

## Context

- The app currently has a `download_links` table used only by the "Links" view (`DownloadLinksView`).
- It also has a `game_links` table attached to a specific game.
- Games live in `games`; their disk presence is tracked by `installs`.
- A `Game` can exist without an `Install` (e.g. `create_game_link`).
- The database is created at `app_data_dir/ghub.db`. On Windows with identifier `com.ghub.app` this is `%APPDATA%\com.ghub.app\ghub.db`.

## Decisions

1. **Create a game card immediately when a link is added.** Do not keep an isolated `download_links` queue.
2. **Use `game_links` as the single source of truth for external links.** Add a `download_status` column to `game_links` (nullable) to track pending/downloaded/error/browser states.
3. **Drop the `download_links` table.** Migrate its rows into `games` + `game_links` on first startup after the change.
4. **"Downloads" / "Wishlist" is a virtual UI filter**, not a database space. It shows games that have a `game_link` with `download_status` != `downloaded` and no real `Install`.
5. **Unified metadata:** all metadata fetching uses the same backend path (`search_game_metadata` / `MetadataAggregator`).
6. **Steam:** parse `appid` from `store.steampowered.com/app/<id>` and open `steam://store/<appid>` via the shell. The card stays in Downloads with `download_status` = `external`.
7. **Itch:** implement a lightweight downloader:
   - Parse the itch page and look for a direct file download link.
   - If found, download and extract into a subfolder of the chosen source.
   - If not found (browser-only / paid / no direct link), set `download_status` = `browser` and open the page in the default browser.
8. **Target space/source:** user selects one when clicking Download. The app remembers the last used target.
9. **Folder placement:** files go into the chosen source directory under a sanitized game-title subfolder. If the folder already exists, append a numeric suffix.
10. **After successful download:** create an `Install` in the chosen source, change `download_status` to `downloaded` (or null), and the game leaves the Downloads filter.
11. **Error handling:** failed downloads keep `download_status` = `error` and remain in Downloads.
12. **Other sources (GOG, Epic, etc.):** only create a game card with a link; no downloader.
13. **Future:** add an "Online Games" space where browser-only itch games can be moved manually.

## Data flow

### Adding a link
1. Frontend: `AddLinkDialog` accepts one or more URLs.
2. Backend: for each URL:
   - Determine source (steam / itch / other).
   - Extract a search query (game title or slug).
   - Call `search_game_metadata` (or `MetadataAggregator`) to get the best match.
   - Create a `Game` with title, description, cover, developer.
   - Create a `game_link` with `url`, `source_type`, `title`, and `download_status`:
     - `pending` for itch and others.
     - `external` for Steam (cannot be downloaded).
     - `browser` later if a direct download link cannot be found.
3. Frontend: refresh the Downloads list.

### Downloading an itch game
1. User clicks Download on a card in the Downloads view.
2. Frontend opens a target selection dialog (space + source).
3. Backend command `download_game_link(game_id, target_space_id, target_source_path)`:
   - Parse the itch page and resolve the download URL.
   - If no direct URL, return `browser` and frontend opens the page.
   - Download file to a temp location.
   - Extract archive into `<target_source>/<sanitized_title>` (or `<sanitized_title>-N` if collision).
   - Run a light scan to find the executable.
   - Create an `Install` in `target_space_id` with the extracted path.
   - Set `game_link.download_status = 'downloaded'`.
4. Frontend: refresh games and remove the card from Downloads view.

### Steam / external links
1. User clicks Open Store.
2. Backend parses `appid` and returns `steam://store/<appid>`.
3. Frontend opens it via `tauri-plugin-shell`.
4. `download_status` remains `external`.

### Browser-only itch games
1. Downloader returns `browser` status.
2. Game stays in Downloads with `download_status = 'browser'`.
3. User can later open it in browser or move it to a future "Online Games" space.

## Implementation steps

1. **Database migration**
   - Add `download_status` column to `game_links`.
   - Read all existing `download_links` rows.
   - For each row create a `Game` + `game_link` with `download_status = 'pending'`.
   - Drop `download_links` table.
   - Update `Database::new` to run the migration once and mark it complete.

2. **Backend models & queries**
   - Update `GameLink` model and `game_links` schema helpers.
   - Add functions to get games with pending links (`get_download_games`).
   - Update `add_game_link` to accept `download_status`.

3. **Backend metadata & link creation**
   - Create `create_game_from_link` command that parses URL, fetches metadata, and creates a game + link.
   - Replace the old `create_download_link` usage; remove or deprecate it.

4. **Backend downloader (itch)**
   - Add URL parsing utilities for Steam and itch.
   - Add Itch page scraper to find direct download URL.
   - Add file download + extraction worker.
   - Add `download_game_link` command with progress/status updates.

5. **Backend shell commands**
   - Add `open_game_link` command for Steam store / external pages.

6. **Frontend types**
   - Add `DownloadStatus` enum.
   - Update `GameLink` interface.
   - Remove `DownloadLink` usage where possible.

7. **Frontend UI**
   - Rewrite `AddLinkDialog` to use `create_game_from_link`.
   - Rewrite `DownloadLinksView` to show pending games.
   - Add a target selection dialog for downloads.
   - Update `App.tsx` so the "Links" view shows the Downloads filter.
   - Add a "Downloads" entry in the sidebar/filter if needed.

8. **Translations**
   - Add keys for statuses, download actions, target selection, errors.

9. **Build & test**
   - Run `npm run build` and `cargo check`.
   - Test with existing `download_links` to verify migration.
   - Test adding a Steam link and opening it.
   - Test adding an itch link (free, downloadable) and downloading it.
   - Test adding a browser-only itch link.

## Migration details

- The migration runs in `Database::new` before the app is fully initialized.
- It checks for the existence of `download_links`. If present, it migrates and drops the table.
- Backups are recommended before first launch after the update. The app already has `backup_database` command.

## Failure modes & risks

- Itch page scraping is fragile; fallback to browser must always work.
- Even free itch games may require a cookie or session for direct download; handle gracefully.
- Folder collisions: must append suffix, never overwrite.
- Disk space: large downloads may fail; keep status as `error`.
- Steam URL parsing: URLs may use `/app/<id>/name` or other forms; normalize.
- Duplicate game cards: if a user later scans the same folder, we may create duplicates. Existing deduplication logic (if any) should be checked.

## Validation

- After update, existing `download_links` rows appear as game cards in the Downloads view.
- Adding a Steam link opens `steam://store/<appid>`.
- Adding an itch link creates a card with metadata.
- Downloading an itch game creates a folder in the chosen source and the game appears in that space.
- Browser-only itch games stay in Downloads with a browser status.
- `npm run build` and `cargo check` pass.

## Future work (out of scope for this plan)

- Add an "Online Games" space and let users move browser-only itch games there.
- Add progress UI for active downloads.
- Add itch API key support for purchased games.
- Support GOG/Epic download where possible.
