# Unit Tests for Title Extraction

## Overview
This document describes the comprehensive unit tests added to `src-tauri/src/title_extraction.rs` based on real-world examples from the application logs.

## Test Coverage

### 1. `test_clean_game_title()`
Tests the title cleaning function with various inputs:

**Version number removal:**
- `"MyGame v1.0"` → `"MyGame"`
- `"Game v1.2.3"`, `"Game V1.0.0"`, `"Game_1.0.1"`, `"Game-2.0.0-beta"` all correctly strip version tags

**Platform tag removal:**
- `"Game (Windows)"` → `"Game"`
- `"Game (PC)"`, `"Game_GOG"`, `"Game_Steam"` all correctly strip platform tags

**Trailing parenthesis stripping (truncation handling):**
- `"Game ("` → `"Game"`
- `"Game ["` → `"Game"`
- `"Game (Demo"` → `"Game"`

**Generic folder name rejection (returns empty string):**
- `"Windows"`, `"win64"`, `"Binaries"`, `"Engine"`, `"jre"`, `"en-us"`, `"MACOSX"`, `"gmlive"`, `"Build"`, `"runtime"` all return empty

**Real log examples:**
- `"(Win)Project Troll v2.2"` → `"(Win)Project Troll"`
- `"Bridgebourn Demo Win64 v0-6-29"` → `"Bridgebourn Demo Win64"`
- `"COOKnRUN_1.1"` → `"COOKnRUN"`
- `"A Night Around The Fire_2022Update"` → `"A Night Around The Fire 2022Update"`

### 2. `test_is_likely_sentence()`
Tests the sentence detection filter that prevents metadata like readme content from being used as game titles:

**Sentences correctly rejected:**
- `"This is a game"` (contains " is ")
- `"garden is a collaboration between:"` (contains " is " and ends with colon)
- `"The game was made by John"` (contains " was " and " by ")
- `"Controls: WASD to move"` (ends with colon)
- `"To play the game you must extract the folder"` (contains " to ")
- `"made by Friedrich Hanisch"` (contains " by ")
- `"System requirements: 4GB RAM"` (ends with colon)

**Valid game titles correctly accepted:**
- `"MyGame"`, `"The Legend of Zelda"`, `"Super Mario Odyssey"`
- `"Game Name 2022"`, `"Bikrash"`, `"DANGEON!"`

### 3. `test_is_generic_exe_name()`
Tests filtering of utility executables that shouldn't be used as game titles:

**Generic names rejected:**
- `"launcher"`, `"setup"`, `"Unity Player"`, `"UE4 Game"`
- `"godot engine"`, `"BootstrapPackagedGame"`, `"crashreport"`
- `"WindowsNoEditor"`, `"shipping"`, `"debug"`, `"runtime"`
- `"redistributable"`, `"microsoft"`, `"nvidia"`, `"amd"`, `"intel"`
- `"steam"`, `"epic"`, `"gog"`, `"origin"`, `"ubisoft"`
- `"ea"`, `"rockstar"`, `"bethesda"`, `"2k"`, `"sega"`
- `"square enix"`, `"capcom"`, `"konami"`, `"bandai namco"`
- `"activision"`, `"blizzard"`, `"microsoft studios"`
- `"xbox"`, `"playstation"`, `"nintendo"`

**Specific game names accepted:**
- `"MyGame"`, `"Awesome Game"`, `"Roguelike"`, `"Project Troll"`

### 4. `test_is_problematic_game_name()`
Tests names known to cause incorrect metadata matches:

**Problematic names rejected:**
- `"ICARUS"`, `"Godot Engine"`, `"BootstrapPackagedGame"`
- `"WindowsNoEditor"`, `"Win64"`, `"Win32"`, `"Shipping"`
- `"Development"`, `"Debug"`, `"Release"`

**Normal names accepted:**
- `"MyGame"`, `"Roguelike"`

### 5. `test_try_extract_from_local_metadata()`
Tests metadata extraction with validation:

**Valid metadata accepted:**
- `name: "Bikrash"` with description → returns `"Bikrash"`

**Invalid metadata rejected:**
- Sentence-like name: `"To play the game you must extract the folder"` → `None`
- Generic name: `"Windows"` → `None`
- Empty name: `""` → `None`
- No metadata → `None`

### 6. `test_extract_title_with_fallback_scenarios()`
Integration tests covering the full fallback strategy with 11 realistic scenarios:

**Scenario 1-2:** Basic functionality
- Metadata with good name (Level 0) → uses metadata
- No metadata, good dir name (Level 1) → uses dir name

