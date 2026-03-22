# Final Implementation Plan: Scanning UI Simplification

## User Requirements (Confirmed)

1. **Remove "Scan all" button completely** - users prefer scanning each directory individually
2. **When source selected, header should not show scan buttons** - toolbar provides all necessary actions
3. **Keep "Scan folder" button only when no source is selected**
4. **Fix ScanDialog button label** (Select Folder → Browse)
5. **Remove misleading "Scan all" button from SpaceSettingsDialog**

## Changes Required

### A. Header.tsx

**A.1 Remove "Scan all" button entirely**
- Delete button rendering and `onScanAllSpaceSources` prop from interface
- Remove conditional check for `selectedSpaceId && onScanAllSpaceSources`

**A.2 Conditionally render "Scan folder" button**
```tsx
// BEFORE:
<button onClick={onScan} className="btn btn-primary text-sm">
  {selectedSource ? t('space.scan') : t('actions.scanFolder')}
</button>

// AFTER:
{!selectedSource && (
  <button onClick={onScan} className="btn btn-primary text-sm">
    {t('actions.scanFolder')}
  </button>
)}
```

**A.3 Update HeaderProps interface**
- Remove `onScanAllSpaceSources?: () => void;`
- Remove `selectedSpaceId?: string | null;` (if no longer needed - check usage)
- Keep `selectedSource?: SelectedSource | null;` (for potential future use, though not used now)

**A.4 Adjust layout**
- Ensure view mode toggles remain properly aligned when scan button is hidden
- May need to adjust spacing/justify-between

### B. App.tsx

**B.1 Remove scanSpaceSources mutation**
```typescript
// Remove:
const scanSpaceSources = useScanSpaceSources();
```

**B.2 Remove handleScanAllSpaceSources function**
```typescript
// Delete entire function (lines ~404-414)
```

**B.3 Update Header props**
```tsx
<Header
  ...
  onScanAllSpaceSources={handleScanAllSpaceSources}  // REMOVE THIS LINE
  ...
/>
```

**B.4 Remove selectedSource prop from Header if not used**
- Check if Header uses `selectedSource` prop - if not, remove it
- Header currently receives `selectedSource` but doesn't use it (line 460 in App.tsx)
- Can remove this prop entirely

**B.5 Verify handleSmartScan still works**
- `handleSmartScan` checks `selectedSource` and either starts scan or opens dialog
- With header button hidden when source selected, this function is only called when no source selected (from header)
- So it will always open dialog - can simplify but not required

### C. SelectedSourceToolbar.tsx

**No changes required** - already provides correct actions: Scan, Cancel, Remove

**Optional improvements:**
- Add confirmation dialog before canceling scan
- Show toast on scan complete/error

### D. ScanDialog.tsx

**D.1 Change "Select Folder" button text**
```tsx
// Line ~184-186
<button
  onClick={handleSelectFolder}
  disabled={isScanning}
  className="btn btn-primary"
>
  📁 {t('space.browse')}
</button>
```

**D.2 Verify translations exist**
- `space.browse` exists in en.json: "Browse"
- `space.browse` exists in ru.json: "Обзор"
- No changes needed to locale files

### E. SpaceSettingsDialog.tsx

**E.1 Remove "Scan all" button from bottom actions**
```tsx
// Remove lines ~190-196:
<button
  onClick={handleScan}
  className="btn btn-primary"
  disabled={sources.filter(s => s.is_active).length === 0}
>
  🔍 {t('scan.scanAll')}
</button>
```

**E.2 Adjust actions layout**
- The bottom bar has "Close" and "Scan all" buttons
- After removal, only "Close" remains - may need to adjust alignment
- Could remove the entire actions bar if only close is needed, or keep for consistency

**E.3 Remove handleScan function if unused**
```typescript
// Delete:
const handleScan = () => {
  setShowScanDialog(true);
};

// And remove showScanDialog state and its usage
```

**E.4 Remove ScanDialog from render if never shown**
```tsx
// Remove:
{showScanDialog && (
  <ScanDialog
    spaces={[space]}
    onClose={handleCloseScanDialog}
  />
)}
```

### F. Additional Improvements (Optional but Recommended)

**F.1 Add toast notifications**
- Install toast library or use existing
- Show success/error messages for scan operations

**F.2 Add confirmation before canceling scan**
- In SelectedSourceToolbar, show confirm dialog before calling `cancelScan`
- Prevents accidental cancellation

**F.3 Improve button disabled states**
- Ensure "Scan" button in toolbar is disabled when already scanning
- Already has `startScan.isPending` check - verify it works correctly

**F.4 Force refetch after scan completes**
- Already handled in `useStartSourceScan` onSuccess (invalidates queries)
- Should be sufficient

## Implementation Order

1. **Phase 1: Remove "Scan all" from Header** (30 min)
   - A.1, A.2, A.3, A.4
   - B.1, B.2, B.3, B.4
   - Test: Header shows only "Scan folder" when no source selected; hides when source selected

2. **Phase 2: Clean up SpaceSettingsDialog** (15 min)
   - E.1, E.2, E.3, E.4
   - Test: Space settings dialog no longer shows "Scan all" button

3. **Phase 3: ScanDialog button label** (5 min)
   - D.1, D.2
   - Test: Button shows "Browse" (or localized equivalent)

4. **Phase 4: Polish** (1 hour)
   - F.1, F.2, F.3, F.4
   - Test all flows: custom scan, source scan, cancel, errors

## Expected Outcome

- **Simpler UI**: No confusing duplicate buttons
- **Clear context**: Header shows only "Scan folder" for custom scans; toolbar handles source scans
- **Consistent**: SpaceSettingsDialog focuses on source management only
- **No broken functionality**: "Scan all" removed entirely (as requested)

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Users miss "Scan all" bulk operation | Users explicitly prefer individual scanning |
| Breaking changes for existing users | Individual scanning is already the primary flow; "Scan all" was secondary and broken |
| Header layout breaks when button hidden | Use conditional rendering that preserves layout (opacity-0 or proper flex) |

---

**Plan ready for implementation. All changes are straightforward UI simplifications with no major architectural impact.**
