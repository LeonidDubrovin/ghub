# Code Review: scanning.rs

## Резюме

Проведен детальный анализ кода [`src-tauri/src/commands/scanning.rs`](src-tauri/src/commands/scanning.rs). Выявлены потенциальные баги, проблемы производительности и возможности для рефакторинга.

---

## 1. Потенциальные баги

### 1.1 Проблема с `find_folder_with_exe` (строки 404-456)

**Проблема:** Функция проверяет `max_depth == 0` в начале, но затем рекурсивно вызывает себя с `max_depth - 1`. Это может привести к тому, что при `max_depth = 1` функция проверит поддиректории, но не пойдет глубже.

```rust
fn find_folder_with_exe(dir: &Path, max_depth: u32) -> Option<PathBuf> {
    if max_depth == 0 {
        return None;  // ← Возвращает None если depth = 0
    }
    
    // ... проверка поддиректорий ...
    
    // Рекурсивный вызов с depth - 1
    if let Some(found) = find_folder_with_exe(&subdir, max_depth - 1) {
        return Some(found);
    }
}
```

**Влияние:** При вызове с `max_depth = 2`, функция проверит:
- Уровень 1: прямые поддиректории (depth = 2)
- Уровень 2: поддиректории поддиректорий (depth = 1)
- Уровень 3: не проверит (depth = 0 → return None)

**Рекомендация:** Изменить условие на `if max_depth == 0 { return None; }` в начале рекурсивной проверки, а не в начале функции.

---

### 1.2 Проблема с дедупликацией путей (строки 115-117)

**Проблема:** Дедупликация использует сравнение строк путей, но пути могут иметь разное представление для одной и той же директории:

```rust
all_games.sort_by(|a, b| a.path.cmp(&b.path));
all_games.dedup_by(|a, b| a.path == b.path);
```

**Пример проблемы:**
- `C:\Games\MyGame`
- `C:\Games\MyGame\`
- `C:\Games\MyGame\.`

Все эти пути указывают на одну директорию, но будут считаться разными.

**Рекомендация:** Нормализовать пути перед сравнением:

```rust
fn normalize_path(path: &str) -> String {
    let p = Path::new(path);
    p.canonicalize()
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .to_string()
}

// Использование:
all_games.sort_by(|a, b| normalize_path(&a.path).cmp(&normalize_path(&b.path)));
all_games.dedup_by(|a, b| normalize_path(&a.path) == normalize_path(&b.path));
```

---

### 1.3 Проблема с `is_generic_exe_name` (строки 786-825)

**Проблема:** Функция проверяет точное совпадение с generic names, но некоторые игры могут содержать эти слова как часть названия:

```rust
let generic_names = [
    "Godot Engine", "BootstrapPackagedGame", "Unity", "Unreal Engine",
    // ...
];

let name_lower = name.to_lowercase();
for generic in &generic_names {
    if name_lower == generic.to_lowercase() {  // ← Точное совпадение
        return true;
    }
}
```

**Пример проблемы:**
- Игра "Unity of Command" будет отфильтрована как generic
- Игра "Unreal Tournament" будет отфильтрована как generic

**Рекомендация:** Использовать более умную проверку:

```rust
fn is_generic_exe_name(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    
    // Точное совпадение с полностью generic names
    let exact_generic = [
        "godot engine", "bootstrappackagedgame", "unity", "unreal engine",
        "unrealengine", "ue4", "ue5", "ue4game",
        // ...
    ];
    
    for generic in &exact_generic {
        if name_lower == *generic {
            return true;
        }
    }
    
    // Проверка на version-only names
    if name.chars().all(|c| c.is_numeric() || c == '.' || c == '_' || c == '-' || c == 'v' || c == 'V') {
        return true;
    }
    
    // Проверка на non-game words (точное совпадение)
    let non_game_words = ["test", "demo", "sample", "example", "tutorial", "template"];
    for word in &non_game_words {
        if name_lower == *word {
            return true;
        }
    }
    
    false
}
```

---

## 2. Проблемы производительности

### 2.1 Множественные вызовы `to_string_lossy()` (строки 269-273)

**Проблема:** Код多次 вызывает `to_string_lossy()` для одного пути:

```rust
let exe_in_deep_subfolder = game_path.to_string_lossy().contains("Engine\\Binaries") || 
                            game_path.to_string_lossy().contains("Engine/Binaries") ||
                            game_path.to_string_lossy().contains("Plugins") ||
                            game_path.to_string_lossy().contains("Binaries\\Win64") ||
                            game_path.to_string_lossy().contains("Binaries/Win64");
