# Asynchronous Scanning Implementation Plan

## Problem Analysis

### Current Behavior
- When adding directories to a space via `AddSpaceDialog`, sources are added to the database synchronously
- Scanning is a separate manual operation via `ScanDialog` or `scan_space_sources` command
- The `scan_space_sources` command is **synchronous** and blocks until completion, which can take significant time for large directories
- UI freezes during scanning, providing no feedback to the user

### Identified Blocking Points
1. **`scan_space_sources` command** (src-tauri/src/commands/scanning.rs:174-234) - performs synchronous directory walking
2. **`scan_directory_internal`** - heavy I/O operations with `WalkDir` that block the thread
3. **No status tracking** - cannot query scan progress or results without re-scanning
4. **No background execution** - scanning runs on the main thread, blocking the UI

## Proposed Solution

### Architecture Overview
Implement an **asynchronous scanning system** with the following components:

```
┌─────────────────┐     start_scan     ┌──────────────────┐
│   AddSpaceDialog │ ────────────────> │  Scanning Service│
│   (or UI)       │                    │  (Background)    │
└─────────────────┘                   └──────────────────┘
                                              │
                                              │ updates
                                              ▼
                                    ┌──────────────────┐
                                    │   Database       │
                                    │  (scan_status,   │
                                    │   scan_result)   │
                                    └──────────────────┘
                                              │
                                              │ poll
                                              ▼
                                    ┌──────────────────┐
                                    │   Frontend       │
                                    │  (status query)  │
                                    └──────────────────┘
```

### Key Design Decisions

1. **Database Schema Changes**
   - Add `scan_status` column to `space_sources` table
     - Values: `idle`, `scanning`, `completed`, `error`
   - Add `scan_result` column (JSON) to store scan results temporarily
   - Add `scan_started_at` and `scan_completed_at` timestamps
   - Add `scan_error` column for error messages

2. **Scanning Service**
   - Spawn background tasks using `std::thread::spawn` or `tokio::task::spawn_blocking`
   - Maintain in-memory map of active scans: `HashMap<space_id, ScanHandle>`
   - Update database with status and results as scan progresses
   - Support cancellation (optional for v1)

3. **New Commands**
   - `start_space_scan(space_id: String) -> Result<ScanJob, String>` - Non-blocking, returns immediately
   - `get_space_scan_status(space_id: String) -> Result<ScanStatus, String>` - Poll for status
   - `get_space_scan_results(space_id: String) -> Result<Vec<ScannedGame>, String>` - Retrieve results
   - `cancel_space_scan(space_id: String) -> Result<(), String>` - Optional cancellation

4. **Frontend Changes**
   - New hook: `useStartSpaceScan()` - triggers async scan
   - New hook: `useSpaceScanStatus(spaceId)` - polls scan status
   - New hook: `useSpaceScanResults(spaceId)` - retrieves results
   - `AddSpaceDialog`: After adding sources, automatically call `start_space_scan()`
   - Show scanning status indicator (spinner + progress)
   - `ScanDialog`: Support both immediate scan (blocking for small dirs) and async scan (for large dirs)

5. **Result Storage Strategy**
   - **Option A (In-Memory)**: Store results in a `Arc<Mutex<HashMap<space_id, Vec<ScannedGame>>>>`
     - Fast, but lost on app restart
     - Simpler implementation
   - **Option B (Database)**: Store results in a new `scan_results` table
     - Persistent, can survive restarts
     - More complex, requires cleanup
   - **Recommended**: Use **Option A** for simplicity, results are transient and only needed for current session

## Implementation Steps

### Phase 1: Database & Models
1. Update `src-tauri/src/database.rs`:
   - Add columns to `space_sources` table in `init_schema()`
   - Add migration for existing databases
   - Add methods:
     - `set_space_source_scan_status(space_id, source_path, status, error)`
     - `clear_space_source_scan_status(space_id, source_path)`
     - `get_space_source_scan_status(space_id) -> Vec<(source_path, status, error)>`

2. Update `src-tauri/src/models.rs`:
   - Add `scan_status: Option<String>` to `SpaceSource`
   - Add `scan_error: Option<String>` to `SpaceSource`
   - Create `ScanStatus` enum/struct for status queries

### Phase 2: Rust Backend
3. Create `src-tauri/src/scanning_service.rs`:
   - `ScanningService` struct with `active_scans: HashMap<String, ScanJob>`
   - `start_scan(space_id, sources)` method - spawns background task
   - Background task function:
     - Update DB status to `scanning`
     - Call `scan_directory_internal` for each source
     - Store results in memory map
     - Update DB status to `completed` or `error`
   - `get_status(space_id)` method
   - `get_results(space_id)` method
   - `cancel(space_id)` method (optional)

4. Update `src-tauri/src/commands/spaces.rs`:
   - Add `start_space_scan` command
   - Add `get_space_scan_status` command
   - Add `get_space_scan_results` command
   - Add `cancel_space_scan` command (optional)

