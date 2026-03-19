# Приоритетный план улучшений локального сканирования

## Резюме

На основе сравнительного анализа с Playnite и itch, составлен приоритетный план улучшений локального сканирования игр. План сфокусирован на максимальной выгоде при минимальных затратах.

---

## Фаза 1: Критические улучшения (1-2 недели)

### 1.1 Сканирование Start Menu ⭐⭐⭐⭐⭐

**Приоритет:** КРИТИЧЕСКИЙ

**Проблема:** ~80% установленных игр имеют ярлыки в Start Menu, которые мы не сканируем.

**Решение:**
```rust
// Добавить новую функцию
fn scan_start_menu_shortcuts() -> Result<Vec<ScannedGame>, String> {
    let start_menu_paths = vec![
        // All Users
        PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs"),
        // Current User
        dirs::data_dir()
            .ok_or("Failed to get data dir")?
            .join("Microsoft\\Windows\\Start Menu\\Programs"),
    ];
    
    let mut games = Vec::new();
    
    for base_path in start_menu_paths {
        if !base_path.exists() {
            continue;
        }
        
        // Рекурсивно сканировать .lnk файлы
        for entry in WalkDir::new(&base_path)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("lnk") {
                        if let Some(game) = parse_lnk_file(path) {
                            games.push(game);
                        }
                    }
                }
            }
        }
    }
    
    Ok(games)
}

// Интегрировать в scan_space_sources
pub fn scan_space_sources(state: State<AppState>, space_id: String) -> Result<Vec<ScannedGame>, String> {
    // ... существующий код ...
    
    // Добавить сканирование Start Menu
    if cfg!(target_os = "windows") {
        match scan_start_menu_shortcuts() {
            Ok(mut start_menu_games) => {
                println!("   📌 Found {} games in Start Menu", start_menu_games.len());
                all_games.append(&mut start_menu_games);
            }
            Err(e) => {
                println!("   ⚠️ Failed to scan Start Menu: {}", e);
            }
        }
    }
    
    // ... остальной код ...
}
```

**Выгода:**
- Покрытие ~80% установленных игр
- Более надежные названия (из ярлыков)
- Минимальные затраты на реализацию

**Файлы для изменения:**
- `src-tauri/src/commands/scanning.rs`

---

### 1.2 Кэширование результатов сканирования ⭐⭐⭐⭐⭐

**Приоритет:** КРИТИЧЕСКИЙ

**Проблема:** Каждое сканирование заново обходит все директории (медленно для больших библиотек).

**Решение:**
```rust
use std::collections::HashMap;
use std::time::SystemTime;

#[derive(Clone)]
struct ScanCache {
    path: PathBuf,
    last_modified: SystemTime,
    games: Vec<ScannedGame>,
}

lazy_static! {
    static ref SCAN_CACHE: std::sync::Mutex<HashMap<String, ScanCache>> = 
        std::sync::Mutex::new(HashMap::new());
}

fn get_directory_modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

fn scan_directory_with_cache(base_path: &Path) -> Result<Vec<ScannedGame>, String> {
    let path_str = base_path.to_string_lossy().to_string();
    
    // Проверить кэш
    {
        let cache = SCAN_CACHE.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = cache.get(&path_str) {
            if let Ok(current_modified) = get_directory_modified_time(base_path) {
                if cached.last_modified >= current_modified {
                    println!("   💾 Using cached results for {}", path_str);
                    return Ok(cached.games.clone());
                }
            }
        }
    }
    
    // Сканировать заново
    println!("   🔍 Scanning directory: {}", path_str);
    let games = scan_directory_internal(base_path)?;
    
    // Обновить кэш
    {
        let mut cache = SCAN_CACHE.lock().map_err(|e| e.to_string())?;
        cache.insert(path_str.clone(), ScanCache {
            path: base_path.to_path_buf(),
            last_modified: get_directory_modified_time(base_path)
                .unwrap_or(SystemTime::UNIX_EPOCH),
            games: games.clone(),
        });
    }
    
    Ok(games)
}

// Добавить команду для очистки кэша
#[tauri::command]
pub fn clear_scan_cache() -> Result<(), String> {
    let mut cache = SCAN_CACHE.lock().map_err(|e| e.to_string())?;
    cache.clear();
    println!("   🗑️ Scan cache cleared");
    Ok(())
}
```

**Выгода:**
- Ускорение повторных сканирований в 10-100 раз
- Снижение нагрузки на диск
- Улучшение UX (быстрые обновления)

**Файлы для изменения:**
- `src-tauri/src/commands/scanning.rs`
- `src-tauri/src/commands/mod.rs` (добавить clear_scan_cache)

---

### 1.3 Поддержка .bat файлов ⭐⭐⭐

**Приоритет:** ВЫСОКИЙ

**Проблема:** Некоторые игры (особенно старые и инди) используют .bat файлы для запуска.

