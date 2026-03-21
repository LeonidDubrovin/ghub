# Простой план асинхронного сканирования

## Проблема
При добавлении директорий в пространство происходит синхронное сканирование, которое блокирует UI на несколько секунд/минут.

## Цель
Сделать сканирование асинхронным, с визуальным отображением прогресса на карточке пространства.

## Workflow

```
1. Пользователь создает пространство, выбирает директории → сохраняется в БД (быстро)
2. В интерфейсе на карточке пространства отображаются источники (директории) с кнопкой "Сканировать" для каждого
3. При нажатии "Сканировать" для источника:
   - Запускается фоновое сканирование ЭТОЙ директории
   - Статус источника меняется на "Сканирование..." с прогресс-баром
   - Найденные игры добавляются/обновляются в библиотеке автоматически
   - Исчезнувшие игры помечаются статусом missing
4. По завершении: статус "Готово" (или "Ошибка" с сообщением)
```

## Детали логики сканирования

При сканировании источника (директории):
1. Сканируем всю директорию и находим все игры
2. Для каждой найденной игры:
   - Проверяем, есть ли уже игра с таким `install_path` в этом пространстве
   - Если есть:
     * Вычисляем fingerprint (контрольную сумму исполняемого файла)
     * Сравниваем с сохраненным
     * Если fingerprint изменился → статус `modified`
     * Если совпадает → обновляем данные (если нужно), статус `installed`
   - Если игры нет → добавляем новую, статус `installed`
3. Для всех игр этого источника, которые были в БД (install_path совпадает), но не найдены в текущем сканировании → статус `missing`
4. Игры со статусом `missing` показываются в UI с иконкой ⚠️ и кнопкой "Найти заново"

## Архитектура

### 1. База данных

**Добавляем колонки в `space_sources` для отслеживания статуса сканирования на уровне источника:**
```sql
ALTER TABLE space_sources ADD COLUMN scan_status TEXT; -- idle, scanning, completed, error
ALTER TABLE space_sources ADD COLUMN scan_progress INTEGER DEFAULT 0;
ALTER TABLE space_sources ADD COLUMN scan_total INTEGER DEFAULT 0;
ALTER TABLE space_sources ADD COLUMN scan_error TEXT;
ALTER TABLE space_sources ADD COLUMN scan_started_at TEXT;
ALTER TABLE space_sources ADD COLUMN scan_completed_at TEXT;
```

**Обоснование:**
- Статус хранится на уровне источника (каждой директории), а не пространства
- Позволяет показывать индивидуальный прогресс для каждого источника
- Не требует отдельной таблицы, проще в реализации
- История сканирований не нужна (только текущий статус)

**Добавляем колонку `status` в таблицу `installs` для отслеживания состояния игр:**
```sql
ALTER TABLE installs ADD COLUMN status TEXT DEFAULT 'installed'; -- installed, missing, modified
```

**Добавляем `fingerprint` в `installs` для контроля изменений:**
```sql
ALTER TABLE installs ADD COLUMN fingerprint TEXT;
```

**Индекс для быстрого поиска:**
```sql
CREATE INDEX IF NOT EXISTS idx_installs_space_status ON installs(space_id, status);
```

### 2. Rust Backend

**Модели (дополняем SpaceSource в models.rs):**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSource {
    pub space_id: String,
    pub source_path: String,
    pub is_active: bool,
    pub scan_recursively: bool,
    pub last_scanned_at: Option<String>,
    pub exclude_patterns: Option<Vec<String>>,
    // Новые поля для статуса сканирования
    pub scan_status: Option<String>, // "idle", "scanning", "completed", "error"
    pub scan_progress: Option<i32>,
    pub scan_total: Option<i32>,
    pub scan_error: Option<String>,
    pub scan_started_at: Option<String>,
    pub scan_completed_at: Option<String>,
}

// Статус игры в пространстве
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStatus {
    Installed,
    Missing,
    Modified,
}
```

**ScanningService:**
```rust
pub struct ScanningService {
    active_scans: Mutex<HashMap<String, ScanHandle>>,
}

struct ScanHandle {
    thread: JoinHandle<()>,
    cancel_flag: Arc<AtomicBool>,
}

impl ScanningService {
    pub fn new() -> Self {
        Self {
            active_scans: Mutex::new(HashMap::new()),
        }
    }

