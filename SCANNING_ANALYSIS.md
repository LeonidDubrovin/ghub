# Scanning Functionality Analysis

## Overview
This document provides a comprehensive analysis of the directory scanning feature in the GameLauncher application. It covers the UI components, data flow, state management, and identifies current issues that need to be addressed.

## Table of Contents
1. [Architecture Overview](#architecture-overview)
2. [Key Components](#key-components)
3. [UI Flows](#ui-flows)
4. [Data Flow](#data-flow)
5. [State Management](#state-management)
6. [Backend Integration](#backend-integration)
7. [Current Issues](#current-issues)
8. [Recommendations](#recommendations)

---

## Architecture Overview

The scanning system consists of three main scanning operations:

1. **Scan a specific source** - Scan a single directory that is already added as a source to a space
2. **Scan all space sources** - Scan all active sources belonging to a space
3. **Custom folder scan** - Scan an arbitrary folder not yet added as a source, with option to add games to a space

These operations are triggered from different UI locations and use different backend commands.

---

## Key Components

### Frontend Components

#### 1. SourceItem (`src/components/SourceItem.tsx`)
- Displays an individual source (directory) in the sidebar
- Shows folder name, full path, and scan status
- **Current**: No scan/cancel buttons (removed to avoid duplication)
- Status indicators:
  - Colored dot: yellow (scanning), green (completed/idle), red (error), gray (inactive)
  - Progress bar and count when scanning
  - Error message when scan fails

#### 2. SelectedSourceToolbar (`src/components/SelectedSourceToolbar.tsx`)
- Appears as a fixed bar below the header when a source is selected
- Shows:
  - Selected source name and path
  - Status message (Ready to scan, Scanning..., Completed, Error)
  - Progress bar during scan
  - Error message if scan failed
- Action buttons:
  - **Scan** (when idle) - starts scanning the selected source
  - **Cancel** (when scanning) - cancels the ongoing scan
  - **Remove** - removes the source from the space (always available, disabled during scan)
- Uses `useSourceScanStatus` to poll scan status every 2s when scanning
- Cancel uses `mutateAsync` with error handling

#### 3. Header (`src/components/Header.tsx`)
- Contains global actions
- **Scan all button**: Shows "Сканировать все" when a space is selected (no source). Triggers `scanSpaceSources` for the entire space.
- **Smart scan button**: 
  - If source selected: shows "Сканировать", immediately starts scanning that source
  - If no source selected: shows "Сканировать папку", opens ScanDialog
- Also contains search, view mode toggles, selection mode toggle

#### 4. ScanDialog (`src/components/ScanDialog.tsx`)
- Modal dialog for scanning custom folders
- Simplified to single mode (no tabs)
- Features:
  - Folder path input (text field)
  - "Select Folder" button opens directory picker
  - "Scan" button triggers scan
  - Shows loading spinner during scan
  - Displays found games with checkboxes, editable titles, exe selection, cover selection
  - Target space selector
  - "Fetch metadata" checkbox
  - "Add selected" button to add games to space
- Does NOT support scanning space sources anymore (that's moved to header)

#### 5. App (`src/App.tsx`)
- Main orchestrator
- State:
  - `selectedSpaceId`: currently selected space
  - `selectedSource`: currently selected source (if any)
  - `showScan`: controls ScanDialog visibility
- Handlers:
  - `handleSmartScan`: decides whether to scan selected source or open dialog
  - `handleScanAllSpaceSources`: calls `scanSpaceSources.mutateAsync` for current space
- Layout:
  - Sidebar on left
  - Header at top
  - SelectedSourceToolbar bar below header (when source selected)
  - Main content area (games grid/list)

### Hooks

#### `useScanning.ts`
- `useSourceScanStatus(spaceId, sourcePath)`: polls source scan status
  - Refetches every 2s when `scan_status === 'scanning'`, otherwise every 10s
  - Returns: `{ scan_status, scan_progress, scan_total, scan_error, ... }`
- `useStartSourceScan()`: mutation to start scanning a source
  - On success: invalidates source_scan_status and space_sources queries
- `useCancelSourceScan()`: mutation to cancel scanning
  - On success: invalidates source_scan_status query

#### `useSpaces.ts`
- `useScanSpaceSources()`: mutation to scan all active sources of a space
  - Backend: `scan_space_sources(space_id)`
  - Returns array of scanned games across all sources
  - **Note**: This mutation does NOT automatically add games to the space; it just returns found games. The UI currently shows games in the main list after refetch.

---

## Data Flow

### Scanning a Selected Source

1. User clicks on a source card in sidebar → `handleSelectSource` sets `selectedSource`
2. SelectedSourceToolbar appears
3. User clicks "Сканировать" in toolbar
4. `startSourceScan.mutate({ spaceId, sourcePath })` is called
5. Backend starts scan and returns immediately
6. `useSourceScanStatus` polls every 2s, updates UI with progress
7. When scan completes (`scan_status === 'completed'`), App's effect refetches games
8. User can cancel at any time with "Отменить" button

### Scanning All Space Sources

1. User selects a space (no source selected)
2. "Сканировать все" button appears in header
3. User clicks it → `handleScanAllSpaceSources`
4. `scanSpaceSources.mutateAsync(selectedSpaceId)` is called
5. Backend scans all active sources sequentially/parallel
6. On completion, `refetchGames()` is called to update library
7. **Note**: No detailed progress UI for individual sources during this operation

### Custom Folder Scan (ScanDialog)

1. User clicks "Сканировать папку" in header (or empty state button)
2. ScanDialog opens
3. User selects folder via button or typing path
4. User clicks "Сканировать" button
5. `scanDirectory.mutateAsync(path)` scans the folder
6. Results appear in dialog with checkboxes
7. User selects games, chooses target space, optionally enables metadata fetch
8. User clicks "Add selected" → creates games in database
9. Dialog closes

---

## State Management

### Local State (App.tsx)
- `selectedSpaceId`: string | null
- `selectedSource`: { spaceId, sourcePath } | null
- `showScan`: boolean

### Server State (React Query)
- `spaces`: list of spaces
- `games`: filtered by selectedSpaceId and selectedSource
- `source_scan_status`: per-source scan status (polling)
- Mutations: `startSourceScan`, `cancelSourceScan`, `scanSpaceSources`, `scanDirectory`, `createGame`

---

## Backend Integration

### Tauri Commands (src-tauri/src/commands/scanning.rs)
- `start_source_scan`: Starts scanning a specific source, creates a scan task
- `cancel_source_scan`: Cancels an ongoing scan
- `get_source_scan_status`: Returns current status of a source scan
- `scan_space_sources`: Scans all active sources of a space
- `scan_directory`: Scans an arbitrary directory (used by ScanDialog)

### Scanning Service (src-tauri/src/scanning_service.rs)
- Manages scan tasks, progress tracking
- Stores scan status in database (space_sources table has scan_status fields)
- Handles recursive directory scanning, game detection

---

## Current Issues

### 1. "Cancel scanning" button appears but doesn't work reliably
**Symptom**: When a source is selected and its status is "scanning", the cancel button shows. Pressing it may not cancel the scan.

**Root causes**:
- The `useCancelSourceScan` mutation uses regular `mutate` (non-async). Errors are not caught in the component.
- Backend cancellation might fail if the scan task has already completed or the task ID is lost.
- No visual feedback that cancellation is in progress (button disabled only during mutation, but mutation might be instant even if backend fails).

**Impact**: User thinks they canceled, but scan continues in background.

### 2. Scanning space sources doesn't work
**Symptom**: Clicking "Сканировать все" appears to do nothing or games don't appear.

**Root causes**:
- The `scanSpaceSources` mutation returns scanned games but does NOT automatically create them in the database.
- The current implementation only calls `refetchGames()` after scan completes, which fetches existing games from the database. But the scan only found games; they haven't been added yet.
- There's no UI to review and confirm games found from space scan (unlike ScanDialog which shows results).
- **Missing step**: After `scanSpaceSources` returns found games, the app should either:
  - Automatically add them to the space (with `createGame` mutations)
  - OR show a review dialog similar to ScanDialog to let user select which games to add

**Impact**: Users expect "Scan all" to add new games to their library, but it only scans and discards results.

### 3. Smart scan button behavior confusion
**Symptom**: When a source is selected, the header scan button says "Сканировать". But if the source is already scanning, clicking it again will start another concurrent scan? Or it might be disabled? Currently, the button is always enabled (no disabled state based on scanning status).

**Root causes**:
- The button doesn't check if the selected source is already scanning before allowing click.
- Could lead to multiple concurrent scans of the same source.
- No visual indication that a source is scanning in the header itself (only in the toolbar bar).

**Impact**: User might accidentally start duplicate scans.

### 4. No progress feedback for "Scan all"
**Symptom**: When scanning all space sources, there's no overall progress indicator. The header button doesn't show any loading state.

**Root causes**:
- `scanSpaceSources` mutation is async but we don't track its `isPending` state in the header.
- The mutation scans all sources sequentially on the backend; frontend has no visibility into which source is being scanned or overall progress.

**Impact**: User doesn't know if the scan started or how long it will take.

### 5. SelectedSourceToolbar cancel button disabled state
**Symptom**: The cancel button is disabled when `cancelScan.isPending` is true. But `cancelScan` mutation is called via `mutateAsync` in the click handler, so the `isPending` might not be set in time to prevent double-clicks. Also, the button should be disabled if no scan is actually in progress.

**Root causes**:
- The cancel button's disabled condition only checks `cancelScan.isPending`, not whether a scan is actually active.
- If the scan completes naturally before cancel request, the button should be hidden (it is conditionally rendered based on `isScanning`), but there could be race conditions.

**Impact**: Minor UX glitch.

### 6. ScanDialog's "Select Folder" button auto-selects but doesn't auto-scan
**Current behavior**: After selecting a folder, the path appears in the input but the user must click "Scan" separately. This is actually good UX (explicit action). However, the button label "Select Folder" might be misinterpreted as "Select and Scan". Consider renaming to "Browse" or "Choose Folder".

**Impact**: Minor clarity issue.

### 7. Inconsistent scan status polling
**Observation**: `useSourceScanStatus` polls every 2s when scanning. But for "Scan all", there's no polling for individual sources. The sources' status will update eventually when the user hovers over them or after some time because `useSourceScanStatus` also polls every 10s when idle. But there's no immediate update.

**Impact**: User doesn't see real-time progress of individual sources during "Scan all".

---

## Recommendations

### 1. Fix Cancel Mutation
- Use `mutateAsync` in SelectedSourceToolbar with proper error handling (already implemented)
- Add a `onSuccess` callback to invalidate queries and ensure UI updates
- Consider adding a confirmation dialog before canceling to prevent accidental clicks

### 2. Implement Review Flow for "Scan all"
- After `scanSpaceSources` completes, show a modal dialog (similar to ScanDialog) listing all newly found games across all sources
- Allow user to select which games to add, choose executables, covers, etc.
- Then perform `createGame` mutations for selected games
- This makes "Scan all" consistent with custom scan flow

### 3. Add Disabled State to Smart Scan Button
- In Header, disable the scan button when `selectedSource` is currently scanning
- Pass `selectedSource` to Header and check `isScanning` from its status (could use a query or derive from selectedSource's scan status)
- Alternatively, use a global state or context to track scanning sources

### 4. Show Progress for "Scan all"
- Add a loading state to the "Scan all" button while mutation is pending
- Consider showing a toast notification when scan starts/completes
- For detailed progress, could show a list of sources being scanned in a temporary panel

### 5. Improve ScanDialog Button Label
- Change "Select Folder" button text to "Browse" or "Choose Folder" to avoid implying immediate scan
- Or keep as is but make it clear that it's just for path selection

### 6. Force Refetch of Source Statuses After "Scan all"
- After `scanSpaceSources` completes, invalidate all `source_scan_status` queries for that space to trigger immediate polling updates
- This ensures the source items show updated status (completed/error) quickly

### 7. Add Error Handling for Scan All
- Show error message if `scanSpaceSources` fails
- Use try/catch in `handleScanAllSpaceSources` and display alert/toast (already partially implemented)

---

## Implementation Priority

1. **High**: Fix "Scan all" to actually add games (issue #2) - core functionality broken
2. **High**: Fix cancel button reliability (issue #1) - user control essential
3. **Medium**: Add disabled state to smart scan button (issue #3) - prevent duplicate scans
4. **Medium**: Add progress feedback for "Scan all" (issue #4) - UX improvement
5. **Low**: Clarify ScanDialog button text (issue #6) - minor clarity
6. **Low**: Force refetch of source statuses after "Scan all" (issue #7) - nice to have

---

## Conclusion

The scanning system has a solid foundation with clear separation of concerns, but the "Scan all" feature is incomplete and the cancel functionality needs robustness improvements. Addressing the high-priority items will make the scanning feature fully functional and user-friendly.
