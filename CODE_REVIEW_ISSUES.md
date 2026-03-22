# Code Review Issues - Current State (After 3 Commits)

**Commits Analyzed:**
- `5d87c46` - fix: resolve compilation errors in scanning module
- `83cf80a` - feat: implement multiple space sources with background scanning
- `3a7786e` - Refactoring and bug fixes (note: commit message is corrupted)

**Date:** 2026-03-21

---

## 🔴 Critical Issues

### 1. Code Duplication Between Scanning Implementations
**Severity:** High

Two separate scanning implementations still exist with significant duplication:
- `src-tauri/src/commands/scanning.rs` (synchronous, config-based)
- `src-tauri/src/scanning_service.rs` (background service)

While [`scanner_constants.rs`](src-tauri/src/scanner_constants.rs) and [`title_extraction.rs`](src-tauri/src/title_extraction.rs) have been extracted, the scanning logic itself is still duplicated. This creates maintenance burden and risk of diverging behavior.

**Recommendation:** Extract a shared `scanner.rs` module with core scanning logic that both implementations can call, parameterized by progress callback and configuration.

---

### 2. Potential Deadlock with Lock Ordering
**Severity:** High

Lock acquisition order is inconsistent across the codebase:
- [`start_scan()`](src-tauri/src/scanning_service.rs:77-84): acquires `active_scans` (check), releases, then re-acquires for insert (though now atomic within one lock)
- [`cancel_scan()`](src-tauri/src/scanning_service.rs:151-159): acquires `active_scans`, then `db`
- [`scan_source()`](src-tauri/src/scanning_service.rs:177-419): acquires `db` multiple times, may access `active_scans` at end

While the recent fix improved the race condition, the overall lock ordering should be documented and enforced to prevent future deadlocks.

**Recommendation:** Establish a clear lock ordering policy (e.g., always acquire `active_scans` before `db`) and refactor all functions to follow it. Add comments documenting the order.

---

## 🟡 Medium Severity

### 3. Missing Unit Tests for Critical Functions
**Severity:** Medium

Core scanning and database functions lack test coverage:
- [`get_installs_for_source()`](src-tauri/src/database.rs:1304-1349) range query logic
- [`get_game_by_fingerprint()`](src-tauri/src/database.rs:631-728) matching behavior
- [`compute_fingerprint()`](src-tauri/src/scanning_service.rs:528-547) fallback strategies
- [`extract_title_with_fallback()`](src-tauri/src/title_extraction.rs:2709-2760) multi-level heuristics
- Windows path handling edge cases