**Решение:**
```rust
// Изменить константу
const EXECUTABLE_EXTENSIONS: &[&str] = &["exe", "lnk", "bat"];

// Обновить функцию has_executable_files
fn has_executable_files(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|entry| {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_str().unwrap_or("").to_lowercase();
                        return EXECUTABLE_EXTENSIONS.contains(&ext_str.as_str());
                    }
                }
                false
            })
        })
        .unwrap_or(false)
}

// Обновить функцию find_all_executables
fn find_all_executables(dir: &Path) -> Vec<String> {
    let mut executables = Vec::new();

    for entry in WalkDir::new(dir).max_depth(4).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_str().unwrap_or("").to_lowercase();
                
                if EXECUTABLE_EXTENSIONS.contains(&ext_str.as_str()) {
                    // ... существующая логика ...
                }
            }
        }
    }

    executables
}
```

**Выгода:**
- Поддержка старых игр
- Поддержка инди-проектов
- Минимальные затраты

**Файлы для изменения:**
- `src-tauri/src/commands/scanning.rs`

---

## Фаза 2: Расширение покрытия (2-3 недели)

### 2.1 UWP игры (Windows) ⭐⭐⭐⭐

**Приоритет:** ВЫСОКИЙ

**Проблема:** Windows Store игры не обнаруживаются.

**Решение:**
```rust
#[cfg(target_os = "windows")]
fn scan_uwp_games() -> Result<Vec<ScannedGame>, String> {
    use std::process::Command;
    
    let mut games = Vec::new();
    
    // Использовать PowerShell для получения списка UWP приложений
    let output = Command::new("powershell")
        .args(&[
            "-Command",
            "Get-AppxPackage | Where-Object {$_.PackageFamilyName -like '*Game*'} | Select-Object Name, PackageFamilyName, InstallLocation"
        ])
        .output()
        .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Парсить вывод PowerShell
        for line in stdout.lines() {
            if line.trim().is_empty() || line.starts_with("Name") {
                continue;
            }
            
            // ... парсинг и создание ScannedGame ...
        }
    }
    
    Ok(games)
}
```

**Выгода:**
- Поддержка современных Windows игр
- Расширение покрытия библиотеки

**Файлы для изменения:**
- `src-tauri/src/commands/scanning.rs`

---

### 2.2 Поддержка архивов ⭐⭐⭐

**Приоритет:** СРЕДНИЙ

**Проблема:** Портативные игры в архивах не обнаруживаются.

**Решение:**
```rust
// Добавить зависимость в Cargo.toml
// zip = "0.6"
// unrar = "0.5"
// sevenz-rust = "0.5"

fn scan_archive(archive_path: &Path) -> Option<ScannedGame> {
    let ext = archive_path.extension()?.to_str()?.to_lowercase();
    
    match ext.as_str() {
        "zip" => scan_zip_archive(archive_path),
        "rar" => scan_rar_archive(archive_path),
        "7z" => scan_7z_archive(archive_path),
        _ => None,
    }
}

fn scan_zip_archive(archive_path: &Path) -> Option<ScannedGame> {
    use std::fs::File;
    use zip::ZipArchive;
    
    let file = File::open(archive_path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    
    // Искать .exe файлы в архиве
    for i in 0..archive.len() {
        let file = archive.by_index(i).ok()?;
        let name = file.name();
        
        if name.ends_with(".exe") && !is_exe_excluded(name) {
            // Создать ScannedGame для архива
            return Some(ScannedGame {
                path: archive_path.to_string_lossy().to_string(),
                title: extract_title_from_path(archive_path),
                executable: Some(name.to_string()),
                // ... остальные поля ...
            });
        }
    }
    
    None
}
```

**Выгода:**
- Поддержка портативных игр
- Расширение покрытия библиотеки

**Файлы для изменения:**
- `src-tauri/Cargo.toml`
- `src-tauri/src/commands/scanning.rs`

---

## Фаза 3: Продвинутые функции (3-4 недели)

### 3.1 CRC-идентификация ⭐⭐

**Приоритет:** СРЕДНИЙ

**Проблема:** Невозможно точно идентифицировать игру по содержимому.

**Решение:**
```rust
fn calculate_crc32(file_path: &Path) -> Option<u32> {
    use std::fs::File;
    use std::io::Read;
    
    let mut file = File::open(file_path).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    
    // Использовать crc32fast для вычисления
    Some(crc32fast::hash(&buffer))
}

fn identify_game_by_crc(crc: u32) -> Option<GameInfo> {
    // Поиск в локальной базе данных игр
    // Или запрос к внешнему API
    None
}
```

**Выгода:**
- Точная идентификация игр
- Возможность автоматического заполнения метаданных

**Файлы для изменения:**
- `src-tauri/Cargo.toml`
- `src-tauri/src/commands/scanning.rs`
- `src-tauri/src/database.rs` (добавить таблицу для CRC)

---

### 3.2 Поддержка плейлистов ⭐⭐

**Приоритет:** НИЗКИЙ

**Проблема:** Многодисковые игры не обрабатываются правильно.

