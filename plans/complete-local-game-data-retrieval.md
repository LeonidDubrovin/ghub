# Plan: Complete Game Data Retrieval Using Real Catalog Examples

## Goal

Use the 600+ real game metadata examples from `games_metadate_examples/` as **golden master test data** to:
1. Add comprehensive unit tests for **title extraction** from folder paths
2. Validate that the algorithm produces the expected game titles
3. Identify and fix any failures to achieve 100% accuracy on real-world data

**Important:** The catalog JSON files contain only metadata (paths, names, executables as strings). We can test **title extraction from path strings**, but **cannot test executable finding** because the actual game directories and files do not exist on disk.

---

## Core Principle

**Each catalog entry provides:**
- `path` → input (folder path string)
- `name` → expected output (clean game title)

**Testing:** `extract_title_from_path(&entry.path)` should return `entry.name` (exact or near-exact match, case-insensitive).

This is **snapshot testing** with real data - the catalog represents the "truth" we must match.

---

## Current State

### Existing Implementation
- `src-tauri/src/title_extraction.rs` - title extraction logic (11 tests, 100+ cases)
- `src-tauri/src/scanner.rs` - directory scanning (23 tests, 50+ cases)
- Current title extraction tests: ~100 synthetic cases

### Available Test Data
- `games_metadate_examples/games_catalog 1.json` - ~1300 entries
- `games_metadate_examples/games_catalog 2.json` - ~1300 entries
- `games_metadate_examples/games_catalog 3.json` - ~500 entries
- **Total: ~3100 real game examples** with folder paths and expected titles

---

## Revised Plan (Focused on Title Extraction)

### Phase 1: Create Catalog-Based Unit Tests

#### Task 1.1: Define Catalog Data Structure
**File:** `src-tauri/tests/catalog_data.rs` (new)
**Purpose:** Load and parse catalog JSON files into Rust structs
```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CatalogEntry {
    pub name: String,
    pub path: String,
    pub engine: String,
    pub executable_path: String,
    pub itch_io_url: String,
    pub steam_url: String,
}

pub fn load_all_catalogs() -> Vec<CatalogEntry> {
    let mut all = Vec::new();
    for file in &["games_metadate_examples/games_catalog 1.json",
                  "games_metadate_examples/games_catalog 2.json",
                  "games_metadate_examples/games_catalog 3.json"] {
        let data = std::fs::read_to_file(file).unwrap();
        let mut entries: Vec<CatalogEntry> = serde_json::from_slice(&data).unwrap();
        all.append(&mut entries);
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_load_catalogs() {
        let catalog = load_all_catalogs();
        assert!(!catalog.is_empty());
        println!("Loaded {} catalog entries", catalog.len());
    }
}
```

