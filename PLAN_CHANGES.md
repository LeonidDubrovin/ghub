# План изменений для GHub

## Статус: В процессе разработки, требуется финализация

---

## Уже реализовано (рабочие изменения)

### 1. Backup Database Command
- ✅ Добавлен `src-tauri/src/commands/backup.rs`
- ✅ Использует `VACUUM INTO` для создания консистентного бэкапа
- ✅ Зарегистрирован в `lib.rs` invoke_handler
- ✅ Бэкап создаётся в папке `backups/` с timestamp

### 2. .bat File Support
- ✅ Modified `src-tauri/src/scanning_service.rs`:
  - `has_executable_files()` now includes `.bat`
  - `has_exe_files()` now includes `.bat`
- ✅ Это расширяет обнаружение игр, которые используют .bat запуск

### 3. UI Improvement for SourceItem
- ✅ `src/components/SourceItem.tsx`:
  - Показывает только имя папки (leaf) вместо полного пути
  - Добавлена иконка папки
  - Улучшена читаемость списка источников

### 4. Database Schema Fix
- ✅ Удалены дублирующие ALTER TABLE для `scan_status` колонок
- ✅ Колонки теперь добавляются напрямую в CREATE TABLE
- ✅ Исправлены запросы SELECT для включения новых колонок

### 5. AppState Enhancement
- ✅ Добавлено поле `db_path` в `AppState` для доступа к пути БД в командах

---

## Проблемные изменения (временно удалены)

### 1. Start Menu Scanning (Windows)
- ❌ Причина: проблемы с импортом `lnk` crate, type inference errors
- ❌ Файл: `src-tauri/src/commands/scanning.rs`
- ❌ Удалён: функция `scan_start_menu_shortcuts()` и её вызов
- 🔄 Возобновление: после настройки dependencies или тестирования на Windows

### 2. Scan Result Caching
- ❌ Причина: сложность с ленивой статикой и type annotations в контексте
- ❌ Удалено: `SCAN_CACHE`, `clear_scan_cache()`, соответствующая логика
- 🔄 Возобновление: переработка с более простым подходом (например, хранение в AppState)

---

## Ошибки компиляции и их решения

| Ошибка | Причина | Решение |
|--------|---------|---------|
| `could not find Lnk in lnk` | Неправильный импорт/API `lnk` crate | Удалён Start Menu scanning |
| `type annotations needed` | Неявные типы в замыканиях | Явно аннотированы типы или удалён проблемный код |
| `SCAN_CACHE not found` | Ленивая статика требует внешнего crate, правильно не импортирована | Удалено |
| `duplicate column name: scan_status` | ALTER TABLE добавлял колонки, уже созданные в CREATE TABLE | Удалены ALTER TABLE statements |

---

## Задачи на будущее (приоритетные)

### P0 – Критический приоритет
1. **Сборка и запуск приложения без ошибок**
   - Сейчас: компилируется с предупреждениями, но без фатальных ошибок
   - Нужно: убедиться, что приложение запускается и создаёт БД

2. **Тестирование backup команды**
   - Проверить, что команда `backup_database` создаёт файл бэкапа
   - Добавить кнопку в UI (если нужно) или вызывать через dev tools

3. **Тестирование .bat поддержки**
   - Найти тестовые .bat файлы/игры
   - Убедиться, что они обнаруживаются и запускаются корректно

### P1 – Высокий приоритет
4. **Возврат Start Menu Scanning**
   - Требует тестирования `lnk` crate на Windows
   - Возможное решение: использовать `shell_link` или `windows-shortcut` вместо `lnk`
   - Или сделать опциональную фичу через `cfg` и Cargo feature

5. **Возврат кэширования сканирования**
   - Упрощённый подход: кэш в памяти в `ScanningService` (Mutex<HashMap>)
   - Ключ: путь, значение: (modified_time, games)
   - Очистка при изменении файлов или по TTL

### P2 – Средний приоритет
6. **Полнотекстовый поиск FTS5**
   - Создание FTS таблицы и триггеров
   - Интеграция в поисковый хук

7. **Steam Library Scanning**
   - Чтение `steamapps/libraryfolders.vdf`
   - Обход manifest-файлов

8. **Metadata Scraping (IGDB/SteamGridDB)**
   - API ключи, запросы, кэширование

---

## Изменения в файловой структуре

```
added:
  src-tauri/src/commands/backup.rs
  backups/ghub_initial.db (created manually)
  PLAN_CHANGES.md (этот файл)

modified:
  src-tauri/src/scanning_service.rs (.bat support)
  src-tauri/src/commands/scanning.rs (removed start menu & caching)
  src-tauri/src/database.rs (schema fixes)
  src-tauri/src/lib.rs (added backup command, db_path)
  src-tauri/src/commands/mod.rs (re-export backup)
  src/components/SourceItem.tsx (UI improvement)
```

---

## Рекомендации по дальнейшим действиям

1. **Зафиксировать текущее рабочее состояние** (commits):
   - "feat: backup database command"
   - "feat: .bat file support in scanning"
   - "fix: database schema duplication"
   - "feat(ui): improve source item display"

2. **Создать release** для тестирования:
   - Собрать .exe
   - Запустить, проверить базовый функционал

3. **Планировать следующие итерации**:
   - Сначала стабильный запуск и бэкапы
   - Потом Start Menu scanning (Windows only)
   - Потом кэширование сканирования
   - Потом FTS5, Steam, metadata

---

## Технические детали

### Backup Command Implementation
- Использует `VACUUM INTO 'path'` SQLite команду
- Создаёт папку `backups` в `app_data_dir`
- Имя файла: `ghub_<timestamp>.db`

### .bat Support
- В `has_executable_files`: расширение `bat` проверяется наравне с `exe`, `lnk`
- В `has_exe_files`: case-insensitive проверка `bat`

### Database Schema
- `space_sources` table теперь включает колонки:
  ```sql
  scan_status TEXT,
  scan_progress INTEGER DEFAULT 0,
  scan_total INTEGER DEFAULT 0,
  scan_error TEXT,
  scan_started_at TEXT,
  scan_completed_at TEXT
  ```

---

## Критические вопросы

1. **Нужно ли интегрировать backup command в UI?**
   - Можно добавить кнопку "Создать бэкап" в настройках
   - Или делать автоматический бэкап при закрытии

2. **Должно ли сканирование меню Пуск быть опциональным?**
   - Да, можно добавить настройку "Scan Start Menu" (вкл/выкл)
   - Или выполнять только при первом запуске

3. **Как тестировать на different OS?**
   - .bat: только Windows, но код кросс-платформенный (ignored на others)
   - Start Menu: только Windows

---

## Следующие шаги (немедленно)

1. ✅ Завершить текущие исправления (уже сделано)
2. ✅ Проверить компиляцию (`cargo check` – PASS)
3. 🔄 Собрать и запустить приложение (`npm run tauri dev`)
4. 🔄 Создать бэкап через UI или командную строку
5. 🔄 Закоммитить изменения
6. 🔄 Подготовить следующий план (кэширование, UI улучшения)

---

*Последнее обновление: 2026-03-21*