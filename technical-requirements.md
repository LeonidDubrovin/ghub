# GHub - Технические требования к игровому лаунчеру

## 1. Обзор проекта

**GHub** — кроссплатформенный десктопный лаунчер для управления игровой библиотекой, который объединяет игры из различных источников (локальные каталоги, Steam, itch.io) в единый интерфейс с поддержкой пространств (spaces), учётом времени игры и массовой загрузки.

### 1.1 Цели проекта
- Единая точка управления всеми играми пользователя
- Offline-first архитектура с опциональной синхронизацией
- Надёжный учёт времени игры (heartbeat-система)
- Интеграция с itch.io API и Butler для загрузок
- Кроссплатформенность (Windows приоритет, macOS, Linux)

---

## 2. Сравнительный анализ технологий

### 2.1 Анализ существующих решений

| Характеристика | Playnite | itch.io app | GHub (предлагаемое) |
|---------------|----------|-------------|---------------------|
| **Фреймворк** | WPF (.NET) | Electron + React | **Tauri 2.0** |
| **Язык backend** | C# | Node.js | **Rust** |
| **Язык frontend** | XAML | TypeScript/React | **TypeScript/React** |
| **База данных** | LiteDB/SQLite | SQLite | **SQLite + FTS5** |
| **Размер приложения** | ~150 MB | ~200 MB | **~15-30 MB** |
| **RAM usage** | ~200 MB | ~300+ MB | **~50-100 MB** |
| **Кроссплатформенность** | Windows only | Win/Mac/Linux | **Win/Mac/Linux** |
| **Плагины** | C# scripts | Нет | **WASM/JS** |

### 2.2 Выбор основного фреймворка

#### Рекомендация: **Tauri 2.0**

**Преимущества:**
1. **Производительность**: Rust backend, нативный WebView вместо Chromium
2. **Размер**: ~15-30 MB vs ~200 MB у Electron
3. **Безопасность**: Rust гарантирует memory safety, sandbox для frontend
4. **Кроссплатформенность**: Windows, macOS, Linux из одной кодовой базы
5. **Интеграция с системой**: Нативный доступ к файловой системе, системным API
6. **Горячая перезагрузка**: Быстрая разработка frontend

**Альтернативы (если Rust неприемлем):**
- **Electron + React**: Проверенное решение, большое сообщество, но тяжеловесное
- **Avalonia UI (.NET 8)**: Если предпочитаете C#, но меньше сообщество

#### Технологический стек

```
┌─────────────────────────────────────────────────────────────┐
│                        GHub Application                      │
├─────────────────────────────────────────────────────────────┤
│  Frontend (WebView)                                          │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  React 18 + TypeScript + Vite                           ││
│  │  ├── TanStack Query (кеширование/синхронизация)         ││
│  │  ├── Zustand (state management)                         ││
│  │  ├── Tailwind CSS + Radix UI (компоненты)               ││
│  │  ├── React Router (навигация)                           ││
│  │  └── i18next (локализация RU/EN)                        ││
│  └─────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│  Tauri Bridge (IPC)                                          │
│  ├── tauri::command (Rust → JS)                              │
│  └── invoke() (JS → Rust)                                    │
├─────────────────────────────────────────────────────────────┤
│  Backend (Rust)                                              │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  Core Services                                          ││
│  │  ├── GameManager (CRUD, импорт, поиск)                  ││
│  │  ├── SpaceManager (пространства/каталоги)               ││
│  │  ├── PlaytimeTracker (heartbeat, сессии)                ││
│  │  ├── DownloadManager (butler integration)               ││
│  │  ├── ItchIntegration (OAuth, API)                       ││
│  │  ├── SteamScanner (VDF parser, manifests)               ││
│  │  ├── MetadataScraper (IGDB, SteamGridDB)                ││
│  │  └── PluginHost (WASM runtime)                          ││
│  └─────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│  Data Layer                                                  │
│  ├── SQLite (rusqlite + r2d2 connection pool)                │
│  ├── FTS5 (полнотекстовый поиск)                             │
│  └── File Cache (artwork, thumbnails)                        │
├─────────────────────────────────────────────────────────────┤
│  External Integrations                                       │
│  ├── Butler CLI (itch.io downloads)                          │
│  ├── Steam VDF Parser                                        │
│  ├── IGDB API (метаданные)                                   │
│  └── SteamGridDB API (обложки)                               │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. Архитектура хранения данных

### 3.1 Файловая структура приложения

```
%APPDATA%/GHub/                          # Windows
~/Library/Application Support/GHub/      # macOS  
~/.local/share/ghub/                     # Linux
│
├── config/
│   ├── settings.json                    # Настройки приложения
│   └── spaces.json                      # Конфигурация пространств
│
├── data/
│   ├── ghub.db                          # Основная SQLite база
│   ├── ghub.db-wal                      # WAL журнал
│   └── ghub.db-shm                      # Shared memory
│
├── cache/
│   ├── artwork/                         # Обложки игр
│   │   ├── {sha256}.jpg                 # Content-addressed storage
│   │   └── {sha256}.webp
│   ├── thumbnails/                      # Миниатюры (128x128)
│   │   └── {sha256}_thumb.webp
│   └── temp/                            # Временные файлы
│
├── logs/
│   ├── app.log                          # Логи приложения (ротация)
│   ├── downloads.log                    # Логи загрузок
│   └── playtime.log                     # Логи сессий (backup)
│
├── plugins/                             # Пользовательские плагины
│   └── {plugin-id}/
│       ├── manifest.json
│       └── plugin.wasm
│
├── backups/                             # Автоматические бэкапы БД
│   ├── ghub_2024-01-15.db
│   └── ghub_2024-01-08.db
│
└── butler/                              # Butler CLI (опционально)
    └── butler.exe
