# План миграции: Множественные каталоги на Space

## Цель
Allow each Space to have multiple source directories (watch folders) for automatic game scanning.

## Архитектурное решение

### Новая таблица: `space_sources`
```sql
CREATE TABLE space_sources (
    space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
    source_path TEXT NOT NULL,
    is_active INTEGER DEFAULT 1,
    scan_recursively INTEGER DEFAULT 1,
    last_scanned_at TEXT,
    exclude_patterns TEXT, -- JSON array
    PRIMARY KEY (space_id, source_path)
);
```

### Миграция существующих данных

1. **Существующие spaces**: поле `path` остается, но становится deprecated
2. **Перенос данных**: Для каждого space с не-NULL path создается запись в space_sources
3. **Совместимость**: Old code continues to work, new code uses space_sources

## Пошаговый план изменений

### Phase 1: Database Migration (Безопасно, обратная совместимость)
- [ ] 1.1 Создать новую таблицу `space_sources`
- [ ] 1.2 Перенести существующие `spaces.path` в `space_sources` (один источник на space)
- [ ] 1.3 Добавить миграцию в `database.rs` (выполняется один раз)
- [ ] 1.4 Обновить `Space` модель - добавить computed свойство или оставить path

### Phase 2: Rust Backend
- [ ] 2.1 Добавить `SpaceSource` struct в `models.rs`
- [ ] 2.2 Добавить CRUD методы для space_sources в `database.rs`
- [ ] 2.3 Обновить команды Tauri:
  - `create_space` → принимать опциональный `initial_sources: Vec<String>`
  - `get_space_with_sources` → новая команда для получения space + его источников
  - `add_space_source` / `remove_space_source` / `update_space_source`
- [ ] 2.4 Обновить `scan_directory` команду → принимать `space_id` и сканировать все его источники

### Phase 3: TypeScript Types & Hooks
- [ ] 3.1 Обновить `Space` interface → добавить `watch_directories: SpaceSource[]`
- [ ] 3.2 Создать `SpaceSource` interface
- [ ] 3.3 Обновить `useSpaces` hook → загружать источники вместе со spaces
- [ ] 3.4 Создать `useSpaceSources` hook для управления источниками

### Phase 4: Frontend Components
- [ ] 4.1 Обновить `AddSpaceDialog`:
  - Добавить multiselect/[]input для каталогов
  - Сохранять выбранные пути как space_sources
- [ ] 4.2 Создать `SpaceSettingsDialog` или расширить `AddSpaceDialog`:
  - Показывать список источников space
  - Возможность добавлять/удалять/переключать источники
  - Кнопка "Сканировать все источники"
- [ ] 4.3 Обновить `ScanDialog`:
  - По умолчанию сканировать все источники выбранного space
  - Возможность сканировать произвольную папку (вне источников)
- [ ] 4.4 Обновить `Sidebar`:
  - Показывать количество источников или иконку multi-folder
  - Tooltip со списком путей

### Phase 5: Scan Logic Enhancement
- [ ] 5.1 Обновить `commands.rs::scan_directory`:
  - Принимать `space_id: Option<String>` вместо одного path
  - Если space_id указан → получать все active источники и сканировать каждый
  - Возвращать объединенный список найденных игр
- [ ] 5.2 Добавить фоновое сканирование (опционально)
- [ ] 5.3 Добавить exclude patterns support (игнорировать временные папки)

### Phase 6: Testing & Polish
- [ ] 6.1 Тестирование миграции на существующих БД
- [ ] 6.2 Unit tests для space_sources CRUD
- [ ] 6.3 UI тесты: добавление space с несколькими папками
- [ ] 6.4 Тест сценария: Steam + локальные папки в одном space
- [ ] 6.5 Обновить локализацию (новые строки)

## Обратная совместимость

### Temporarily keep `spaces.path`
- Поле `path` в `spaces` останется для старых записей
- Новые spaces создаются с path=NULL
- Код проверяет: если space.path существует → используем его как единственный источник
- Если space.watch_directories непустой → используем их
- Migration gradually converts old spaces to new model

### Frontend graceful degradation
- Если у space нет источников в БД, показывать空或 предупреждение
- ScanDialog по-прежнему позволяет выбрать произвольную папку

## Скрипт миграции SQL