**Scenario 3-10:** Generic folder name handling (Level 3 fallback to parent)
- `"Windows"` → parent `"MyCollection"`
- `"jre"` (Java runtime) → parent `"Greedy Miners"`
- `"en-us"` (language folder) → parent `"Redist"`
- `"Build"` → parent `"MyGame"`
- `"D3D12"` → parent `"Blattgold Download"`
- `"New folder"` → parent `"Animal Crushing"`
- `"WindowsClient"` → parent `"Balance'em"`
- `"Win"` → parent `"GMTK2025"`

**Scenario 11:** Multiple generic levels
- `"games"` in `/games_genre/games_tmp_for_dev/games` → parent `"games_tmp_for_dev"`

### 7. `test_extract_title_from_executable()`
Tests executable name extraction:

**Valid:**
- `"MyGame.exe"` → `"MyGame"`
- `"MyGame_v1.0.exe"` → `"MyGame"` (version stripped)

**Rejected:**
- `"launcher.exe"` → `None` (generic name filtered)

**No executable:**
- `None` → `None`

### 8. `test_real_world_examples_from_logs()`
Tests based on actual problematic cases from the logs:

**Metadata validation:**
- ✅ `"DANGEON!"` from readme → accepted (good name)
- ❌ `"This demo is purely to showcase the core gameplay."` → rejected (sentence)
- ❌ `"Control: WASD - Movement"` → rejected (controls description)
- ❌ `"To play the game you must extract the folder"` → rejected (instructional sentence)
- ❌ `"----------------------------------------------------------------------------------"` → rejected (separator line)
- ❌ `"{"` from broken JSON → rejected (invalid)
- ❌ `"-------- Controls"` → rejected (UI text)
- ❌ `"It is a fast-paced first-person alien action game "` → rejected (sentence)
- ❌ `"Thank you so much for purchasing \"Infineural\"."` → rejected (starts with "Thank you")

**Real folder structures tested:**
- `jre/bin` with `javaws.exe` → correctly uses parent `"Greedy Miners"`
- `Engine/Extras/Redist/en-us` → correctly uses parent `"Redist"`
- `Build/D3D12` → correctly uses parent `"Blattgold Download"`
- `WindowsClient` → correctly uses parent `"Balance'em"`
- `Win` subfolder → correctly uses parent `"GMTK2025"`

### 9. `test_exe_metadata_extraction()`
Tests EXE metadata product name extraction:

**Valid product name:**
- `"Roguelike"` → accepted

**Generic product name rejected:**
- `"Unity Player"` → rejected

**Deep subfolder detection:**
- Path `"/games/MyGame/Engine/Binaries/Win64"` → metadata ignored (deep subfolder)
- Path `"/games/MyGame/Data/Plugins/x86_64"` → metadata ignored (plugins folder)

### 10. `test_company_name_extraction()`
Tests company name as last resort (Level 5):

**Valid company name:**
- `"Valve"` → accepted when product name is generic

**Generic company name rejected:**
- `"Microsoft Corporation"` → rejected

**None handling:**
- `None` → `None`

### 11. `test_find_title_in_parents()`
Tests parent directory traversal with generic name skipping:

**Immediate parent:**
- `"/games/MyGame/Data"` → `"MyGame"`

**Skip generic parent:**
- `"/games/MyCollection/Windows/Data"` → skips `"Windows"`, finds `"MyCollection"`
- `"/games/Collection/Build/Windows/Data"` → skips `"Build"` and `"Windows"`, finds `"Collection"`

**All generic → None:**
- `"/games/Windows/Build/Release/Data"` → all generic, returns `None`

## Test Statistics
- **Total test functions:** 11
- **Total test cases:** 100+
- **Coverage:** All critical title extraction paths, including edge cases from real logs
- **Real log examples used:** 15+ specific cases

## Key Issues Validated
1. ✅ Generic folder names (jre, en-us, Build, Engine, etc.) now correctly fallback to parent
2. ✅ Metadata sentences are properly filtered out
3. ✅ Utility executables (javaws.exe, etc.) are filtered by generic name detection
4. ✅ Deep subfolder EXE metadata is ignored (Engine/Binaries, Plugins)
5. ✅ Trailing parenthesis truncation is handled
6. ✅ Version numbers are stripped from folder names
7. ✅ Multi-level parent fallback works correctly

## Running Tests
```bash
cd src-tauri
cargo test title_extraction::tests -- --nocapture
```

All tests should pass with the fixes implemented.