```

### 3.2 Схема базы данных SQLite

```sql
-- ============================================
-- GHUB DATABASE SCHEMA v1.0
-- SQLite with FTS5 for full-text search
-- ============================================

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA auto_vacuum = INCREMENTAL;

-- ============================================
-- SPACES (Пространства/Библиотеки)
-- ============================================
CREATE TABLE spaces (
    id              TEXT PRIMARY KEY,           -- UUID v7
    name            TEXT NOT NULL,              -- "Основная библиотека"
    path            TEXT,                       -- "D:/Games" (NULL для виртуальных)
    type            TEXT NOT NULL DEFAULT 'local',  -- local|steam|itch|virtual
    icon            TEXT,                       -- emoji или путь к иконке
    color           TEXT,                       -- HEX цвет для UI
    sort_order      INTEGER DEFAULT 0,
    is_active       BOOLEAN DEFAULT TRUE,
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);

-- Индексы для spaces
CREATE INDEX idx_spaces_type ON spaces(type);
CREATE INDEX idx_spaces_active ON spaces(is_active);

-- ============================================
-- SOURCES (Источники данных)
-- ============================================
CREATE TABLE sources (
    id              TEXT PRIMARY KEY,           -- 'steam', 'itch', 'igdb', 'manual'
    name            TEXT NOT NULL,
    api_endpoint    TEXT,
    auth_required   BOOLEAN DEFAULT FALSE,
    created_at      TEXT DEFAULT (datetime('now'))
);

-- Предустановленные источники
INSERT INTO sources (id, name, api_endpoint, auth_required) VALUES
    ('manual', 'Manual Entry', NULL, FALSE),
    ('steam', 'Steam', 'https://store.steampowered.com/api', FALSE),
    ('itch', 'itch.io', 'https://api.itch.io', TRUE),
    ('igdb', 'IGDB', 'https://api.igdb.com/v4', TRUE),
    ('steamgriddb', 'SteamGridDB', 'https://www.steamgriddb.com/api/v2', TRUE);

-- ============================================
-- GAMES (Основная таблица игр)
-- ============================================
CREATE TABLE games (
    id              TEXT PRIMARY KEY,           -- UUID v7
    title           TEXT NOT NULL,
    sort_title      TEXT,                       -- Для сортировки (без "The", "A")
    description     TEXT,
    release_date    TEXT,                       -- ISO 8601 date
    developer       TEXT,
    publisher       TEXT,
    
    -- Медиа (ссылки на кеш или URL)
    cover_image     TEXT,                       -- SHA256 hash или URL
    background_image TEXT,
    icon_image      TEXT,
    
    -- Агрегированные данные
    total_playtime_seconds  INTEGER DEFAULT 0,
    last_played_at  TEXT,
    times_launched  INTEGER DEFAULT 0,
    
    -- Статусы
    is_favorite     BOOLEAN DEFAULT FALSE,
    is_hidden       BOOLEAN DEFAULT FALSE,
    completion_status TEXT DEFAULT 'not_played', -- not_played|playing|completed|abandoned|on_hold
    user_rating     INTEGER CHECK (user_rating BETWEEN 1 AND 10),
    
    -- Мета
    added_at        TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now')),
    
    -- Для дедупликации
    fingerprint     TEXT                        -- Hash от (title + developer) для детекции дублей
);

-- Индексы для games
CREATE INDEX idx_games_title ON games(title COLLATE NOCASE);
CREATE INDEX idx_games_sort_title ON games(sort_title COLLATE NOCASE);
CREATE INDEX idx_games_last_played ON games(last_played_at DESC);
CREATE INDEX idx_games_playtime ON games(total_playtime_seconds DESC);
CREATE INDEX idx_games_favorite ON games(is_favorite) WHERE is_favorite = TRUE;
CREATE INDEX idx_games_fingerprint ON games(fingerprint);
CREATE INDEX idx_games_completion ON games(completion_status);