#### Task 1.2: Add Main Title Extraction Test
**File:** `src-tauri/src/title_extraction.rs` (add to tests module)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::catalog_data::load_all_catalogs;
    
    #[test]
    fn test_title_extraction_from_catalog() {
        let catalog = load_all_catalogs();
        let mut failures = Vec::new();
        
        for entry in catalog {
            let extracted = extract_title_from_path(&entry.path);
            let extracted_clean = extracted.trim().to_lowercase();
            let expected_clean = entry.name.trim().to_lowercase();
            
            if extracted_clean != expected_clean {
                failures.push((entry.path, entry.name, extracted));
            }
        }
        
        if !failures.is_empty() {
            eprintln!("Failed {} out of {} entries:", failures.len(), catalog.len());
            for (path, expected, got) in failures.iter().take(10) {
                eprintln!("  Path: {}", path);
                eprintln!("    Expected: {}", expected);
                eprintln!("    Got:      {}", got);
                eprintln!("    Difference: {}", 
                    if expected.to_lowercase() == got.to_lowercase() {
                        "Only case difference"
                    } else {
                        "Actual content differs"
                    });
            }
            assert!(false, "{} title extraction failures", failures.len());
        }
    }
}
```

#### Task 1.3: Add Engine Detection Test (Optional)
**If engine detection is implemented:**
```rust
#[test]
fn test_engine_detection_from_catalog() {
    let catalog = load_all_catalogs();
    let mut mismatches = 0;
    
    for entry in catalog {
        if entry.engine == "Unknown" {
            continue; // Skip unknown engines
        }
        let detected = detect_engine_from_path(&entry.path);
        if detected != entry.engine {
            eprintln!("Engine mismatch: {} -> expected {}, got {}", 
                entry.path, entry.engine, detected);
            mismatches += 1;
        }
    }
    
    if mismatches > 0 {
        eprintln!("Total engine mismatches: {}", mismatches);
    }
    // Don't fail - Unknown engines may be legitimately undetectable
}
```

---

### Phase 2: Run Tests and Fix Failures

#### Task 2.1: Initial Test Run
**Command:** `cargo test title_extraction::tests::test_title_extraction_from_catalog --release -- --nocapture`
**Expected:** Many failures initially
**Output:** Count of failures, first 10 examples

#### Task 2.2: Analyze Failure Patterns
**Create:** `scripts/analyze_catalog_failures.rs` (standalone tool)
**Purpose:** Categorize failure types
```rust
enum FailureType {
    VersionSuffix,      // "Game v1.0" vs "Game"
    PlatformTag,        // "Game Windows" vs "Game"
    DemoLabel,          // "Game Demo" vs "Game"
    Punctuation,        // "Game: Director's Cut" vs "Game"
    CaseDifference,     // "game" vs "Game" (acceptable)
    CompletelyDifferent,// Major mismatch
}
```

**Output:** Statistics:
- Total entries: 3100
- Pass rate: X%
- Failure breakdown by type
- Examples of each type

#### Task 2.3: Iterative Fixes to `extract_title_from_path()`
**File:** `src-tauri/src/title_extraction.rs`

**Common patterns from catalog (to verify):**
- Version numbers: `v0.1`, `v1.2.3`, `ver 1.0`
- Platform: `Windows`, `Win64`, `x64`, `Linux`, `Mac`
- Release type: `Demo`, `Prologue`, `Alpha`, `Beta`, `Early Access`, `Preview`
- Build tags: `Build 123`, `r123`, `rev 1`
- Parenthetical: `(Windows)`, `[Win64]`, `{x64}`
- Punctuation: `:`, `-`, `|`, `·`
- Special chars: `'`, `"`, `!`, `?`, `©`

**Update logic to:**
1. Remove version numbers: `\bv\d+\.\d+\b` (and variants)
2. Remove platform tags: `\b(Windows|Win64?|Linux|Mac|OSX|x86_64|x64)\b`
3. Remove release types: `\b(Demo|Prologue|Alpha|Beta|Early\s+Access|Preview|Pre-alpha)\b`
4. Remove build numbers: `\b(?:Build|Ver|Version|r|rev)\s*\d+\b`
5. Remove parenthetical/bracketed: `\([^)]*\)`, `\[[^\]]*\]`, `\{[^}]*\}`
6. Normalize punctuation: replace `[-:|·]` with space
7. Trim whitespace, collapse multiple spaces
8. Handle case: preserve original case or normalize to title case

**After each change:**
- Run catalog test
- Check if failures decrease
- Ensure no regressions on existing ~100 tests

#### Task 2.4: Handle Edge Cases
**From catalog analysis:**
- Unicode characters (emoji, non-ASCII)
- Extremely long names (>260 chars)
- Names with only special characters
- Empty or near-empty names after cleaning
- Multiple consecutive separators

---

### Phase 3: Validate and Document

#### Task 3.1: Achieve 100% Pass Rate
**Goal:** All 3100+ catalog entries produce exact (or case-insensitive) title match
**Acceptable:** Case differences only (e.g., "game" vs "Game")
**Unacceptable:** Content differences (missing words, extra words, different words)

#### Task 3.2: Generate Coverage Report
```bash
cargo llvm-cov --html --output-dir coverage/
```
**Goal:** >95% coverage of `title_extraction.rs`

#### Task 3.3: Code Quality
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

#### Task 3.4: Update Documentation
- `IMPLEMENTATION_SUMMARY.md` - final test results, coverage, patterns handled
- `CATALOG_VALIDATION.md` (new) - catalog statistics, how to run tests, failure analysis
- `games_metadate_examples/README.md` - catalog structure, source, usage

---

### Phase 4: Test Infrastructure (Optional)

#### Task 4.1: Fix Test Runners
- `test_title_extraction.sh` - use script-relative paths
- `test_title_extraction.bat` - same
- Test on Windows