```

**Рекомендация:** Сохранить результат в переменную:

```rust
let path_str = game_path.to_string_lossy();
let exe_in_deep_subfolder = path_str.contains("Engine\\Binaries") || 
                            path_str.contains("Engine/Binaries") ||
                            path_str.contains("Plugins") ||
                            path_str.contains("Binaries\\Win64") ||
                            path_str.contains("Binaries/Win64");
```

---

### 2.2 Неэффективный поиск обложек (строки 547-620)

**Проблема:** Функция `find_cover_candidates` ищет в 17 предопределенных путях, даже если они не существуют:

```rust
let search_paths = [
    dir.to_path_buf(),
    dir.join("images"),
    dir.join("image"),
    // ... 14 других путей
];

for search_path in &search_paths {
    if !search_path.exists() {  // ← Проверка существования для каждого пути
        continue;
    }
    // ...
}
```

**Рекомендация:** Использовать более эффективный подход:

```rust
fn find_cover_candidates(dir: &Path) -> Vec<String> {
    let mut candidates = Vec::new();
    
    // Common subdirectory names for images
    let image_dirs = ["images", "image", "img", "art", "assets", "media", 
                      "resources", "gfx", "graphics", "covers", "cover", 
                      "box", "boxart", "screenshots", "screenshot", "promo"];
    
    // Search in root directory first
    search_images_in_dir(dir, &mut candidates);
    
    // Search in common subdirectories
    for image_dir in &image_dirs {
        let subdir = dir.join(image_dir);
        if subdir.exists() {
            search_images_in_dir(&subdir, &mut candidates);
        }
    }
    
    // Remove duplicates and limit
    candidates.sort();
    candidates.dedup();
    candidates.truncate(15);
    candidates
}

fn search_images_in_dir(dir: &Path, candidates: &mut Vec<String>) {
    for entry in WalkDir::new(dir).max_depth(3).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        
        if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        
        let relative = path.strip_prefix(dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        
        candidates.push(relative);
    }
}
```

---

### 2.3 Дублирование кода в парсерах метаданных (строки 994-1400)

**Проблема:** Функции `read_json_metadata`, `read_yaml_metadata`, `read_toml_metadata`, `read_xml_metadata`, `read_ini_metadata` имеют大量重复 кода для извлечения общих полей.

**Рекомендация:** Создать общую функцию для извлечения полей:

```rust
fn extract_metadata_fields(content: &str, parser: impl Fn(&str) -> Option<HashMap<String, String>>) -> Option<LocalMetadata> {
    let fields = parser(content)?;
    
    let mut metadata = LocalMetadata {
        name: None,
        description: None,
        developer: None,
        publisher: None,
        version: None,
        release_date: None,
    };
    
    // Extract name
    metadata.name = fields.get("name")
        .or_else(|| fields.get("title"))
        .or_else(|| fields.get("game_name"))
        .cloned();
    
    // Extract description
    metadata.description = fields.get("description")
        .or_else(|| fields.get("desc"))
        .or_else(|| fields.get("about"))
        .cloned();
    
    // Extract developer
    metadata.developer = fields.get("developer")
        .or_else(|| fields.get("dev"))
        .or_else(|| fields.get("author"))
        .cloned();
    
    // Extract publisher
    metadata.publisher = fields.get("publisher").cloned();
    
    // Extract version
    metadata.version = fields.get("version")
        .or_else(|| fields.get("ver"))
        .cloned();
    
    // Extract release_date
    metadata.release_date = fields.get("release_date")
        .or_else(|| fields.get("releasedate"))
        .or_else(|| fields.get("date"))
        .cloned();
    
    if metadata.name.is_some() || metadata.description.is_some() {
        Some(metadata)
    } else {
        None
    }
}
```

---

## 3. Возможности для рефакторинга

### 3.1 Извлечение логики определения названия игры (строки 236-321)

**Проблема:** Логика определения названия игры занимает ~85 строк и имеет 7 уровней fallback. Это сложно для понимания и поддержки.

**Рекомендация:** Извлечь в отдельную функцию:

```rust
fn extract_game_title(
    game_path: &Path,
    dir_name: &str,
    local_metadata: &Option<LocalMetadata>,
    exe_metadata: &Option<ExeMetadata>,
    executable: &Option<String>,
) -> String {
    // Level 0: Local metadata
    if let Some(title) = try_extract_from_local_metadata(local_metadata) {
        return title;
    }
    
    // Level 1: Cleaned directory name
    if let Some(title) = try_extract_from_dir_name(dir_name) {
        return title;
    }
    
    // Level 2: EXE metadata (if not in deep subfolder)
    if let Some(title) = try_extract_from_exe_metadata(game_path, exe_metadata) {
        return title;
    }
    
    // Level 3: Parent directory
    if let Some(title) = try_extract_from_parent_dir(game_path, 3) {
        return title;
    }
    
    // Level 4: Executable name
    if let Some(title) = try_extract_from_executable(executable) {
        return title;
    }
    
    // Level 5: Company name
    if let Some(title) = try_extract_from_company_name(exe_metadata) {
        return title;
    }
    
    // Fallback
    if dir_name != "Unknown" {
        dir_name.to_string()
    } else {
        "Unknown Game".to_string()
    }
}