    pub fn start_scan(&self, space_id: String, source_path: String, db: Arc<Mutex<Connection>>) -> Result<(), String> {
        // Проверяем, нет ли уже активного сканирования этого источника
        // Создаем запись в БД: обновляем space_sources.scan_status = "scanning"
        // Запускаем фоновый поток
        let thread = std::thread::spawn({
            let db_clone = db.clone();
            let source_path_clone = source_path.clone();
            let space_id_clone = space_id.clone();
            move || {
                // Фоновое сканирование
                Self::scan_source(space_id_clone, source_path_clone, db_clone);
            }
        });

        let handle = ScanHandle {
            thread,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        };
        self.active_scans.lock().unwrap().insert(source_path, handle);
        Ok(())
    }

    fn scan_source(space_id: String, source_path: String, db: Arc<Mutex<Connection>>) {
        // 1. Получаем список активных источников для пространства
        // 2. Для каждого источника (в данном случае только source_path):
        //    - Устанавливаем статус "scanning", progress=0, total=количество найденных игр
        //    - Сканируем директорию: scan_directory_internal(source_path)
        //    - Для каждой найденной игры:
        //        * Проверяем, есть ли уже install с таким install_path в этом пространстве
        //        * Если есть: сравниваем fingerprint
        //          - fingerprint совпадает → обновляем данные (если изменились), статус installed
        //          - fingerprint изменился → статус modified
        //        * Если нет: создаем новую игру и install, статус installed
        //        * Увеличиваем progress
        //    - После сканирования: находим все installs для этого источника, которых нет в найденных
        //      - Помечаем их статусом "missing"
        //    - Устанавливаем статус "completed" или "error"
    }

    pub fn get_source_scan_status(&self, space_id: &str, source_path: &str) -> Result<SourceScanStatus, String> {
        // Читаем из БД статус сканирования для данного источника
    }

    pub fn cancel_scan(&self, space_id: &str, source_path: &str) -> Result<(), String> {
        // Устанавливаем cancel_flag
        // Обновляем статус в БД на "idle"
    }
}
```

**Новые команды:**
- `start_source_scan(space_id: String, source_path: String) -> Result<(), String>`
- `get_source_scan_status(space_id: String, source_path: String) -> Result<SourceScanStatus, String>`
- `cancel_source_scan(space_id: String, source_path: String) -> Result<(), String>`
- `get_space_sources_with_status(space_id: String) -> Result<Vec<SpaceSource>, String>` (уже есть get_space_sources, дополним)

### 3. Frontend

**Новые хуки:**
```typescript
// src/hooks/useScanning.ts
export interface SourceScanStatus {
    space_id: string;
    source_path: string;
    status: 'idle' | 'scanning' | 'completed' | 'error';
    progress_current: number;
    progress_total: number;
    error_message?: string;
    started_at?: string;
    completed_at?: string;
}

export function useSourceScanStatus(spaceId: string, sourcePath: string) {
  return useQuery({
    queryKey: ['source_scan_status', spaceId, sourcePath],
    queryFn: () => invoke<SourceScanStatus>('get_source_scan_status', { spaceId, sourcePath }),
    enabled: !!spaceId && !!sourcePath,
    refetchInterval: 2000, // Опрашиваем каждые 2 секунды
  });
}

export function useStartSourceScan() {
  const queryClient = useQueryClient();
  
  return useMutation({
    mutationFn: ({ spaceId, sourcePath }: { spaceId: string; sourcePath: string }) =>
      invoke('start_source_scan', { spaceId, sourcePath }),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ['source_scan_status', variables.spaceId, variables.sourcePath] });
      queryClient.invalidateQueries({ queryKey: ['space_sources', variables.spaceId] });
    },
  });
}

export function useCancelSourceScan() {
  return useMutation({
    mutationFn: ({ spaceId, sourcePath }: { spaceId: string; sourcePath: string }) =>
      invoke('cancel_source_scan', { spaceId, sourcePath }),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ['source_scan_status', variables.spaceId, variables.sourcePath] });
    },
  });
}
```

**Обновление SpaceItem (карточка пространства):**
```tsx
// src/components/SpaceItem.tsx - НЕОБХОДИМО ПЕРЕРАБОТАТЬ
// Теперь SpaceItem должен показывать список источников (space_sources)
// Для каждого источника:
//   - путь
//   - кнопка "Сканировать" (если не сканируется)
//   - статус сканирования (idle/scanning/completed/error) с прогресс-баром
//   - кнопка "Отменить" (если scanning)
//   - кнопка "Удалить" источник