5. Update `src-tauri/src/commands/mod.rs`:
   - Re-export new scanning commands

6. Update `src-tauri/src/lib.rs`:
   - Initialize `ScanningService` in `AppState`
   - Register commands

### Phase 3: Frontend
7. Update `src/hooks/useSpaces.ts`:
   - `useStartSpaceScan()` mutation
   - `useSpaceScanStatus(spaceId)` query (polling every 2-3 seconds)
   - `useSpaceScanResults(spaceId)` query

8. Update `src/components/AddSpaceDialog.tsx`:
   - After adding sources, call `startSpaceScan.mutateAsync(spaceId)`
   - Show "Scanning..." status with spinner
   - Optionally navigate to space view where scan status is shown

9. Create `src/components/ScanStatus.tsx`:
   - Display current scan status (idle, scanning, completed, error)
   - Show progress indicator (number of sources scanned / total)
   - Show error message if failed
   - Button to view results when complete

10. Update `src/components/ScanDialog.tsx`:
    - Add "Async Scan" mode that uses the new background scanning
    - Show real-time status updates
    - Auto-load results when scan completes

### Phase 4: Testing & Polish
11. Test with large directories (10k+ files)
12. Test error handling (permission denied, path not found)
13. Test cancellation (if implemented)
14. Add proper error messages to UI
15. Optimize memory usage (stream results instead of collecting all)

## Technical Considerations

### Thread Safety
- Use `Arc<Mutex<>>` for shared state in `ScanningService`
- Ensure `scan_directory_internal` is thread-safe (it appears to be - uses local variables)
- Database access from background threads: Each thread needs its own DB connection or use connection pool

### Database Connections
- Current `AppState.db` is a `Mutex<Connection>` - not safe for concurrent access from multiple threads
- Solution: Create a new DB connection for each background scan thread
- Or use `r2d2` connection pool (more complex)

### Progress Reporting (Future Enhancement)
- Could add `on_progress` callback via Tauri events
- Emit events: `scan_progress { source: "...", files_scanned: 123, games_found: 5 }`
- Frontend listens to events and updates UI in real-time

### Cancellation Support
- Use `std::sync::atomic::AtomicBool` flag per scan
- Check flag periodically during scanning (in `WalkDir` loop)
- Clean up resources on cancellation

## Migration Plan

### Database Migration
```sql
-- Add new columns to space_sources
ALTER TABLE space_sources ADD COLUMN scan_status TEXT;
ALTER TABLE space_sources ADD COLUMN scan_result TEXT; -- JSON
ALTER TABLE space_sources ADD COLUMN scan_started_at TEXT;
ALTER TABLE space_sources ADD COLUMN scan_completed_at TEXT;
ALTER TABLE space_sources ADD COLUMN scan_error TEXT;
```

### Code Migration
- Keep existing `scan_space_sources` command for backward compatibility
- New commands use the same scanning logic but run asynchronously
- Gradual rollout: Add new commands, update frontend, deprecate old command later

## Rollout Strategy

1. **Backend Only**: Implement async scanning commands, keep old synchronous command
2. **AddSpaceDialog**: After creating space with sources, call `start_space_scan` in background
3. **Space View**: Show scan status badge on space cards
4. **ScanDialog**: Add option to "Scan in Background" for large directories
5. **Notifications**: Toast notification when scan completes
6. **Deprecate**: Eventually remove `scan_space_sources` or make it async too

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Memory leaks from accumulated scan results | High | Implement TTL for results, auto-cleanup after 1 hour |
| Database lock contention | Medium | Use separate DB connections per thread |
| UI doesn't update on scan completion | Medium | Use React Query invalidation on scan status change |
| Large scans consume too much CPU | Medium | Add configurable `max_scan_depth` and `max_exe_search_depth` |
| Crash during scan leaves stale status | Medium | Clear status on service startup, implement heartbeat |

## Success Metrics

- [ ] UI remains responsive during large directory scans (10k+ files)
- [ ] Scan status visible to user within 2 seconds of starting
- [ ] Results available within 5 seconds of scan completion
- [ ] No memory leaks after 100+ scans
- [ ] Error messages displayed clearly to user

## Estimated Effort

- Database changes: 1 hour
- Rust backend: 4-6 hours
- Frontend: 3-4 hours
- Testing & polish: 2-3 hours
- **Total: 10-14 hours**

## Future Enhancements

1. **Progress Events**: Real-time progress via Tauri events
2. **Scan Scheduling**: Schedule scans for off-hours
3. **Incremental Scans**: Only scan changed directories (using file modification times)
4. **Distributed Scans**: Scan multiple spaces in parallel (with configurable concurrency limit)
5. **Scan History**: Keep history of scans for comparison
6. **Smart Scanning**: Skip unchanged directories based on cached directory hashes
