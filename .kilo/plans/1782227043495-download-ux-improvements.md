# Plan: Download UX improvements

> **Implementation status:** implemented and rebuilt.
> - `npm run build` passed.
> - `npm run build:win64` passed and updated `build/win64/ghub.exe`.

## Goal
- Give immediate visual feedback when the user clicks **Download** (currently there is a multi-second delay before anything happens while the upload list is fetched).
- Keep download error and loading indicators tied to the correct game card; switching to another card should not show another game's error/loading state.
- Show the upload date for each itch.io upload in the file selection dialog, so users can pick the newest version.

## Context from code inspection
- `src/components/GameDetailsView.tsx:275-298` (`handleStartDownload`) only resets the error and then performs network calls (`get_itch_api_key`, `get_itch_game_uploads`) before opening `SelectUploadDialog`. No loading state is set during this phase, so the button appears idle.
- `isDownloading`, `downloadError`, `downloadProgress`, `downloadSpeed`, `downloadingLinkId` are top-level `useState` values in `GameDetailsView.tsx`. They are not scoped to a game/link, so when the user switches cards the same state is rendered for the newly selected game.
- `src/components/SelectUploadDialog.tsx` displays `display_name`, `size`, and `platforms`, but there is no date column.
- `ItchUpload` in `src/types/index.ts` and `ItchApiUpload` in `src-tauri/src/itch_api.rs` do not include a `created_at` field, although the itch.io `/games/{id}/uploads` endpoint returns it.

## Design decisions
1. **Immediate feedback:** open `SelectUploadDialog` immediately with a loading spinner, and also put the **Download** button into a loading state while the upload list is being resolved. Two visible cues remove the impression that nothing is happening.
2. **Scoped download/error state:** keep the actual state global (because progress is delivered by backend events), but only render the spinner, progress bar, and error message when the current game's active link matches the tracked link ID (`activeLink?.id === downloadingLinkId`, `activeLink?.id === downloadErrorLinkId`). This keeps the state visible when the user returns to the same game, but never shows it on the wrong card.
3. **Upload date:** add `created_at` to the backend and frontend upload types, and display it as an absolute localized date in `SelectUploadDialog` (fallback to empty if the API does not provide it).
4. **Close dialogs on game switch:** reset `showUploadDialog` and `showTargetDialog` when `selectedGame?.id` changes, so the user never sees an upload/target dialog for the wrong game.

## Implementation steps

### 1. Backend: expose upload creation date
- In `src-tauri/src/itch_api.rs`:
  - Add `#[serde(default)] pub created_at: Option<String>` to `ItchApiUpload`.
- In `src-tauri/src/commands/itch_api.rs` (where `get_itch_game_uploads` returns `Vec<ItchUpload>`), pass the field through unchanged.

### 2. Frontend types: add `created_at` to `ItchUpload`
- In `src/types/index.ts`:
  - Add `created_at: string | null;` to `ItchUpload`.

### 3. Frontend: `SelectUploadDialog` loading and date display
- In `src/components/SelectUploadDialog.tsx`:
  - Add `isLoading?: boolean` prop and show a centered spinner / "Loading uploads…" message when true.
  - Display `created_at` as an absolute localized date (e.g., `new Date(created_at).toLocaleDateString()`) next to the size or filename. Show nothing if `created_at` is null/invalid.

### 4. Frontend: `GameDetailsView` immediate feedback and scoped state
- In `src/components/GameDetailsView.tsx`:
  - Add state:
    - `isFetchingUploads: boolean` (or `isFetchingUploadsForGameId: string | null`).
    - `downloadErrorLinkId: string | null` (tracks which link the current error belongs to).
  - In `handleStartDownload`:
    - Set `isFetchingUploads(true)` and `downloadError(null)` / `downloadErrorLinkId(null)` immediately.
    - Open `setShowUploadDialog(true)` immediately (with `uploads` empty).
    - Fetch API key and uploads as before; after `setUploads`, set `isFetchingUploads(false)`.
  - In `handleDownloadTarget`:
    - Set `downloadError(null)` and `downloadErrorLinkId(null)` at the start.
    - On error, set `downloadError(err)` and `downloadErrorLinkId(activeLink?.id ?? null)`.
  - In the render logic where error/loading UI is shown:
    - Show the spinner / progress bar only if `activeLink?.id === downloadingLinkId`.
    - Show the error message only if `activeLink?.id === downloadErrorLinkId`.
    - While `isFetchingUploads`, show the button as loading/disabled (label can remain "Download" or become "Loading…").
  - Add a `useEffect` that resets `showUploadDialog`, `showTargetDialog`, `isFetchingUploads`, and the selected upload when `selectedGame?.id` changes.

### 5. Frontend: `App` wiring (no changes needed if callback shape is unchanged)
- `onGameDownloaded` already switches space and refreshes the game; no new wiring is required for this plan.

## Validation
- `npm run build` (tsc + vite) must pass.
- `npm run build:win64` must pass and produce `build/win64/ghub.exe`.
- Manual checks:
  1. Click **Download** on an itch game in `incoming` — a dialog should appear immediately with a spinner, then the upload list.
  2. Trigger a download error (e.g., disconnect the network briefly) and switch to another game card — the error message should disappear; switching back should show it again.
  3. Start a download and switch to another card — the progress bar should not appear on the other card; returning to the original card should show the progress again.
  4. The upload list should show the date for each upload if the API returns it.

## Risks
- If the itch.io API does not return `created_at` for a particular upload, the date column will be empty. This is acceptable because the field is optional.
- If the user starts multiple parallel downloads, a single `downloadProgress` object will not track each download separately. The current UI strongly implies one download at a time, so this is acceptable; if parallel downloads are later supported, switch to a `Map<linkId, progress>`.
- Closing the upload/target dialog on every game switch means a user who clicks Download, opens the dialog, then clicks another game by accident will lose the dialog. This is the intended behavior — the dialog belongs to the selected game.