// Примерная структура:
export default function SpaceItem({ space }: { space: Space }) {
  const { data: sources = [] } = useSpaceSources(space.id);
  
  return (
    <div className="space-card">
      <h3>{space.name}</h3>
      {/* ... другие поля ... */}
      
      <div className="sources-list">
        <h4>Источники:</h4>
        {sources.map(source => (
          <SourceItem key={source.source_path} source={source} spaceId={space.id} />
        ))}
        <button onClick={handleAddSource}>+ Добавить источник</button>
      </div>
    </div>
  );
}

// Компонент SourceItem:
function SourceItem({ source, spaceId }: { source: SpaceSource; spaceId: string }) {
  const { data: scanStatus } = useSourceScanStatus(spaceId, source.source_path);
  const startScan = useStartSourceScan();
  const cancelScan = useCancelSourceScan();

  return (
    <div className="source-item">
      <span className="source-path">{source.source_path}</span>
      
      {scanStatus?.status === 'scanning' && (
        <div className="scan-progress">
          <progress value={scanStatus.progress_current} max={scanStatus.progress_total} />
          <span>{scanStatus.progress_current}/{scanStatus.progress_total}</span>
          <button onClick={() => cancelScan.mutate({ spaceId, sourcePath: source.source_path })}>
            Отменить
          </button>
        </div>
      )}
      
      {scanStatus?.status === 'error' && (
        <div className="scan-error">{scanStatus.error_message}</div>
      )}
      
      {(scanStatus?.status === 'idle' || scanStatus?.status === 'completed' || !scanStatus) && (
        <button
          onClick={() => startScan.mutate({ spaceId, sourcePath: source.source_path })}
          disabled={scanStatus?.status === 'scanning'}
        >
          Сканировать
        </button>
      )}
    </div>
  );
}
```

**Управление источниками:**
- Нужен UI для добавления новых источников к существующему пространству
- Можно добавить кнопку "Добавить источник" в SpaceItem
- Использовать тот же диалог выбора папки, что и в AddSpaceDialog
- Вызывать `add_space_source` команду, затем `start_source_scan`

**Отображение missing игр:**
- В компоненте GameCard/GameGrid:
  - Проверять `install.status` (добавить в тип Game/Install)
  - Если `status === 'missing'` → показывать иконку ⚠️
  - Добавить кнопку "Найти заново" → запускает сканирование источника этой игры
- В интерфейсе: фильтр "Показать отсутствующие" или отдельная вкладка

**AddSpaceDialog:**
- Пока оставляем без изменений (создание пространства с выбором источников)
- После создания пространства НЕ запускаем автосканирование
- Пользователь вручную нажимает "Сканировать" для каждого источника

## Последовательность реализации

### Phase 1: Database
1. Миграция: добавляем колонки в `space_sources` (scan_status, scan_progress, scan_total, scan_error, scan_started_at, scan_completed_at)
2. Миграция: добавляем колонки в `installs` (status, fingerprint)
3. Создаем индекс для быстрого поиска: `idx_installs_space_status`

### Phase 2: Rust Backend
4. Обновляем `SpaceSource` модель в `src-tauri/src/models.rs` (добавляем поля сканирования)
5. Обновляем `Install` модель (добавляем status, fingerprint)
6. Создаем `src-tauri/src/scanning_service.rs`:
   - `ScanningService` struct с `active_scans: HashMap`
   - `start_scan(space_id, source_path)` - запуск фонового сканирования источника
   - `scan_source()` - логика сканирования одной директории с проверкой дублей и fingerprint
   - `get_source_scan_status()` - получение статуса из БД
   - `cancel_scan()` - отмена
7. Добавляем методы в `Database` (src-tauri/src/database.rs):
   - `set_source_scan_status(space_id, source_path, status, progress, total, error)`
   - `clear_source_scan_status(space_id, source_path)`
   - `get_source_scan_status(space_id, source_path) -> Option<SourceScanStatus>`
   - `get_install_by_path(space_id, install_path) -> Option<Install>`
   - `update_install(install_id, status, fingerprint)` - обновляет статус и fingerprint
   - `get_installs_for_source(space_id, source_path) -> Vec<Install>` - получить все installs для источника
8. Реализуем команды в `src-tauri/src/commands/spaces.rs`:
   - `start_source_scan`
   - `get_source_scan_status`
   - `cancel_source_scan`
9. Инициализируем `ScanningService` в `AppState` (src-tauri/src/lib.rs)
10. Регистрируем команды в `src-tauri/src/commands/mod.rs`

### Phase 3: Frontend
11. Создаем `src/hooks/useScanning.ts`:
    - `useSourceScanStatus(spaceId, sourcePath)`
    - `useStartSourceScan()`
    - `useCancelSourceScan()`
12. Обновляем `src/hooks/useSpaces.ts`:
    - Убеждаемся, что `useSpaceSources` возвращает `SpaceSource` с полями сканирования
13. Создаем/обновляем компоненты:
    - `src/components/SpaceItem.tsx` - перерабатываем: показываем список источников, кнопки сканирования, статус для каждого
    - `src/components/SourceItem.tsx` - новый компонент для отображения источника (или встроить в SpaceItem)
    - `src/components/GameCard.tsx` / `GameGrid.tsx` - показывать иконку ⚠️ для missing игр, кнопка "Найти заново"
14. Добавляем UI для управления источниками:
    - Кнопка "Добавить источник" в SpaceItem
    - Диалог выбора папки (переиспользовать из AddSpaceDialog)
    - После добавления источника: вызываем `add_space_source` + автоматически `start_source_scan`
15. Обновляем `src/components/AddSpaceDialog.tsx`:
    - Пока оставляем без изменений (создание пространства с источниками)
    - НЕ запускаем автосканирование после создания

### Phase 4: Testing & Polish
16. Тестируем полный цикл:
    - Создание пространства с несколькими источниками
    - Запуск сканирования для каждого источника
    - Проверка прогресса (отображение в UI)
    - Проверка добавления игр в библиотеку
    - Проверка обработки дублей (одна игра в нескольких источниках)
    - Проверка missing статуса (удаление файла, повторное сканирование)
    - Проверка modified статуса (изменение exe файла)
    - Отмена сканирования
    - Обработка ошибок (нет доступа, путь не существует)
17. Добавляем обработку ошибок в UI (показ сообщений)
18. Оптимизация: кэширование fingerprint, параллельное сканирование (опционально)

## Отличия от предыдущего плана

- **Статус на уровне источника**, а не пространства (каждый источник сканируется независимо)
- **ScanDialog не нужен** для управления сканированием пространств (все через SpaceItem)
- **Полное сканирование** при каждом запуске: проверяет все игры источника, обновляет статусы (installed/missing/modified)
- **Fingerprint проверка** для обнаружения изменений
- **Управление источниками** после создания пространства (добавление/удаление)
- **Missing игры** остаются в библиотеке, но помечаются статусом и показываются с иконкой ⚠️

## Оценка сложности

- База данных: 1 час
- Rust backend: 6-8 часов (логика сканирования с проверкой дублей и fingerprint сложная)
- Frontend: 4-5 часов (обновление UI, новые компоненты, управление источниками)
- Тестирование: 2-3 часа
- **Итого: 13-17 часов**

## Риски

1. **Производительность fingerprint**: вычисление контрольной суммы для каждого exe может быть медленным. Нужно кэшировать или вычислять только при изменении размера/даты.
2. **Параллельное сканирование**: если пользователь запустит сканирование для двух источников одновременно, возможны гонки при добавлении игр. Нужна синхронизация (Mutex на уровне БД операций).
3. **Утечки памяти**: фоновые потоки должны корректно завершаться при закрытии приложения. Нужен shutdown hook.
4. **Сложность логики**: обработка всех кейсов (дубли, missing, modified) может быть нетривиальной. Нужно тщательно тестировать.

## Дополнительные улучшения (пост-фактум)

- Параллельное сканирование нескольких источников (с настройкой лимита потоков)
- Общий прогресс по всем источникам пространства
- Возможность приостановки/возобновления сканирования
- История изменений статусов игр
- Автоматическое сканирование по расписанию
- Уведомления о завершении сканирования

## Дополнительные улучшения (пост-фактум)

- Показывать количество найденных игр на карточке
- Возможность просмотра истории сканирований
- Параллельное сканирование нескольких источников (с настройкой лимита)
- Приостановка/возобновление сканирования
- Интеграция с системными уведомлениями