**Решение:**
```rust
fn parse_cue_file(cue_path: &Path) -> Option<Vec<String>> {
    // Парсинг .cue файлов
    // Возврат списка путей к .bin файлам
}

fn parse_m3u_file(m3u_path: &Path) -> Option<Vec<String>> {
    // Парсинг .m3u файлов
    // Возврат списка путей к медиа файлам
}
```

**Выгода:**
- Поддержка многодисковых игр
- Улучшение обработки эмулируемых игр

**Файлы для изменения:**
- `src-tauri/src/commands/scanning.rs`

---

## Фаза 4: Оптимизация (1-2 недели)

### 4.1 Параллельная обработка ⭐⭐⭐

**Приоритет:** СРЕДНИЙ

**Проблема:** Сканирование больших директорий выполняется последовательно.

**Решение:**
```rust
use rayon::prelude::*;

fn scan_directory_parallel(base_path: &Path) -> Result<Vec<ScannedGame>, String> {
    let entries: Vec<_> = WalkDir::new(base_path)
        .max_depth(MAX_SCAN_DEPTH)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    
    let games: Vec<_> = entries
        .par_iter()
        .filter_map(|entry| {
            scan_single_directory(entry.path()).ok()
        })
        .flatten()
        .collect();
    
    Ok(games)
}
```

**Выгода:**
- Ускорение сканирования в N раз (где N - количество ядер CPU)
- Улучшение отзывчивости UI

**Файлы для изменения:**
- `src-tauri/Cargo.toml`
- `src-tauri/src/commands/scanning.rs`

---

### 4.2 Улучшение логирования ⭐⭐

**Приоритет:** НИЗКИЙ

**Проблема:** Логирование недостаточно детальное для отладки.

**Решение:**
```rust
use log::{debug, info, warn, error};

fn scan_directory_internal(base_path: &Path) -> Result<Vec<ScannedGame>, String> {
    info!("Starting scan of directory: {}", base_path.display());
    
    // ... существующий код ...
    
    debug!("Found {} executables in {}", executables.len(), dir.display());
    info!("Scan completed: {} games found", games.len());
    
    Ok(games)
}
```

**Выгода:**
- Улучшение отладки
- Мониторинг производительности
- Анализ проблемных случаев

**Файлы для изменения:**
- `src-tauri/Cargo.toml`
- `src-tauri/src/commands/scanning.rs`

---

## Сводная таблица приоритетов

| Улучшение | Приоритет | Сложность | Выгода | Время |
|-----------|-----------|-----------|--------|-------|
| Start Menu сканирование | КРИТИЧЕСКИЙ | Средняя | Очень высокая | 3-5 дней |
| Кэширование результатов | КРИТИЧЕСКИЙ | Средняя | Очень высокая | 2-3 дня |
| Поддержка .bat файлов | ВЫСОКИЙ | Низкая | Средняя | 1 день |
| UWP игры | ВЫСОКИЙ | Высокая | Высокая | 5-7 дней |
| Поддержка архивов | СРЕДНИЙ | Средняя | Средняя | 3-5 дней |
| CRC-идентификация | СРЕДНИЙ | Высокая | Средняя | 5-7 дней |
| Параллельная обработка | СРЕДНИЙ | Средняя | Высокая | 2-3 дня |
| Поддержка плейлистов | НИЗКИЙ | Низкая | Низкая | 1-2 дня |
| Улучшение логирования | НИЗКИЙ | Низкая | Низкая | 1 день |

---

## Рекомендуемый порядок реализации

### Неделя 1: Критические улучшения
1. **День 1-2:** Кэширование результатов сканирования
2. **День 3-5:** Сканирование Start Menu

### Неделя 2: Быстрые победы
3. **День 1:** Поддержка .bat файлов
4. **День 2-3:** Улучшение логирования
5. **День 4-5:** Тестирование и отладка

### Неделя 3-4: Расширение покрытия
6. **День 1-5:** UWP игры (Windows)
7. **День 6-10:** Поддержка архивов

### Неделя 5-6: Продвинутые функции
8. **День 1-5:** CRC-идентификация
9. **День 6-10:** Параллельная обработка

---

## Критерии успеха

### Фаза 1
- [ ] Start Menu сканирование обнаруживает >80% установленных игр
- [ ] Кэширование ускоряет повторные сканирования в >10 раз
- [ ] .bat файлы корректно обрабатываются

### Фаза 2
- [ ] UWP игры обнаруживаются на Windows 10/11
- [ ] Архивы .zip, .rar, .7z поддерживаются

### Фаза 3
- [ ] CRC-идентификация работает для >50% игр
- [ ] Параллельная обработка ускоряет сканирование в >2 раза

### Фаза 4
- [ ] Все улучшения протестированы и стабильны
- [ ] Документация обновлена
- [ ] Производительность соответствует ожиданиям

---

## Заключение

Реализация этого плана позволит достичь уровня Playnite по покрытию установленных игр, сохранив наши преимущества в гибкости и производительности. Критические улучшения (Start Menu и кэширование) должны быть реализованы в первую очередь, так как они дают максимальную выгоду при минимальных затратах.