-- ============================================
-- GAME_SOURCES (Связь игры с внешними ID)
-- ============================================
CREATE TABLE game_sources (
    id              TEXT PRIMARY KEY,
    game_id         TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    source_id       TEXT NOT NULL REFERENCES sources(id),
    external_id     TEXT NOT NULL,              -- ID игры в источнике
    external_url    TEXT,                       -- Ссылка на страницу игры
    metadata_json   TEXT,                       -- Дополнительные данные из источника
    last_synced_at  TEXT,
    UNIQUE(game_id, source_id)
);

CREATE INDEX idx_game_sources_external ON game_sources(source_id, external_id);

-- ============================================
-- INSTALLS (Установки игр)
-- ============================================
CREATE TABLE installs (
    id              TEXT PRIMARY KEY,           -- UUID v7
    game_id         TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    space_id        TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
    
    -- Путь и запуск
    install_path    TEXT NOT NULL,              -- "D:/Games/Celeste"
    executable_path TEXT,                       -- Относительный: "Celeste.exe"
    launch_arguments TEXT,                      -- Аргументы запуска
    working_directory TEXT,                     -- Рабочая директория
    
    -- Альтернативные способы запуска
    launch_uri      TEXT,                       -- "steam://run/504230"
    
    -- Статус
    status          TEXT DEFAULT 'installed',   -- installed|installing|update_available|broken
    version         TEXT,
    install_size_bytes INTEGER,
    
    -- Даты
    installed_at    TEXT DEFAULT (datetime('now')),
    last_verified_at TEXT,
    
    UNIQUE(game_id, space_id)                   -- Одна установка игры на пространство
);

CREATE INDEX idx_installs_game ON installs(game_id);
CREATE INDEX idx_installs_space ON installs(space_id);
CREATE INDEX idx_installs_path ON installs(install_path);

-- ============================================
-- TAGS (Теги)
-- ============================================
CREATE TABLE tags (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    color           TEXT,                       -- HEX цвет
    category        TEXT DEFAULT 'user',        -- user|genre|feature|platform
    created_at      TEXT DEFAULT (datetime('now'))
);

CREATE TABLE game_tags (
    game_id         TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    tag_id          TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (game_id, tag_id)
);

CREATE INDEX idx_game_tags_tag ON game_tags(tag_id);

-- ============================================
-- PLAY SESSIONS (Сессии игры)
-- ============================================
CREATE TABLE play_sessions (
    id              TEXT PRIMARY KEY,           -- UUID v7
    game_id         TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    install_id      TEXT REFERENCES installs(id) ON DELETE SET NULL,
    
    started_at      TEXT NOT NULL,              -- ISO 8601 datetime
    ended_at        TEXT,                       -- NULL = сессия активна
    duration_seconds INTEGER,                   -- Вычисляется при завершении
    
    -- Для восстановления при сбое
    last_heartbeat_at TEXT,
    heartbeat_count   INTEGER DEFAULT 0,
    
    -- Статус сессии
    status          TEXT DEFAULT 'active',      -- active|completed|crashed|recovered
    
    created_at      TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_play_sessions_game ON play_sessions(game_id);
CREATE INDEX idx_play_sessions_active ON play_sessions(status) WHERE status = 'active';
CREATE INDEX idx_play_sessions_dates ON play_sessions(started_at DESC);

-- ============================================
-- ACTIVE SESSIONS (Heartbeat таблица)
-- Используется для real-time отслеживания
-- ============================================
CREATE TABLE active_sessions (
    id              TEXT PRIMARY KEY,           -- = play_sessions.id
    game_id         TEXT NOT NULL,
    process_pid     INTEGER,
    accumulated_seconds INTEGER DEFAULT 0,      -- Накопленное время с последнего checkpoint
    last_heartbeat  TEXT NOT NULL,
    checkpoint_at   TEXT,                       -- Последний checkpoint в play_sessions
    
    FOREIGN KEY (id) REFERENCES play_sessions(id) ON DELETE CASCADE
);

-- ============================================
-- DOWNLOADS (Очередь загрузок)
-- ============================================
CREATE TABLE downloads (
    id              TEXT PRIMARY KEY,           -- UUID v7
    
    -- Что качаем
    source_type     TEXT NOT NULL,              -- 'itch'|'direct'
    source_url      TEXT NOT NULL,
    source_id       TEXT,                       -- ID на источнике (itch game_id)
    
    -- Связь с игрой (может быть NULL для "отложенных")
    game_id         TEXT REFERENCES games(id) ON DELETE SET NULL,
    target_space_id TEXT REFERENCES spaces(id),
    target_path     TEXT,
    
    -- Статус
    status          TEXT DEFAULT 'pending',     -- pending|queued|downloading|paused|completed|failed
    priority        INTEGER DEFAULT 0,
    progress_percent REAL DEFAULT 0,
    downloaded_bytes INTEGER DEFAULT 0,
    total_bytes     INTEGER,
    
    -- Butler-специфичное
    butler_operation_id TEXT,
    
    -- Мета
    title           TEXT,                       -- Название для отображения
    error_message   TEXT,
    retry_count     INTEGER DEFAULT 0,
    
    created_at      TEXT DEFAULT (datetime('now')),
    started_at      TEXT,
    completed_at    TEXT
);

CREATE INDEX idx_downloads_status ON downloads(status);
CREATE INDEX idx_downloads_game ON downloads(game_id);

-- ============================================
-- WISHLIST / DEFERRED (Отложенные для просмотра)
-- ============================================
CREATE TABLE wishlist (
    id              TEXT PRIMARY KEY,
    url             TEXT NOT NULL,
    title           TEXT,                       -- Извлечённое название
    source_type     TEXT,                       -- 'itch'|'steam'|'other'
    source_id       TEXT,                       -- ID если распарсили
    thumbnail_url   TEXT,
    notes           TEXT,                       -- Заметки пользователя
    
    status          TEXT DEFAULT 'pending',     -- pending|reviewed|added|dismissed
    target_space_id TEXT REFERENCES spaces(id), -- Куда хотим скачать
    
    created_at      TEXT DEFAULT (datetime('now')),
    reviewed_at     TEXT
);

CREATE INDEX idx_wishlist_status ON wishlist(status);
CREATE INDEX idx_wishlist_source ON wishlist(source_type, source_id);

-- ============================================
-- SETTINGS (Настройки приложения)
-- ============================================
CREATE TABLE settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL,              -- JSON-encoded value
    updated_at      TEXT DEFAULT (datetime('now'))
);