fn try_extract_from_local_metadata(metadata: &Option<LocalMetadata>) -> Option<String> {
    metadata.as_ref()
        .and_then(|m| m.name.as_ref())
        .filter(|name| !name.is_empty() && !is_generic_exe_name(name) && !is_problematic_game_name(name))
        .cloned()
}

// ... другие helper функции ...
```

---

### 3.2 Создание структуры для конфигурации сканирования

**Проблема:** Константы и настройки сканирования разбросаны по файлу:

```rust
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "ico", "bmp", "webp", "gif"];
const MAX_SCAN_DEPTH: usize = 5;
const METADATA_FILES: &[&str] = &[...];
```

**Рекомендация:** Создать структуру конфигурации:

```rust
struct ScanConfig {
    image_extensions: Vec<String>,
    max_scan_depth: usize,
    metadata_files: Vec<String>,
    exe_exclusion_patterns: Vec<Regex>,
    folder_exclusion_patterns: Vec<Regex>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            image_extensions: vec!["png".into(), "jpg".into(), "jpeg".into(), 
                                   "ico".into(), "bmp".into(), "webp".into(), "gif".into()],
            max_scan_depth: 5,
            metadata_files: vec![
                "game.json".into(), "info.json".into(), "metadata.json".into(),
                // ...
            ],
            exe_exclusion_patterns: vec![
                Regex::new(r"(?i)unins\d*").unwrap(),
                // ...
            ],
            folder_exclusion_patterns: vec![
                Regex::new(r"(?i)^(engine|redist|redistributables)$").unwrap(),
                // ...
            ],
        }
    }
}
```

---

### 3.3 Добавление логирования вместо println!

**Проблема:** Код использует `println!` для отладки, что не подходит для production:

```rust
println!("      [scan] Found game folder: {}", path.display());
println!("      [scan] Game folder resolved to: {}", game_path.display());
```

**Рекомендация:** Использовать crate `log`:

```rust
use log::{debug, info, warn, error};

// Вместо println!:
debug!("[scan] Found game folder: {}", path.display());
info!("[scan] Game folder resolved to: {}", game_path.display());
warn!("[scan] Skipping excluded folder: {}", path.display());
error!("[scan] Failed to read directory: {}", e);
```

---

## 4. Сводная таблица проблем

| Тип | Проблема | Серьезность | Сложность исправления |
|-----|----------|-------------|----------------------|
| Баг | `find_folder_with_exe` depth logic | Средняя | Низкая |
| Баг | Дедупликация путей | Средняя | Средняя |
| Баг | `is_generic_exe_name` false positives | Высокая | Средняя |
| Производительность | Множественные `to_string_lossy()` | Низкая | Низкая |
| Производительность | Неэффективный поиск обложек | Средняя | Средняя |
| Рефакторинг | Дублирование кода парсеров | Средняя | Высокая |
| Рефакторинг | Сложная логика названий | Высокая | Высокая |
| Рефакторинг | Отсутствие конфигурации | Низкая | Средняя |
| Рефакторинг | println! вместо log | Низкая | Низкая |

---

## 5. Рекомендуемый порядок исправлений

### Приоритет 1: Критические баги
1. Исправить `is_generic_exe_name` false positives
2. Исправить дедупликацию путей

### Приоритет 2: Рефакторинг
3. Извлечь логику определения названия игры
4. Устранить дублирование кода парсеров

### Приоритет 3: Оптимизация
5. Оптимизировать поиск обложек
6. Устранить множественные `to_string_lossy()`

### Приоритет 4: Улучшения
7. Добавить конфигурацию сканирования
8. Заменить println! на log

---

## 6. Заключение

Текущая реализация сканирования функциональна, но имеет несколько потенциальных багов и областей для улучшения. Наиболее критичными являются:

1. **`is_generic_exe_name` false positives** - может отфильтровать реальные игры
2. **Дедупликация путей** - может пропустить дубликаты

Рекомендуется исправить эти баги в первую очередь, а затем провести рефакторинг для улучшения поддерживаемости кода.
