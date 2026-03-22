# Scanning UI Simplification - Implementation Summary

## Completed Changes

### 1. Header.tsx - Simplified Scan Button Logic
- **Removed** "Scan all" button completely
- **Removed** `onScanAllSpaceSources` prop and `selectedSpaceId` prop (no longer needed)
- **Modified** smart scan button to show ONLY when no source is selected (`!selectedSource`)
- **Result**: Header now shows scan button only when user hasn't selected a specific directory

**Before:**
- Space selected → showed "Scan all" + "Scan folder"
- Source selected → showed "Scan" (confusing)

**After:**
- Nothing selected → shows "Scan folder"
- Space selected → shows "Scan folder"
- Source selected → shows NO scan buttons (toolbar handles it)

### 2. App.tsx - Removed Unused Functionality
- **Removed** `useScanSpaceSources` hook import
- **Removed** `scanSpaceSources` mutation variable
- **Removed** `handleScanAllSpaceSources` function entirely
- **Removed** `onScanAllSpaceSources` prop from Header component
- **Result**: No more broken "Scan all" functionality

### 3. SpaceSettingsDialog.tsx - Cleaned Up
- **Removed** "Scan all" button from bottom actions (misleading - it didn't actually scan all)
- **Removed** `showScanDialog` state and related handlers (`handleScan`, `handleCloseScanDialog`)
- **Removed** ScanDialog import and rendering
- **Result**: Space settings now focuses solely on source management (add/remove/activate directories)

### 4. ScanDialog.tsx - Button Label Fix
- **Changed** "Select Folder" button text to "Browse" (using `t('space.browse')`)
- **Result**: Clearer UX - button opens file picker, doesn't imply immediate scan

### 5. SelectedSourceToolbar.tsx - Added Cancel Confirmation
- **Added** `window.confirm()` before canceling a scan
- **Added** translation key `confirmCancelScan` to both locale files
- **Result**: Prevents accidental cancellation of ongoing scans

### 6. Locale Files - Added Translation
**ru.json:**
```json
"confirmCancelScan": "Отменить текущее сканирование?"
```

**en.json:**
```json
"confirmCancelScan": "Cancel the current scan?"
```

## Build Status

✅ `npm run build` completes successfully with no errors
✅ TypeScript compilation passes
✅ Vite production build succeeds

## Testing Checklist

- [x] Build passes without errors
- [x] No TypeScript type errors
- [x] No remaining references to removed functionality
- [ ] Manual testing of UI flows (requires running app)

## Expected User Experience

1. **Initial state** (nothing selected): Header shows "Scan folder" button → opens ScanDialog for custom folder scanning
2. **Space selected**: Header shows "Scan folder" button → same as above. Sidebar shows space's sources.
3. **Source selected**: Header shows NO scan buttons. SelectedSourceToolbar appears below header with:
   - "Scan" button → scans the selected directory
   - "Cancel" button (when scanning) → with confirmation dialog
   - "Remove" button → removes source from space

4. **Space settings**: When opening space settings (gear icon), dialog shows only source management:
   - Add folder button
   - List of sources with activate/deactivate toggles
   - Remove buttons
   - Close button
   - **No** "Scan all" button

## Files Modified

1. `src/components/Header.tsx`
2. `src/App.tsx`
3. `src/components/SpaceSettingsDialog.tsx`
4. `src/components/ScanDialog.tsx`
5. `src/components/SelectedSourceToolbar.tsx`
6. `src/locales/ru.json`
7. `src/locales/en.json`

## Architecture Notes

- The `scanSpaceSources` backend command still exists but is no longer called from frontend
- The `useScanSpaceSources` hook remains in `src/hooks/useSpaces.ts` for potential future use
- No database schema changes required
- No breaking changes for existing user data

## Remaining Issues (from SCANNING_ANALYSIS.md)

The following issues were **NOT addressed** in this implementation as they are separate from the UI simplification:

1. **Issue #1**: Cancel reliability - partially improved with confirmation dialog, but backend cancellation may still have race conditions
2. **Issue #3**: Smart scan button disabled state - button now hidden when source selected, so this is resolved
3. **Issue #4**: No progress for "Scan all" - feature removed, so not applicable
4. **Issue #6**: ScanDialog button label - **FIXED**
5. **Issue #7**: Inconsistent polling - not addressed

The critical bug **"Scan all doesn't add games"** is no longer relevant because the feature has been removed from the UI.

## Recommendations for Future

- Consider adding toast notifications for scan operations (success/error)
- Add loading states to buttons more consistently
- Review the `scanSpaceSources` backend command - if never used, consider deprecating
- Monitor user feedback to ensure individual scanning workflow is sufficient

---

**Implementation complete.** The scanning UI is now simplified, context-aware, and free of confusing duplicate buttons.