-- Предустановленные настройки
INSERT INTO settings (key, value) VALUES
    ('language', '"ru"'),
    ('theme', '"dark"'),
    ('heartbeat_interval_ms', '15000'),
    ('checkpoint_interval_ms', '60000'),
    ('auto_backup_enabled', 'true'),
    ('auto_backup_days', '7'),
    ('default_space_id', 'null'),
    ('minimize_to_tray', 'true'),
    ('start_minimized', 'false'),
    ('close_to_tray', 'true');

-- ============================================
-- AUTH TOKENS (Токены авторизации)
-- Хранятся зашифрованно (DPAPI/Keychain)
-- ============================================
CREATE TABLE auth_tokens (
    service         TEXT PRIMARY KEY,           -- 'itch', 'igdb'
    encrypted_token BLOB NOT NULL,              -- Зашифрованный токен
    refresh_token   BLOB,                       -- Зашифрованный refresh token
    expires_at      TEXT,
    scopes          TEXT,                       -- JSON array
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);

-- ============================================
-- FULL-TEXT SEARCH (FTS5)
-- ============================================
CREATE VIRTUAL TABLE games_fts USING fts5(
    title,
    description,
    developer,
    publisher,
    content='games',
    content_rowid='rowid'
);

-- Триггеры для синхронизации FTS
CREATE TRIGGER games_ai AFTER INSERT ON games BEGIN
    INSERT INTO games_fts(rowid, title, description, developer, publisher)
    VALUES (NEW.rowid, NEW.title, NEW.description, NEW.developer, NEW.publisher);
END;

CREATE TRIGGER games_ad AFTER DELETE ON games BEGIN
    INSERT INTO games_fts(games_fts, rowid, title, description, developer, publisher)
    VALUES('delete', OLD.rowid, OLD.title, OLD.description, OLD.developer, OLD.publisher);
END;

CREATE TRIGGER games_au AFTER UPDATE ON games BEGIN
    INSERT INTO games_fts(games_fts, rowid, title, description, developer, publisher)
    VALUES('delete', OLD.rowid, OLD.title, OLD.description, OLD.developer, OLD.publisher);
    INSERT INTO games_fts(rowid, title, description, developer, publisher)
    VALUES (NEW.rowid, NEW.title, NEW.description, NEW.developer, NEW.publisher);
END;

-- ============================================
-- VIEWS (Представления для удобства)
-- ============================================

-- Игры с информацией об установках
CREATE VIEW v_games_with_installs AS
SELECT 
    g.*,
    GROUP_CONCAT(DISTINCT s.name) as space_names,
    COUNT(i.id) as install_count,
    SUM(i.install_size_bytes) as total_install_size
FROM games g
LEFT JOIN installs i ON g.id = i.game_id
LEFT JOIN spaces s ON i.space_id = s.id
GROUP BY g.id;

-- Активные сессии с информацией об игре
CREATE VIEW v_active_sessions AS
SELECT 
    a.*,
    g.title as game_title,
    g.cover_image,
    ps.started_at as session_started_at
FROM active_sessions a
JOIN games g ON a.game_id = g.id
JOIN play_sessions ps ON a.id = ps.id;

-- Статистика по пространствам
CREATE VIEW v_space_stats AS
SELECT 
    s.*,
    COUNT(DISTINCT i.game_id) as game_count,
    SUM(i.install_size_bytes) as total_size
FROM spaces s
LEFT JOIN installs i ON s.id = i.space_id
GROUP BY s.id;
```

### 3.3 Безопасное хранение токенов

```rust
// Псевдокод для шифрования токенов