```sql
-- Создаем таблицу space_sources
CREATE TABLE IF NOT EXISTS space_sources (
    space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
    source_path TEXT NOT NULL,
    is_active INTEGER DEFAULT 1,
    scan_recursively INTEGER DEFAULT 1,
    last_scanned_at TEXT,
    exclude_patterns TEXT,
    PRIMARY KEY (space_id, source_path)
);

-- Мигрируем существующие paths
INSERT OR IGNORE INTO space_sources (space_id, source_path)
SELECT id, path FROM spaces 
WHERE path IS NOT NULL AND path != '';

-- Индексы
CREATE INDEX IF NOT EXISTS idx_space_sources_space ON space_sources(space_id);
CREATE INDEX IF NOT EXISTS idx_space_sources_path ON space_sources(source_path);
```

## Изменения кода (деталенно)

### 1. models.rs (Rust)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSource {
    pub space_id: String,
    pub source_path: String,
    pub is_active: bool,
    pub scan_recursively: bool,
    pub last_scanned_at: Option<String>,
    pub exclude_patterns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    // existing fields...
    #[serde(skip)]
    pub watch_directories: Option<Vec<SpaceSource>>, // loaded on demand
}
```

### 2. database.rs
```rust
impl Database {
    // CRUD for space_sources
    pub fn get_space_sources(&self, space_id: &str) -> Result<Vec<SpaceSource>> { ... }
    pub fn add_space_source(&self, space_id: &str, path: &str) -> Result<()> { ... }
    pub fn remove_space_source(&self, space_id: &str, path: &str) -> Result<()> { ... }
    pub fn update_space_source(&self, space_id: &str, path: &str, is_active: bool) -> Result<()> { ... }
    
    // Migration
    fn migrate_space_sources(&self) -> Result<()> {
        // Create table
        // Insert from spaces.path
        Ok(())
    }
}
```

### 3. commands.rs
```rust
#[tauri::command]
pub fn get_space_with_sources(state: State<AppState>, space_id: String) -> Result<SpaceWithSources, String> {
    // Return space + array of sources
}

#[tauri::command]
pub fn add_space_source(state: State<AppState>, space_id: String, path: String) -> Result<(), String> { ... }

#[tauri::command]
pub fn remove_space_source(state: State<AppState>, space_id: String, path: String) -> Result<(), String> { ... }

#[tauri::command]
pub async fn scan_space_sources(state: State<'_, AppState>, space_id: String) -> Result<Vec<ScannedGame>, String> {
    // Get all active sources for space
    // Scan each directory
    // Return combined results
}
```

### 4. TypeScript types
```typescript
export interface SpaceSource {
  space_id: string;
  source_path: string;
  is_active: boolean;
  scan_recursively: boolean;
  last_scanned_at?: string;
  exclude_patterns?: string[];
}

export interface Space {
  // existing...
  watch_directories?: SpaceSource[];
}
```

### 5. AddSpaceDialog.tsx changes
- После создания space → автоматически вызывать `add_space_source` для каждого выбранного каталога
- Multiple folder selection using `<input type="file" webkitdirectory directory multiple />`

### 6. New component: SpaceSourcesManager
```tsx
interface SpaceSourcesManagerProps {
  spaceId: string;
  sources: SpaceSource[];
  onUpdate: () => void;
}

// Shows list of sources with:
// - Add folder button
// - Remove button for each
// - Toggle active/inactive
// - Scan button for individual source
```

## Неопределенности и вопросы

1. **Следует ли удалить поле `spaces.path` полностью?**
   - Пока оставить для совместимости
   - В будущем можно сделать deprecated

2. **Может ли один source_path принадлежать нескольким spaces?**
   - Нет, PRIMARY KEY (space_id, source_path) предотвращает дубли
   - Одна папка может быть источником только для одного space

3. **Сканирование пересекающихся каталогов?**
   - Пользователь ответственен за не-overlapping источники
   - Duplicate detection сработает одинаково (fingerprint)

4. **Exclude patterns:**
   - Простой JSON array: `["*.tmp", "saves", "__MACOSX"]`
   - Apply during scan в `commands.rs`

## Сроки
- Phase 1-2: 1-2 дня
- Phase 3: 1 день
- Phase 4: 2-3 дня
- Phase 5: 1 день
- Phase 6: 1 день

**Итого**: ~1 неделя