**Recommendation:** Add unit tests covering:
- Path prefix matching with various separators (Windows `\`, Unix `/`)
- Fingerprint collision scenarios
- Title extraction fallback chain
- Concurrent scan operations

---

### 4. TypeScript `scan_status` Type Could Be Clearer
**File:** [`src/types/index.ts:23`](src/types/index.ts:23)
**Severity:** Medium

The `scan_status` field is optional (`undefined`), which is used to mean "idle". This weakens type safety.

```typescript
scan_status?: 'idle' | 'scanning' | 'completed' | 'error';
```

**Recommendation:** Make it required with explicit 'idle' value, or use a discriminated union:
```typescript
type ScanStatus = 
  | { status: 'idle' }
  | { status: 'scanning'; progress: number; total: number }
  | { status: 'completed' }
  | { status: 'error'; error: string };
```

---

### 5. Backup Command Lacks Validation
**File:** [`src-tauri/src/commands/backup.rs`](src-tauri/src/commands/backup.rs)
**Severity:** Medium

The backup command doesn't verify that the backup directory exists and is writable before attempting the backup operation.

**Recommendation:** Add pre-flight checks:
```rust
let backup_dir = app_data_dir.join("backups");
if !backup_dir.exists() {
    std::fs::create_dir_all(&backup_dir)?;
}
// Optionally check writable permissions
```

---

### 6. `extract_title_with_fallback` Complexity
**File:** [`src-tauri/src/title_extraction.rs:2709-2760`](src-tauri/src/title_extraction.rs:2709-2760)
**Severity:** Medium

The title extraction function has a complex multi-level fallback strategy with 5 levels. While extracted to a separate module, the function itself is long and could benefit from being broken into smaller, testable functions with clearer configuration.

**Recommendation:** Consider:
- Breaking into separate functions per fallback level
- Making the fallback order/configurable
- Adding more comprehensive tests for each level

---

## 🟢 Low / Suggestions

### 7. `SpaceItem.handleSourceRemoved` Removed
**File:** [`src/components/SpaceItem.tsx`](src/components/SpaceItem.tsx)
**Status:** Resolved by removal

The `onRemoved` callback prop was removed from [`SourceItem`](src/components/SourceItem.tsx), so this no-op is no longer an issue. The parent component can handle updates via query invalidation or refetch.

---

### 8. `ScanDialog` Auto-Scan Removed
**File:** [`src/components/ScanDialog.tsx`](src/components/ScanDialog.tsx)
**Status:** Resolved

The automatic scan trigger on mode switch was removed. Users now click "Scan Now" manually, preventing accidental or unwanted scans.

---

### 9. `SourceItem` Remove Button Disabled During Scan
**File:** [`src/components/SourceItem.tsx:168`](src/components/SourceItem.tsx:168)
**Status:** Resolved

The remove button is now disabled while a scan is in progress: `disabled={removeSource.isPending || isScanning}`.

---

### 10. `AddSpaceDialog` Partial Failure Handling
**File:** [`src/components/AddSpaceDialog.tsx:70-95`](src/components/AddSpaceDialog.tsx:70-95)
**Status:** Resolved

The dialog now tracks failed sources and keeps the dialog open on partial failure, showing an error message with details.

---

### 11. Migration Error Handling Improved
**File:** [`src-tauri/src/database.rs:161-236`](src-tauri/src/database.rs:161-236)
**Status:** Resolved

All `ALTER TABLE` statements now properly handle "duplicate column" errors while propagating other errors.

---

### 12. Fingerprint Fallback Simplified
**File:** [`src-tauri/src/scanning_service.rs:528-547`](src-tauri/src/scanning_service.rs:528-547)
**Status:** Resolved

The fallback now uses only the title (more stable) instead of title + folder size (which fluctuates due to logs/caches).

---

### 13. Cancellation Checks Added
**File:** [`src-tauri/src/scanning_service.rs:235-239`](src-tauri/src/scanning_service.rs:235-239)
**Status:** Resolved

The mark-missing loop now periodically checks the cancellation flag every 100 iterations.

---

### 14. Logging Upgraded
**File:** [`src-tauri/src/commands/spaces.rs`](src-tauri/src/commands/spaces.rs)
**Status:** Resolved

`println!` statements have been replaced with `log` crate macros (`debug!`, `info!`, etc.).

---

### 15. Magic Numbers Moved to Constants
**Files:** [`src-tauri/src/scanner_constants.rs`](src-tauri/src/scanner_constants.rs)
**Status:** Resolved

All hardcoded scanning limits are now defined as constants:
- `MAX_SCAN_DEPTH: 5`
- `MAX_EXE_SEARCH_DEPTH: 4`
- `MAX_COVER_CANDIDATES: 15`
- `MAX_COVER_SEARCH_DEPTH: 3`
- `MAX_GAME_FOLDER_SEARCH_DEPTH: 2`

---

## ⚠️ Important Notes

### Corrupted Commit Message

Commit `3a7786e` has a **shell command** as its message (`$(cat <<'EOF'`). This is a **critical git hygiene issue** that should be fixed immediately:

```bash
# Option 1: Amend the most recent commit (if not pushed)
git commit --amend -m "refactor: improve scanning reliability and fix critical bugs"

# Option 2: Interactive rebase to edit commit message
git rebase -i HEAD~3
# Mark the problematic commit as 'edit', then:
git commit --amend -m "feat: enhance scanning with deduplication and error handling"
git rebase --continue
```

**Do not rewrite history if already pushed to shared repository** - coordinate with team first.

---

### Build Status

Unable to verify compilation due to terminal encoding issues in the review environment. **Run `cargo check` manually to ensure no compilation errors**, especially:
- Missing imports for new modules
- Regex compilation errors in `scanner_constants`
- Correct feature flags for Windows-specific code

---

## Summary

The codebase has been **significantly improved** from the initial `83cf80a` implementation. Most critical bugs are fixed:

✅ Race condition in `start_scan`  
✅ Incorrect LIKE pattern in `get_installs_for_source`  
✅ Silent errors in `create_space`  
✅ Game duplication across spaces  
✅ Panic cleanup  
✅ Migration error handling  
✅ UI error handling (SourceItem, AddSpaceDialog, ScanDialog)  
✅ Auto-scan removed  
✅ Remove button disabled during scan  
✅ Fingerprint fallback simplified  
✅ Cancellation checks added  
✅ Logging upgraded  
✅ Constants extracted  

**Remaining high-priority items:**
1. Eliminate code duplication between scanning implementations
2. Document and enforce lock ordering
3. Add comprehensive unit tests
4. Fix commit message corruption

The architecture is sound, but addressing the remaining issues will improve maintainability and reliability.