#[cfg(target_os = "windows")]
fn store_token(service: &str, token: &str) -> Result<()> {
    // Используем Windows DPAPI через windows-sys crate
    let encrypted = dpapi::encrypt(token.as_bytes())?;
    db.execute("INSERT OR REPLACE INTO auth_tokens ...", (service, encrypted))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn store_token(service: &str, token: &str) -> Result<()> {
    // Используем macOS Keychain через security-framework crate
    keychain::set_generic_password("GHub", service, token)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn store_token(service: &str, token: &str) -> Result<()> {
    // Используем libsecret через secret-service crate
    secret_service::store("GHub", service, token)?;
    Ok(())
}
```

---

## 4. Логика учёта времени игры (Heartbeat)

### 4.1 Алгоритм

```
┌──────────────────────────────────────────────────────────────────┐
│                    PLAYTIME TRACKING FLOW                         │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  [Запуск игры]                                                    │
│       │                                                           │
│       ▼                                                           │
│  ┌─────────────────┐                                              │
│  │ CREATE SESSION  │  → play_sessions (status='active')           │
│  │                 │  → active_sessions (accumulated=0)           │
│  └────────┬────────┘                                              │
│           │                                                       │
│           ▼                                                       │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │                   HEARTBEAT LOOP                         │     │
│  │  ┌─────────────────────────────────────────────────────┐ │     │
│  │  │ Каждые 15 секунд:                                   │ │     │
│  │  │  1. Проверить: процесс игры жив?                    │ │     │
│  │  │  2. Если ДА:                                        │ │     │
│  │  │     - active_sessions.accumulated += 15             │ │     │
│  │  │     - active_sessions.last_heartbeat = NOW          │ │     │
│  │  │  3. Если НЕТ → выход из цикла                       │ │     │
│  │  └─────────────────────────────────────────────────────┘ │     │
│  │                                                           │     │
│  │  ┌─────────────────────────────────────────────────────┐ │     │
│  │  │ Каждые 60 секунд (CHECKPOINT):                      │ │     │
│  │  │  1. Записать accumulated в play_sessions            │ │     │
│  │  │  2. Обновить games.total_playtime_seconds           │ │     │
│  │  │  3. Сбросить accumulated = 0                        │ │     │
│  │  │  4. checkpoint_at = NOW                             │ │     │
│  │  └─────────────────────────────────────────────────────┘ │     │
│  └─────────────────────────────────────────────────────────┘     │
│           │                                                       │
│           ▼                                                       │
│  ┌─────────────────┐                                              │
│  │ END SESSION     │  Нормальное завершение:                      │
│  │                 │  - Финальный checkpoint                      │
│  │                 │  - play_sessions.status = 'completed'        │
│  │                 │  - Удалить из active_sessions                │
│  └─────────────────┘                                              │
│                                                                   │
├──────────────────────────────────────────────────────────────────┤
│                    RECOVERY ON STARTUP                            │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  При запуске приложения:                                          │
│  1. Найти все записи в active_sessions                            │
│  2. Для каждой:                                                   │
│     a. Вычислить: lost_time = accumulated +                       │
│        (last_heartbeat - checkpoint_at)                           │
│     b. Добавить lost_time к play_sessions.duration_seconds        │
│     c. Обновить games.total_playtime_seconds                      │
│     d. play_sessions.status = 'recovered'                         │
│     e. Удалить из active_sessions                                 │
│                                                                   │
│  Максимальная потеря: ~15 секунд (один интервал heartbeat)        │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### 4.2 Rust реализация (псевдокод)

```rust
pub struct PlaytimeTracker {
    db: Arc<Database>,
    active: Arc<RwLock<HashMap<String, ActiveSession>>>,
    heartbeat_interval: Duration,   // 15 sec
    checkpoint_interval: Duration,  // 60 sec
}

impl PlaytimeTracker {
    pub async fn start_session(&self, game_id: &str, install_id: &str, pid: u32) -> Result<String> {
        let session_id = Uuid::now_v7().to_string();
        let now = Utc::now();
        
        // Создаём записи в БД
        self.db.execute(
            "INSERT INTO play_sessions (id, game_id, install_id, started_at, status) 
             VALUES (?, ?, ?, ?, 'active')",
            (&session_id, game_id, install_id, now.to_rfc3339())
        )?;
        
        self.db.execute(
            "INSERT INTO active_sessions (id, game_id, process_pid, last_heartbeat) 
             VALUES (?, ?, ?, ?)",
            (&session_id, game_id, pid, now.to_rfc3339())
        )?;
        
        // Запускаем heartbeat loop
        self.spawn_heartbeat_loop(session_id.clone(), pid);
        
        Ok(session_id)
    }
    
    fn spawn_heartbeat_loop(&self, session_id: String, pid: u32) {
        let db = self.db.clone();
        let heartbeat_interval = self.heartbeat_interval;
        let checkpoint_interval = self.checkpoint_interval;
        
        tokio::spawn(async move {
            let mut checkpoint_timer = Instant::now();
            
            loop {
                tokio::time::sleep(heartbeat_interval).await;
                
                // Проверяем, жив ли процесс
                if !is_process_running(pid) {
                    break;
                }
                
                let now = Utc::now();
                let delta_secs = heartbeat_interval.as_secs() as i32;
                
                // Обновляем heartbeat
                db.execute(
                    "UPDATE active_sessions 
                     SET accumulated_seconds = accumulated_seconds + ?,
                         last_heartbeat = ?
                     WHERE id = ?",
                    (delta_secs, now.to_rfc3339(), &session_id)
                ).ok();
                
                // Checkpoint
                if checkpoint_timer.elapsed() >= checkpoint_interval {
                    Self::do_checkpoint(&db, &session_id).await;
                    checkpoint_timer = Instant::now();
                }
            }
            
            // Финальное завершение сессии
            Self::end_session(&db, &session_id).await;
        });
    }
}
```

---

## 5. Интеграция с itch.io и Butler

### 5.1 Архитектура интеграции

```
┌─────────────────────────────────────────────────────────────┐
│                     ITCH.IO INTEGRATION                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────┐     OAuth2      ┌─────────────────┐        │
│  │   GHub      │ ←──────────────→│  itch.io API    │        │
│  │   (Tauri)   │                 │  api.itch.io    │        │
│  └──────┬──────┘                 └─────────────────┘        │
│         │                                                    │
│         │ Spawn process                                      │
│         ▼                                                    │
│  ┌─────────────┐                 ┌─────────────────┐        │
│  │   Butler    │ ←──────────────→│  wharf          │        │
│  │   CLI       │   HTTP/2        │  (CDN)          │        │
│  └──────┬──────┘                 └─────────────────┘        │
│         │                                                    │
│         │ JSON progress events (stdout)                      │
│         ▼                                                    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Target Directory (Space)                            │    │
│  │  D:/Games/itch/Celeste/                              │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 OAuth2 Flow для itch.io

```
1. Пользователь нажимает "Войти в itch.io"
2. GHub открывает браузер: 
   https://itch.io/user/oauth?client_id=XXX&scope=profile:me&redirect_uri=ghub://oauth
3. Пользователь авторизуется на itch.io
4. itch.io редиректит на ghub://oauth?code=XXX
5. Tauri перехватывает deep link, получает code
6. GHub обменивает code на access_token через API
7. Токен шифруется и сохраняется в auth_tokens
```

### 5.3 Butler команды

```bash
# Загрузка игры
butler upgrade --json "channel_url" "target_directory"

# Пример output (JSON lines):
{"type":"progress","bps":12500000,"progress":0.45}
{"type":"install-info","installed_size":1073741824}
{"type":"done"}

# Получение информации о канале
butler status "channel_url"

# Верификация установки
butler verify "target_directory"
```

### 5.4 Download Manager

```rust
pub struct DownloadManager {
    butler_path: PathBuf,
    max_concurrent: usize,
    active_downloads: Arc<RwLock<HashMap<String, DownloadHandle>>>,
}

impl DownloadManager {
    pub async fn queue_download(&self, request: DownloadRequest) -> Result<String> {
        let download_id = Uuid::now_v7().to_string();
        
        // Сохраняем в БД
        self.db.execute(
            "INSERT INTO downloads (id, source_type, source_url, target_space_id, status, title)
             VALUES (?, ?, ?, ?, 'queued', ?)",
            (&download_id, &request.source_type, &request.url, 
             &request.space_id, &request.title)
        )?;
        
        // Запускаем если есть слоты
        self.try_start_next().await;
        
        Ok(download_id)
    }
    
    async fn start_butler_download(&self, download: Download) -> Result<()> {
        let mut child = Command::new(&self.butler_path)
            .args(["upgrade", "--json", &download.source_url, &download.target_path])
            .stdout(Stdio::piped())
            .spawn()?;
        
        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        
        for line in reader.lines() {
            let event: ButlerEvent = serde_json::from_str(&line?)?;
            
            match event {
                ButlerEvent::Progress { bps, progress } => {
                    self.update_progress(&download.id, progress * 100.0).await;
                    self.emit_event("download-progress", &download.id, progress);
                }
                ButlerEvent::Done => {
                    self.complete_download(&download.id).await;
                }
                ButlerEvent::Error { message } => {
                    self.fail_download(&download.id, &message).await;
                }
            }
        }
        
        Ok(())
    }
}
```

---

## 6. Сканирование файловой системы

### 6.1 Алгоритм обнаружения игр

```
┌─────────────────────────────────────────────────────────────┐
│                    GAME DETECTION FLOW                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Input: Directory path (e.g., "D:/Games")                    │
│                                                              │
│  1. ENUMERATE SUBDIRECTORIES                                 │
│     └── Каждая папка первого уровня = потенциальная игра     │
│                                                              │
│  2. FOR EACH POTENTIAL GAME DIRECTORY:                       │
│     │                                                        │
│     ├── 2.1 Find executables (*.exe, *.app, AppImage)        │
│     │       └── Score by: name match, size, location         │
│     │                                                        │
│     ├── 2.2 Look for metadata files:                         │
│     │       ├── .itch.toml (itch.io games)                   │
│     │       ├── steam_appid.txt                              │
│     │       ├── goggame-*.info                               │
│     │       ├── unins000.exe (installer info)                │
│     │       └── *.ico, icon.png                              │
│     │                                                        │
│     ├── 2.3 Extract title:                                   │
│     │       ├── From metadata file                           │
│     │       ├── From folder name                             │
│     │       ├── From executable metadata (PE/plist)          │
│     │       └── Clean up: remove version, platform info      │
│     │                                                        │
│     └── 2.4 Generate fingerprint:                            │
│             └── hash(lowercase(title) + developer)           │
│                                                              │
│  3. CHECK FOR DUPLICATES                                     │
│     └── Compare fingerprints with existing games             │
│                                                              │
│  4. RETURN CANDIDATES                                        │
│     └── List of (path, title, executable, confidence_score)  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Steam Library Scanning

```rust
// Парсинг Steam libraryfolders.vdf
pub async fn scan_steam_libraries() -> Result<Vec<SteamLibrary>> {
    let steam_path = get_steam_path()?; // Registry on Windows, ~/.steam on Linux
    let vdf_path = steam_path.join("steamapps/libraryfolders.vdf");
    
    let vdf = vdf::parse(&std::fs::read_to_string(vdf_path)?)?;
    
    let mut libraries = Vec::new();
    for (_, folder) in vdf["libraryfolders"].iter() {
        let path = PathBuf::from(folder["path"].as_str()?);
        let apps: Vec<u64> = folder["apps"].keys()
            .filter_map(|k| k.parse().ok())
            .collect();
        
        libraries.push(SteamLibrary { path, app_ids: apps });
    }
    
    Ok(libraries)
}

// Парсинг appmanifest_*.acf для получения информации об игре
pub async fn parse_app_manifest(path: &Path) -> Result<SteamApp> {
    let acf = vdf::parse(&std::fs::read_to_string(path)?)?;
    let state = &acf["AppState"];
    
    Ok(SteamApp {
        app_id: state["appid"].as_str()?.parse()?,
        name: state["name"].as_str()?.to_string(),
        install_dir: state["installdir"].as_str()?.to_string(),
        size_on_disk: state["SizeOnDisk"].as_str()?.parse()?,
        last_updated: state["LastUpdated"].as_str()?.parse()?,
    })
}
```

---

## 7. UI/UX Архитектура

### 7.1 Структура интерфейса

```
┌────────────────────────────────────────────────────────────────────┐
│  ⬛ GHub                                              _ □ ✕        │
├────────────────────────────────────────────────────────────────────┤
│ ┌──────────┬───────────────────────────────────────────────────────┤
│ │          │  🔍 Search...                    [Grid] [List] ⚙️     │
│ │ SPACES   ├───────────────────────────────────────────────────────┤
│ │          │                                                       │
│ │ 📚 All   │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │
│ │          │  │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │     │
│ │ ⭐ Fav   │  │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │     │
│ │          │  │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │     │
│ │ 🎮 Steam │  │ Celeste │ │ Hollow  │ │ Hades   │ │ Dead    │     │
│ │          │  │ ⏱ 24h   │ │ Knight  │ │ ⏱ 156h  │ │ Cells   │     │
│ │ 🐛 itch  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘     │
│ │          │                                                       │
│ │ 📁 D:\   │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │
│ │    Games │  │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │     │
│ │          │  │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │     │
│ │ 📁 E:\   │  │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │ │ ▓▓▓▓▓▓▓ │     │
│ │    Indies│  │ Terraria│ │ Stardew │ │ Undertl │ │ Cuphead │     │
│ │          │  │         │ │ Valley  │ │         │ │         │     │
│ │ ─────────│  └─────────┘ └─────────┘ └─────────┘ └─────────┘     │
│ │ ➕ Add   │                                                       │
│ │   Space  │                                                       │
│ │          │ ────────────────────────────────────────────────────  │
│ │ ─────────│  DOWNLOADS                              2 active ↓    │
│ │ ⬇️ Queue │  ├── Inscryption ████████████░░░░ 67% 12.5 MB/s       │
│ │ (3)      │  └── Outer Wilds ████░░░░░░░░░░░░ 23% 8.2 MB/s        │
│ └──────────┴───────────────────────────────────────────────────────┤
└────────────────────────────────────────────────────────────────────┘
```

### 7.2 Компонентная структура React

```
src/
├── main.tsx                    # Entry point
├── App.tsx                     # Main app component
│
├── components/
│   ├── layout/
│   │   ├── Sidebar.tsx         # Боковая панель пространств
│   │   ├── Header.tsx          # Поиск, переключение вида
│   │   └── DownloadBar.tsx     # Панель загрузок
│   │
│   ├── games/
│   │   ├── GameGrid.tsx        # Сетка игр
│   │   ├── GameList.tsx        # Список игр
│   │   ├── GameCard.tsx        # Карточка игры
│   │   ├── GameDetails.tsx     # Детальная страница игры
│   │   └── GameContextMenu.tsx # Контекстное меню
│   │
│   ├── spaces/
│   │   ├── SpaceList.tsx       # Список пространств
│   │   ├── SpaceItem.tsx       # Элемент пространства
│   │   └── AddSpaceDialog.tsx  # Диалог добавления
│   │
│   ├── import/
│   │   ├── ScanDialog.tsx      # Диалог сканирования папки
│   │   ├── GameMatcher.tsx     # Выбор совпадения из интернета
│   │   └── BulkImport.tsx      # Массовый импорт
│   │
│   ├── settings/
│   │   ├── SettingsDialog.tsx
│   │   ├── GeneralSettings.tsx
│   │   ├── IntegrationSettings.tsx
│   │   └── LanguageSettings.tsx
│   │
│   └── common/
│       ├── Button.tsx
│       ├── Dialog.tsx
│       ├── Input.tsx
│       └── Spinner.tsx
│
├── hooks/
│   ├── useGames.ts             # TanStack Query hooks
│   ├── useSpaces.ts
│   ├── useDownloads.ts
│   ├── usePlaytime.ts
│   └── useTauriCommand.ts      # Обёртка для Tauri invoke
│
├── store/
│   └── index.ts                # Zustand store
│
├── lib/
│   ├── tauri.ts                # Tauri API helpers
│   ├── i18n.ts                 # i18next config
│   └── utils.ts
│
├── types/
│   └── index.ts                # TypeScript types
│
└── locales/
    ├── ru.json
    └── en.json
```

---

## 8. Roadmap разработки

### Phase 1: Foundation (4-6 недель)
- [ ] Настройка Tauri + React + TypeScript проекта
- [ ] Схема БД SQLite, миграции
- [ ] Базовый UI: сайдбар, сетка игр
- [ ] CRUD операции для games, spaces, installs
- [ ] Сканирование локальных папок
- [ ] Запуск игр (exe)

### Phase 2: Core Features (4-6 недель)
- [ ] Полнотекстовый поиск (FTS5)
- [ ] Учёт времени игры (heartbeat)
- [ ] Steam library scanning
- [ ] Получение метаданных (IGDB/SteamGridDB)
- [ ] Кеширование artwork

### Phase 3: itch.io Integration (3-4 недели)
- [ ] OAuth2 авторизация
- [ ] Интеграция с Butler
- [ ] Менеджер загрузок
- [ ] Wishlist/отложенные ссылки

### Phase 4: Polish (2-3 недели)
- [ ] Локализация (RU/EN)
- [ ] Настройки приложения
- [ ] Автоматические бэкапы БД
- [ ] Системный трей
- [ ] Auto-updater

### Phase 5: Extended (по мере необходимости)
- [ ] Плагинная система (WASM)
- [ ] Дополнительные источники метаданных
- [ ] Cloud sync (опционально)

---

## 9. Зависимости Rust (Cargo.toml)

```toml
[package]
name = "ghub"
version = "0.1.0"
edition = "2024"

[dependencies]
# Tauri
tauri = { version = "2.0", features = ["devtools", "tray-icon", "protocol-asset"] }
tauri-plugin-shell = "2.0"
tauri-plugin-dialog = "2.0"
tauri-plugin-fs = "2.0"
tauri-plugin-process = "2.0"
tauri-plugin-deep-link = "2.0"

# Database
rusqlite = { version = "0.32", features = ["bundled", "backup"] }
r2d2 = "0.8"
r2d2_sqlite = "0.25"

# Async
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Utilities
uuid = { version = "1", features = ["v7"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"

# Platform-specific
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = ["Win32_Security_Cryptography"] }

[target.'cfg(target_os = "macos")'.dependencies]
security-framework = "3"

[target.'cfg(target_os = "linux")'.dependencies]
secret-service = "4"

# VDF parsing (Steam)
keyvalues-serde = "0.2"

# HTTP client (for API calls)
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
```

---

## 10. Заключение

Предлагаемый стек технологий обеспечивает:

1. **Производительность**: Tauri + Rust даёт минимальное потребление ресурсов
2. **Кроссплатформенность**: Единая кодовая база для Win/Mac/Linux
3. **Надёжность**: SQLite WAL + heartbeat система для данных
4. **Расширяемость**: Плагинная архитектура, модульный код
5. **Современный UX**: React + TailwindCSS для гибкого интерфейса

Альтернативный вариант для тех, кто предпочитает C#:
- **Avalonia UI** + **.NET 8** — хороший выбор, если команда знакома с экосистемой .NET
- Меньшее сообщество, но зрелый фреймворк
- Аналогичная архитектура данных (SQLite + FTS)