#### Task 4.2: Create Small Fixture for Fast Tests
**File:** `src-tauri/tests/fixtures/small_catalog.rs`
**Purpose:** 50 representative entries for quick CI/CD
**Selection:** Include all failure patterns, diverse engines

#### Task 4.3: Make Full Catalog Test Optional
**Reason:** 3100 entries may be slow for CI
```rust
#[test]
#[ignore] // Run with `cargo test -- --ignored`
fn test_title_extraction_catalog_full() {
    if std::env::var("RUN_FULL_CATALOG").is_err() {
        return;
    }
    // ... test code
}
```

---

## Detailed Task List

### Week 1: Setup and Initial Analysis

**Day 1:**
1. Create `tests/catalog_data.rs` - load all 3 JSON files
2. Add `test_title_extraction_from_catalog()` to `title_extraction.rs`
3. Run initial test, capture failure count and examples

**Day 2:**
4. Create `scripts/analyze_catalog_failures.rs` - categorize failures
5. Run analyzer, generate report
6. Identify top 3-5 failure patterns

**Day 3-7:**
7. Iteratively update `extract_title_from_path()`:
   - Add version number removal
   - Add platform tag removal
   - Add demo/alpha/beta removal
   - Add punctuation normalization
   - After each change: run test, verify failures decrease
8. Target: 0 failures on full catalog

### Week 2: Polish and Documentation

**Day 8-9:**
9. Run coverage report, add tests for any uncovered branches
10. Run `cargo clippy`, fix warnings
11. Run `cargo fmt --all`

**Day 10-11:**
12. Write `CATALOG_VALIDATION.md` - statistics, usage, patterns handled
13. Update `IMPLEMENTATION_SUMMARY.md` with final results
14. Write `games_metadate_examples/README.md`

**Day 12:**
15. Final validation on Windows
16. Update test runners if needed
17. Create small fixture for CI/CD

---

## Success Criteria

### Must Have
- [ ] All 3100+ catalog entries pass title extraction test (exact or case-insensitive match)
- [ ] All existing unit tests still pass (no regressions)
- [ ] Coverage >95% for title_extraction module
- [ ] No clippy warnings

### Should Have
- [ ] Engine detection test (if applicable)
- [ ] Small catalog fixture for fast tests
- [ ] Test runners work on Windows
- [ ] Comprehensive documentation

### Nice to Have
- [ ] Standalone failure analyzer tool
- [ ] Pre-commit hook
- [ ] Performance benchmarks

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Some catalog titles are "wrong" (don't match folder name) | Medium | High | Review catalog generation; may need to accept near-exact matches for truly ambiguous cases |
| High initial failure rate (many edge cases) | High | Medium | Allocate time for iterative fixes; use failure analyzer |
| Performance: full catalog test slow | Medium | Low | Make test optional; use small fixture for CI |
| Cannot achieve 100% due to catalog inconsistencies | Low | High | Document any unresolvable cases; consider catalog as "approximate truth" |

---

## Timeline

**Total: 10-14 days** (part-time)

- **Days 1-3:** Setup, initial test run, failure analysis (3d)
- **Days 4-7:** Iterative fixes to achieve 100% (4d)
- **Days 8-11:** Code quality, coverage, documentation (4d)
- **Day 12:** Final validation, polish (1d)

---

## References

- **Catalog data:** `games_metadate_examples/games_catalog *.json`
- **Title extraction:** `src-tauri/src/title_extraction.rs`
- **Existing tests:** `src-tauri/src/title_extraction.rs` (tests module)
- **Scanner constants:** `src-tauri/src/scanner_constants.rs` (engine detection)

---

## Key Clarification

**What we CAN test with catalog:**
- ✅ Title extraction from path strings (the main algorithm)
- ⚠️ Engine detection (if implemented, but catalog may have "Unknown")
- ❌ Executable finding - **NOT POSSIBLE** because actual game files don't exist

**What the catalog represents:**
- Real-world folder paths and expected clean titles
- Patterns from 3100+ actual indie games
- The "golden master" for title extraction accuracy

**Testing philosophy:**
- The catalog `name` is the **expected output**
- `extract_title_from_path(entry.path)` must produce `entry.name`
- Any deviation is a bug to be fixed
- Focus on **accuracy**, not approximation
