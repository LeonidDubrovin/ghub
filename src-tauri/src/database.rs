use crate::models::{Game, Install, Setting, Space, SpaceSource};
use rusqlite::{params, Connection, Result};
use serde_json;
use std::path::Path;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            PRAGMA auto_vacuum = INCREMENTAL;
        ",
        )?;

        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            -- Spaces table
            CREATE TABLE IF NOT EXISTS spaces (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                path            TEXT,
                type            TEXT NOT NULL DEFAULT 'local',
                icon            TEXT,
                color           TEXT,
                sort_order      INTEGER DEFAULT 0,
                is_active       INTEGER DEFAULT 1,
                created_at      TEXT DEFAULT (datetime('now')),
                updated_at      TEXT DEFAULT (datetime('now'))
            );
            
            -- Space sources (watch directories)
            CREATE TABLE IF NOT EXISTS space_sources (
                space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
                source_path TEXT NOT NULL,
                is_active INTEGER DEFAULT 1,
                scan_recursively INTEGER DEFAULT 1,
                last_scanned_at TEXT,
                exclude_patterns TEXT,
                scan_status TEXT,
                scan_progress INTEGER DEFAULT 0,
                scan_total INTEGER DEFAULT 0,
                scan_error TEXT,
                scan_started_at TEXT,
                scan_completed_at TEXT,
                PRIMARY KEY (space_id, source_path)
            );
            
            CREATE INDEX IF NOT EXISTS idx_space_sources_space ON space_sources(space_id);
            CREATE INDEX IF NOT EXISTS idx_space_sources_path ON space_sources(source_path);
            
            -- Games table
            CREATE TABLE IF NOT EXISTS games (
                id              TEXT PRIMARY KEY,
                title           TEXT NOT NULL,
                sort_title      TEXT,
                description     TEXT,
                release_date    TEXT,
                developer       TEXT,
                publisher       TEXT,
                cover_image     TEXT,
                background_image TEXT,
                total_playtime_seconds INTEGER DEFAULT 0,
                last_played_at  TEXT,
                times_launched  INTEGER DEFAULT 0,
                is_favorite     INTEGER DEFAULT 0,
                is_hidden       INTEGER DEFAULT 0,
                completion_status TEXT DEFAULT 'not_played',
                user_rating     INTEGER CHECK (user_rating BETWEEN 1 AND 10),
                added_at        TEXT DEFAULT (datetime('now')),
                updated_at      TEXT DEFAULT (datetime('now')),
                fingerprint     TEXT,
                external_link   TEXT
            );
            
            -- Installs table
            CREATE TABLE IF NOT EXISTS installs (
                id              TEXT PRIMARY KEY,
                game_id         TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                space_id        TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
                install_path    TEXT NOT NULL,
                executable_path TEXT,
                launch_arguments TEXT,
                working_directory TEXT,
                status          TEXT DEFAULT 'installed',
                version         TEXT,
                install_size_bytes INTEGER,
                installed_at    TEXT DEFAULT (datetime('now')),
                UNIQUE(game_id, space_id)
            );
            
            -- Settings table
            CREATE TABLE IF NOT EXISTS settings (
                key             TEXT PRIMARY KEY,
                value           TEXT NOT NULL,
                updated_at      TEXT DEFAULT (datetime('now'))
            );
            
            -- Play sessions table
            CREATE TABLE IF NOT EXISTS play_sessions (
                id              TEXT PRIMARY KEY,
                game_id         TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                install_id      TEXT REFERENCES installs(id) ON DELETE SET NULL,
                started_at      TEXT NOT NULL,
                ended_at        TEXT,
                duration_seconds INTEGER,
                last_heartbeat_at TEXT,
                status          TEXT DEFAULT 'active',
                created_at      TEXT DEFAULT (datetime('now'))
            );
            
            -- Active sessions for heartbeat tracking
            CREATE TABLE IF NOT EXISTS active_sessions (
                id              TEXT PRIMARY KEY REFERENCES play_sessions(id) ON DELETE CASCADE,
                game_id         TEXT NOT NULL,
                process_pid     INTEGER,
                accumulated_seconds INTEGER DEFAULT 0,
                last_heartbeat  TEXT NOT NULL,
                checkpoint_at   TEXT
            );
            
            -- Download links table
            CREATE TABLE IF NOT EXISTS download_links (
                id              TEXT PRIMARY KEY,
                url             TEXT NOT NULL,
                title           TEXT NOT NULL,
                cover_url       TEXT,
                description     TEXT,
                status          TEXT DEFAULT 'pending', -- pending, downloaded, archived
                added_at        TEXT DEFAULT (datetime('now'))
            );

            -- Indexes
            CREATE INDEX IF NOT EXISTS idx_games_title ON games(title COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS idx_games_last_played ON games(last_played_at DESC);
            CREATE INDEX IF NOT EXISTS idx_installs_game ON installs(game_id);
            CREATE INDEX IF NOT EXISTS idx_installs_space ON installs(space_id);
            
            -- Default settings
            INSERT OR IGNORE INTO settings (key, value) VALUES
                ('language', '"ru"'),
                ('theme', '"dark"'),
                ('view_mode', '"grid"');
        "#,
        )?;

        // Migration: Add external_link if not exists
        match self.conn.execute("ALTER TABLE games ADD COLUMN external_link TEXT", []) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }

        // Migration: Migrate existing spaces.path to space_sources
        self.migrate_space_paths()?;

        // Migration: Add scan tracking columns to space_sources (if missing)
        match self
            .conn
            .execute("ALTER TABLE space_sources ADD COLUMN scan_status TEXT", [])
        {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }
        match self.conn.execute(
            "ALTER TABLE space_sources ADD COLUMN scan_progress INTEGER DEFAULT 0",
            [],
        ) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }
        match self.conn.execute(
            "ALTER TABLE space_sources ADD COLUMN scan_total INTEGER DEFAULT 0",
            [],
        ) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }
        match self
            .conn
            .execute("ALTER TABLE space_sources ADD COLUMN scan_error TEXT", [])
        {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }
        match self.conn.execute(
            "ALTER TABLE space_sources ADD COLUMN scan_started_at TEXT",
            [],
        ) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }
        match self.conn.execute(
            "ALTER TABLE space_sources ADD COLUMN scan_completed_at TEXT",
            [],
        ) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }

        // Migration: Add status and fingerprint to installs
        match self.conn.execute(
            "ALTER TABLE installs ADD COLUMN status TEXT DEFAULT 'installed'",
            [],
        ) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }
        match self
            .conn
            .execute("ALTER TABLE installs ADD COLUMN fingerprint TEXT", [])
        {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }

        // Migration: Create index for installs status queries
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_installs_space_status ON installs(space_id, status)",
            [],
        )?;

        Ok(())
    }

    fn migrate_space_paths(&self) -> Result<()> {
        // Create space_sources from existing spaces.path values
        // Only for spaces that have a path and don't already have a space_source entry
        self.conn.execute_batch(
            r#"
            INSERT OR IGNORE INTO space_sources (space_id, source_path, is_active, scan_recursively)
            SELECT id, path, 1, 1 
            FROM spaces 
            WHERE path IS NOT NULL AND path != '' AND 
                  id NOT IN (SELECT space_id FROM space_sources);
        "#,
        )?;

        Ok(())
    }

    // ============ SPACES ============

    pub fn get_all_spaces(&self) -> Result<Vec<Space>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, path, type, icon, color, sort_order, is_active, created_at, updated_at 
             FROM spaces WHERE is_active = 1 ORDER BY sort_order, name"
        )?;

        let spaces = stmt
            .query_map([], |row| {
                Ok(Space {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    space_type: row.get(3)?,
                    icon: row.get(4)?,
                    color: row.get(5)?,
                    sort_order: row.get(6)?,
                    is_active: row.get::<_, i32>(7)? == 1,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(spaces)
    }

    pub fn get_space_by_id(&self, id: &str) -> Result<Space> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, path, type, icon, color, sort_order, is_active, created_at, updated_at 
             FROM spaces WHERE id = ?"
        )?;

        stmt.query_row([id], |row| {
            Ok(Space {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                space_type: row.get(3)?,
                icon: row.get(4)?,
                color: row.get(5)?,
                sort_order: row.get(6)?,
                is_active: row.get::<_, i32>(7)? == 1,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
    }

    pub fn create_space(
        &self,
        id: &str,
        name: &str,
        path: Option<&str>,
        space_type: &str,
        icon: Option<&str>,
        color: Option<&str>,
    ) -> Result<Space> {
        self.conn.execute(
            "INSERT INTO spaces (id, name, path, type, icon, color) VALUES (?, ?, ?, ?, ?, ?)",
            params![id, name, path, space_type, icon, color],
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT id, name, path, type, icon, color, sort_order, is_active, created_at, updated_at FROM spaces WHERE id = ?"
        )?;

        stmt.query_row([id], |row| {
            Ok(Space {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                space_type: row.get(3)?,
                icon: row.get(4)?,
                color: row.get(5)?,
                sort_order: row.get(6)?,
                is_active: row.get::<_, i32>(7)? == 1,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
    }

    pub fn delete_space(&self, id: &str) -> Result<()> {
        self.conn
            .execute("UPDATE spaces SET is_active = 0 WHERE id = ?", [id])?;
        Ok(())
    }

    // ============ SPACE SOURCES ============

    pub fn get_space_sources(&self, space_id: &str) -> Result<Vec<SpaceSource>> {
        let mut stmt = self.conn.prepare(
            "SELECT space_id, source_path, is_active, scan_recursively, last_scanned_at, exclude_patterns,
                    scan_status, scan_progress, scan_total, scan_error, scan_started_at, scan_completed_at
             FROM space_sources 
             WHERE space_id = ? 
             ORDER BY source_path"
        )?;

        let sources = stmt
            .query_map([space_id], |row| {
                let patterns_json: Option<String> = row.get(5)?;
                let exclude_patterns: Option<Vec<String>> = patterns_json
                    .as_ref()
                    .and_then(|json| serde_json::from_str(json).ok());

                Ok(SpaceSource {
                    space_id: row.get(0)?,
                    source_path: row.get(1)?,
                    is_active: row.get::<_, i32>(2)? == 1,
                    scan_recursively: row.get::<_, i32>(3)? == 1,
                    last_scanned_at: row.get(4)?,
                    exclude_patterns,
                    scan_status: row.get(6)?,
                    scan_progress: row.get(7)?,
                    scan_total: row.get(8)?,
                    scan_error: row.get(9)?,
                    scan_started_at: row.get(10)?,
                    scan_completed_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(sources)
    }

    pub fn add_space_source(
        &self,
        space_id: &str,
        source_path: &str,
        scan_recursively: bool,
    ) -> Result<()> {
        // Serialize empty vector as JSON for exclude_patterns
        let exclude_patterns_json = serde_json::to_string(&Vec::<String>::new())
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        self.conn.execute(
            "INSERT OR REPLACE INTO space_sources (space_id, source_path, is_active, scan_recursively, exclude_patterns) VALUES (?, ?, 1, ?, ?)",
            params![space_id, source_path, scan_recursively as i32, exclude_patterns_json]
        )?;
        Ok(())
    }

    pub fn remove_space_source(&self, space_id: &str, source_path: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM space_sources WHERE space_id = ? AND source_path = ?",
            params![space_id, source_path],
        )?;
        Ok(())
    }

    pub fn update_space_source(
        &self,
        space_id: &str,
        source_path: &str,
        is_active: bool,
        scan_recursively: Option<bool>,
    ) -> Result<()> {
        if let Some(rec) = scan_recursively {
            self.conn.execute(
                "UPDATE space_sources SET is_active = ?, scan_recursively = ? WHERE space_id = ? AND source_path = ?",
                params![is_active as i32, rec as i32, space_id, source_path]
            )?;
        } else {
            self.conn.execute(
                "UPDATE space_sources SET is_active = ? WHERE space_id = ? AND source_path = ?",
                params![is_active as i32, space_id, source_path],
            )?;
        }
        Ok(())
    }

    pub fn get_all_active_sources(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT space_id, source_path FROM space_sources WHERE is_active = 1")?;

        let sources = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>>>()?;

        Ok(sources)
    }

    pub fn get_active_sources_for_space(&self, space_id: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT space_id, source_path FROM space_sources WHERE space_id = ? AND is_active = 1",
        )?;

        let sources = stmt
            .query_map([space_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>>>()?;

        Ok(sources)
    }

    // ============ GAMES ============

    pub fn get_all_games(&self) -> Result<Vec<Game>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id, g.title, g.sort_title, g.description, g.release_date, g.developer, g.publisher,
                    g.cover_image, g.background_image, g.total_playtime_seconds, g.last_played_at,
                    g.times_launched, g.is_favorite, g.is_hidden, g.completion_status, g.user_rating,
                    g.added_at, g.updated_at, g.external_link,
                    i.space_id, s.name as space_name, s.type as space_type,
                    i.install_path, i.executable_path, i.status, i.fingerprint
             FROM games g
             LEFT JOIN installs i ON g.id = i.game_id
             LEFT JOIN spaces s ON i.space_id = s.id
             WHERE g.is_hidden = 0
             ORDER BY g.title COLLATE NOCASE"
        )?;

        let games = stmt
            .query_map([], |row| {
                Ok(Game {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    sort_title: row.get(2)?,
                    description: row.get(3)?,
                    release_date: row.get(4)?,
                    developer: row.get(5)?,
                    publisher: row.get(6)?,
                    cover_image: row.get(7)?,
                    background_image: row.get(8)?,
                    total_playtime_seconds: row.get(9)?,
                    last_played_at: row.get(10)?,
                    times_launched: row.get(11)?,
                    is_favorite: row.get::<_, i32>(12)? == 1,
                    is_hidden: row.get::<_, i32>(13)? == 1,
                    completion_status: row.get(14)?,
                    user_rating: row.get(15)?,
                    added_at: row.get(16)?,
                    updated_at: row.get(17)?,
                    external_link: row.get(18).ok(),
                    space_id: row.get(19).ok(),
                    space_name: row.get(20).ok(),
                    space_type: row.get(21).ok(),
                    install_path: row.get(22).ok(),
                    executable_path: row.get(23).ok(),
                    install_status: row.get(24).ok(),
                    install_fingerprint: row.get(25).ok(),
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(games)
    }

    pub fn get_games_by_space(&self, space_id: &str) -> Result<Vec<Game>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id, g.title, g.sort_title, g.description, g.release_date, g.developer, g.publisher,
                    g.cover_image, g.background_image, g.total_playtime_seconds, g.last_played_at,
                    g.times_launched, g.is_favorite, g.is_hidden, g.completion_status, g.user_rating,
                    g.added_at, g.updated_at, g.external_link,
                    i.space_id, s.name as space_name, s.type as space_type,
                    i.install_path, i.executable_path, i.status, i.fingerprint
             FROM games g
             JOIN installs i ON g.id = i.game_id
             LEFT JOIN spaces s ON i.space_id = s.id
             WHERE i.space_id = ? AND g.is_hidden = 0
             ORDER BY g.title COLLATE NOCASE"
        )?;

        let games = stmt
            .query_map([space_id], |row| {
                Ok(Game {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    sort_title: row.get(2)?,
                    description: row.get(3)?,
                    release_date: row.get(4)?,
                    developer: row.get(5)?,
                    publisher: row.get(6)?,
                    cover_image: row.get(7)?,
                    background_image: row.get(8)?,
                    total_playtime_seconds: row.get(9)?,
                    last_played_at: row.get(10)?,
                    times_launched: row.get(11)?,
                    is_favorite: row.get::<_, i32>(12)? == 1,
                    is_hidden: row.get::<_, i32>(13)? == 1,
                    completion_status: row.get(14)?,
                    user_rating: row.get(15)?,
                    added_at: row.get(16)?,
                    updated_at: row.get(17)?,
                    external_link: row.get(18).ok(),
                    space_id: row.get(19).ok(),
                    space_name: row.get(20).ok(),
                    space_type: row.get(21).ok(),
                    install_path: row.get(22).ok(),
                    executable_path: row.get(23).ok(),
                    install_status: row.get(24).ok(),
                    install_fingerprint: row.get(25).ok(),
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(games)
    }

    pub fn create_game(
        &self,
        id: &str,
        title: &str,
        description: Option<&str>,
        developer: Option<&str>,
        cover_image: Option<&str>,
        external_link: Option<&str>,
    ) -> Result<Game> {
        let fingerprint = format!("{}-{}", title.to_lowercase(), developer.unwrap_or(""));

        self.conn.execute(
            "INSERT INTO games (id, title, description, developer, cover_image, fingerprint, external_link) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![id, title, description, developer, cover_image, fingerprint, external_link]
        )?;

        self.get_game_by_id(id)
    }

    pub fn get_game_by_id(&self, id: &str) -> Result<Game> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id, g.title, g.sort_title, g.description, g.release_date, g.developer, g.publisher,
                    g.cover_image, g.background_image, g.total_playtime_seconds, g.last_played_at,
                    g.times_launched, g.is_favorite, g.is_hidden, g.completion_status, g.user_rating,
                    g.added_at, g.updated_at, g.external_link,
                    i.space_id, s.name as space_name, s.type as space_type,
                    i.install_path, i.executable_path, i.status, i.fingerprint
             FROM games g
             LEFT JOIN installs i ON g.id = i.game_id
             LEFT JOIN spaces s ON i.space_id = s.id
             WHERE g.id = ?"
        )?;

        stmt.query_row([id], |row| {
            Ok(Game {
                id: row.get(0)?,
                title: row.get(1)?,
                sort_title: row.get(2)?,
                description: row.get(3)?,
                release_date: row.get(4)?,
                developer: row.get(5)?,
                publisher: row.get(6)?,
                cover_image: row.get(7)?,
                background_image: row.get(8)?,
                total_playtime_seconds: row.get(9)?,
                last_played_at: row.get(10)?,
                times_launched: row.get(11)?,
                is_favorite: row.get::<_, i32>(12)? == 1,
                is_hidden: row.get::<_, i32>(13)? == 1,
                completion_status: row.get(14)?,
                user_rating: row.get(15)?,
                added_at: row.get(16)?,
                updated_at: row.get(17)?,
                external_link: row.get(18).ok(),
                space_id: row.get(19).ok(),
                space_name: row.get(20).ok(),
                space_type: row.get(21).ok(),
                install_path: row.get(22).ok(),
                executable_path: row.get(23).ok(),
                install_status: row.get(24).ok(),
                install_fingerprint: row.get(25).ok(),
            })
        })
    }

    /// Find a game by its fingerprint (title + developer)
    /// Returns the first match if found (case-insensitive)
    pub fn get_game_by_fingerprint(
        &self,
        title: &str,
        developer: Option<&str>,
    ) -> Result<Option<Game>> {
        // Build query: match title (case-insensitive) and developer if provided
        let mut stmt = if let Some(_dev) = developer {
            self.conn.prepare(
                "SELECT g.id, g.title, g.sort_title, g.description, g.release_date, g.developer, g.publisher,
                        g.cover_image, g.background_image, g.total_playtime_seconds, g.last_played_at,
                        g.times_launched, g.is_favorite, g.is_hidden, g.completion_status, g.user_rating,
                        g.added_at, g.updated_at, g.external_link
                 FROM games g
                 WHERE g.title = ? COLLATE NOCASE AND g.developer = ? COLLATE NOCASE
                 LIMIT 1"
            )?
        } else {
            self.conn.prepare(
                "SELECT g.id, g.title, g.sort_title, g.description, g.release_date, g.developer, g.publisher,
                        g.cover_image, g.background_image, g.total_playtime_seconds, g.last_played_at,
                        g.times_launched, g.is_favorite, g.is_hidden, g.completion_status, g.user_rating,
                        g.added_at, g.updated_at, g.external_link
                 FROM games g
                 WHERE g.title = ? COLLATE NOCASE
                 LIMIT 1"
            )?
        };

        let result = if let Some(dev) = developer {
            stmt.query_row(params![title, dev], |row| {
                Ok(Game {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    sort_title: row.get(2)?,
                    description: row.get(3)?,
                    release_date: row.get(4)?,
                    developer: row.get(5)?,
                    publisher: row.get(6)?,
                    cover_image: row.get(7)?,
                    background_image: row.get(8)?,
                    total_playtime_seconds: row.get(9)?,
                    last_played_at: row.get(10)?,
                    times_launched: row.get(11)?,
                    is_favorite: row.get::<_, i32>(12)? == 1,
                    is_hidden: row.get::<_, i32>(13)? == 1,
                    completion_status: row.get(14)?,
                    user_rating: row.get(15)?,
                    added_at: row.get(16)?,
                    updated_at: row.get(17)?,
                    external_link: row.get(18).ok(),
                    space_id: None,
                    space_name: None,
                    space_type: None,
                    install_path: None,
                    executable_path: None,
                    install_status: None,
                    install_fingerprint: None,
                })
            })
        } else {
            stmt.query_row([title], |row| {
                Ok(Game {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    sort_title: row.get(2)?,
                    description: row.get(3)?,
                    release_date: row.get(4)?,
                    developer: row.get(5)?,
                    publisher: row.get(6)?,
                    cover_image: row.get(7)?,
                    background_image: row.get(8)?,
                    total_playtime_seconds: row.get(9)?,
                    last_played_at: row.get(10)?,
                    times_launched: row.get(11)?,
                    is_favorite: row.get::<_, i32>(12)? == 1,
                    is_hidden: row.get::<_, i32>(13)? == 1,
                    completion_status: row.get(14)?,
                    user_rating: row.get(15)?,
                    added_at: row.get(16)?,
                    updated_at: row.get(17)?,
                    external_link: row.get(18).ok(),
                    space_id: None,
                    space_name: None,
                    space_type: None,
                    install_path: None,
                    executable_path: None,
                    install_status: None,
                    install_fingerprint: None,
                })
            })
        };

        match result {
            Ok(game) => Ok(Some(game)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn update_game(
        &self,
        id: &str,
        title: Option<&str>,
        description: Option<&str>,
        developer: Option<&str>,
        publisher: Option<&str>,
        cover_image: Option<&str>,
        is_favorite: Option<bool>,
        completion_status: Option<&str>,
        user_rating: Option<i32>,
    ) -> Result<()> {
        if let Some(t) = title {
            self.conn.execute(
                "UPDATE games SET title = ?, updated_at = datetime('now') WHERE id = ?",
                params![t, id],
            )?;
        }
        if let Some(d) = description {
            self.conn.execute(
                "UPDATE games SET description = ?, updated_at = datetime('now') WHERE id = ?",
                params![d, id],
            )?;
        }
        if let Some(dev) = developer {
            self.conn.execute(
                "UPDATE games SET developer = ?, updated_at = datetime('now') WHERE id = ?",
                params![dev, id],
            )?;
        }
        if let Some(pub_) = publisher {
            self.conn.execute(
                "UPDATE games SET publisher = ?, updated_at = datetime('now') WHERE id = ?",
                params![pub_, id],
            )?;
        }
        if let Some(cover) = cover_image {
            self.conn.execute(
                "UPDATE games SET cover_image = ?, updated_at = datetime('now') WHERE id = ?",
                params![cover, id],
            )?;
        }
        if let Some(f) = is_favorite {
            self.conn.execute(
                "UPDATE games SET is_favorite = ?, updated_at = datetime('now') WHERE id = ?",
                params![f as i32, id],
            )?;
        }
        if let Some(s) = completion_status {
            self.conn.execute(
                "UPDATE games SET completion_status = ?, updated_at = datetime('now') WHERE id = ?",
                params![s, id],
            )?;
        }
        if let Some(rating) = user_rating {
            self.conn.execute(
                "UPDATE games SET user_rating = ?, updated_at = datetime('now') WHERE id = ?",
                params![rating, id],
            )?;
        }
        Ok(())
    }

    /// Update game fields, resetting to NULL when None is passed (for refresh operations)
    pub fn update_game_with_reset(
        &self,
        id: &str,
        title: Option<&str>,
        description: Option<&str>,
        developer: Option<&str>,
        publisher: Option<&str>,
        cover_image: Option<&str>,
        is_favorite: Option<bool>,
        completion_status: Option<&str>,
        user_rating: Option<i32>,
    ) -> Result<()> {
        // For fields that should be reset to NULL when None, use COALESCE or direct NULL assignment
        let title_val: Option<&str> = title;
        let desc_val: Option<&str> = description;
        let dev_val: Option<&str> = developer;
        let pub_val: Option<&str> = publisher;
        let cover_val: Option<&str> = cover_image;

        self.conn.execute(
            "UPDATE games SET
                title = COALESCE(?1, title),
                description = ?2,
                developer = ?3,
                publisher = ?4,
                cover_image = ?5,
                is_favorite = COALESCE(?6, is_favorite),
                completion_status = COALESCE(?7, completion_status),
                user_rating = COALESCE(?8, user_rating),
                updated_at = datetime('now')
             WHERE id = ?9",
            params![
                title_val,
                desc_val,
                dev_val,
                pub_val,
                cover_val,
                is_favorite.map(|b| b as i32),
                completion_status,
                user_rating,
                id
            ],
        )?;
        Ok(())
    }

    pub fn delete_game(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM games WHERE id = ?", [id])?;
        Ok(())
    }

    // ============ INSTALLS ============

    pub fn create_install(
        &self,
        id: &str,
        game_id: &str,
        space_id: &str,
        install_path: &str,
        executable_path: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO installs (id, game_id, space_id, install_path, executable_path) VALUES (?, ?, ?, ?, ?)",
            params![id, game_id, space_id, install_path, executable_path]
        )?;
        Ok(())
    }

    pub fn get_install(&self, game_id: &str, space_id: &str) -> Result<Option<Install>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, game_id, space_id, install_path, executable_path, launch_arguments, 
                    working_directory, status, version, install_size_bytes, installed_at, fingerprint
             FROM installs
             WHERE game_id = ? AND space_id = ?"
        )?;

        let result = stmt.query_row(params![game_id, space_id], |row| {
            Ok(Install {
                id: row.get(0)?,
                game_id: row.get(1)?,
                space_id: row.get(2)?,
                install_path: row.get(3)?,
                executable_path: row.get(4)?,
                launch_arguments: row.get(5)?,
                working_directory: row.get(6)?,
                status: row.get(7)?,
                version: row.get(8)?,
                install_size_bytes: row.get(9)?,
                installed_at: row.get(10)?,
                fingerprint: row.get(11)?,
            })
        });

        match result {
            Ok(install) => Ok(Some(install)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_installs_for_game(&self, game_id: &str) -> Result<Vec<Install>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, game_id, space_id, install_path, executable_path, launch_arguments, 
                    working_directory, status, version, install_size_bytes, installed_at, fingerprint
             FROM installs WHERE game_id = ?"
        )?;

        let installs = stmt
            .query_map([game_id], |row| {
                Ok(Install {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    space_id: row.get(2)?,
                    install_path: row.get(3)?,
                    executable_path: row.get(4)?,
                    launch_arguments: row.get(5)?,
                    working_directory: row.get(6)?,
                    status: row.get(7)?,
                    version: row.get(8)?,
                    install_size_bytes: row.get(9)?,
                    installed_at: row.get(10)?,
                    fingerprint: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(installs)
    }

    pub fn update_install_executable(&self, install_id: &str, executable_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE installs SET executable_path = ? WHERE id = ?",
            params![executable_path, install_id],
        )?;
        Ok(())
    }

    // ============ SETTINGS ============

    pub fn get_settings(&self) -> Result<Vec<Setting>> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let settings = stmt
            .query_map([], |row| {
                Ok(Setting {
                    key: row.get(0)?,
                    value: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(settings)
    }

    pub fn update_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))",
            params![key, value]
        )?;
        Ok(())
    }

    // ============ PLAYTIME TRACKING ============

    pub fn create_play_session(
        &self,
        id: &str,
        game_id: &str,
        install_id: Option<&str>,
        started_at: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO play_sessions (id, game_id, install_id, started_at, status) VALUES (?, ?, ?, ?, 'active')",
            params![id, game_id, install_id, started_at]
        )?;
        Ok(())
    }

    pub fn create_active_session(
        &self,
        id: &str,
        game_id: &str,
        pid: u32,
        last_heartbeat: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO active_sessions (id, game_id, process_pid, accumulated_seconds, last_heartbeat) 
             VALUES (?, ?, ?, 0, ?)",
            params![id, game_id, pid, last_heartbeat]
        )?;
        Ok(())
    }

    pub fn update_active_session_heartbeat(
        &self,
        id: &str,
        accumulated: i64,
        heartbeat: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE active_sessions SET accumulated_seconds = ?, last_heartbeat = ? WHERE id = ?",
            params![accumulated, heartbeat, id],
        )?;
        Ok(())
    }

    pub fn update_active_session_checkpoint(&self, id: &str, checkpoint_at: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE active_sessions SET checkpoint_at = ?, accumulated_seconds = 0 WHERE id = ?",
            params![checkpoint_at, id],
        )?;
        Ok(())
    }

    pub fn checkpoint_session(&self, id: &str, duration: i64, heartbeat: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE play_sessions SET duration_seconds = COALESCE(duration_seconds, 0) + ?, last_heartbeat_at = ? WHERE id = ?",
            params![duration, heartbeat, id]
        )?;
        Ok(())
    }

    pub fn complete_session(&self, id: &str, ended_at: &str, duration: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE play_sessions SET ended_at = ?, duration_seconds = ?, status = 'completed' WHERE id = ?",
            params![ended_at, duration, id]
        )?;
        Ok(())
    }

    pub fn delete_active_session(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM active_sessions WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn add_playtime(&self, game_id: &str, seconds: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET total_playtime_seconds = total_playtime_seconds + ?, 
             times_launched = times_launched + 1,
             updated_at = datetime('now')
             WHERE id = ?",
            params![seconds, game_id],
        )?;
        Ok(())
    }

    pub fn update_last_played(&self, game_id: &str, last_played: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET last_played_at = ?, updated_at = datetime('now') WHERE id = ?",
            params![last_played, game_id],
        )?;
        Ok(())
    }

    pub fn get_active_sessions(&self) -> Result<Vec<ActiveSessionRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, game_id, accumulated_seconds FROM active_sessions")?;

        let sessions = stmt
            .query_map([], |row| {
                Ok(ActiveSessionRow {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    accumulated_seconds: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(sessions)
    }

    pub fn recover_session(&self, id: &str, accumulated: i64, now: &str) -> Result<()> {
        // Add remaining time to game
        let mut stmt = self
            .conn
            .prepare("SELECT game_id FROM active_sessions WHERE id = ?")?;
        let game_id: String = stmt.query_row([id], |row| row.get(0))?;

        self.add_playtime(&game_id, accumulated)?;

        // Mark play_session as recovered
        self.conn.execute(
            "UPDATE play_sessions SET status = 'recovered', ended_at = ? WHERE id = ?",
            params![now, id],
        )?;

        // Remove active session
        self.delete_active_session(id)?;

        Ok(())
    }

    // ============ DOWNLOAD LINKS ============

    pub fn get_download_links(&self) -> Result<Vec<crate::models::DownloadLink>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, cover_url, description, status, added_at FROM download_links ORDER BY added_at DESC"
        )?;

        let links = stmt
            .query_map([], |row| {
                Ok(crate::models::DownloadLink {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    cover_url: row.get(3)?,
                    description: row.get(4)?,
                    status: row.get(5)?,
                    added_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(links)
    }

    pub fn create_download_link(
        &self,
        url: &str,
        title: &str,
        cover_url: Option<&str>,
        description: Option<&str>,
    ) -> Result<crate::models::DownloadLink> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO download_links (id, url, title, cover_url, description) VALUES (?, ?, ?, ?, ?)",
            params![id, url, title, cover_url, description]
        )?;

        Ok(crate::models::DownloadLink {
            id,
            url: url.to_string(),
            title: title.to_string(),
            cover_url: cover_url.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            status: "pending".to_string(),
            added_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }

    pub fn delete_download_link(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM download_links WHERE id = ?", [id])?;
        Ok(())
    }

    // ============ SPACE SOURCE SCAN STATUS ============

    /// Set scan status for a source
    pub fn set_source_scan_status(
        &self,
        space_id: &str,
        source_path: &str,
        status: Option<&str>,
        progress: Option<i32>,
        total: Option<i32>,
        error: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        if status == Some("scanning") {
            self.conn.execute(
                "UPDATE space_sources SET scan_status = ?, scan_progress = ?, scan_total = ?, scan_error = ?, scan_started_at = ?, scan_completed_at = NULL WHERE space_id = ? AND source_path = ?",
                params![status, progress, total, error, now, space_id, source_path]
            )?;
        } else if let Some(st) = status {
            self.conn.execute(
                "UPDATE space_sources SET scan_status = ?, scan_progress = ?, scan_total = ?, scan_error = ?, scan_completed_at = ? WHERE space_id = ? AND source_path = ?",
                params![st, progress, total, error, now, space_id, source_path]
            )?;
        } else {
            // Clear status
            self.conn.execute(
                "UPDATE space_sources SET scan_status = NULL, scan_progress = 0, scan_total = 0, scan_error = NULL, scan_started_at = NULL, scan_completed_at = NULL WHERE space_id = ? AND source_path = ?",
                params![space_id, source_path]
            )?;
        }
        Ok(())
    }

    /// Get scan status for a specific source
    pub fn get_source_scan_status(
        &self,
        space_id: &str,
        source_path: &str,
    ) -> Result<Option<SpaceSource>> {
        let mut stmt = self.conn.prepare(
            "SELECT space_id, source_path, is_active, scan_recursively, last_scanned_at, exclude_patterns,
                    scan_status, scan_progress, scan_total, scan_error, scan_started_at, scan_completed_at
             FROM space_sources
             WHERE space_id = ? AND source_path = ?"
        )?;

        let result = stmt.query_row(params![space_id, source_path], |row| {
            let patterns_json: Option<String> = row.get(5)?;
            let exclude_patterns: Option<Vec<String>> = patterns_json
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok());

            Ok(SpaceSource {
                space_id: row.get(0)?,
                source_path: row.get(1)?,
                is_active: row.get::<_, i32>(2)? == 1,
                scan_recursively: row.get::<_, i32>(3)? == 1,
                last_scanned_at: row.get(4)?,
                exclude_patterns,
                scan_status: row.get(6)?,
                scan_progress: row.get(7)?,
                scan_total: row.get(8)?,
                scan_error: row.get(9)?,
                scan_started_at: row.get(10)?,
                scan_completed_at: row.get(11)?,
            })
        });

        match result {
            Ok(source) => Ok(Some(source)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Update last_scanned_at timestamp for a source
    pub fn update_source_last_scanned(&self, space_id: &str, source_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE space_sources SET last_scanned_at = datetime('now') WHERE space_id = ? AND source_path = ?",
            params![space_id, source_path],
        )?;
        Ok(())
    }

    /// Get install by path within a space
    pub fn get_install_by_path(
        &self,
        space_id: &str,
        install_path: &str,
    ) -> Result<Option<Install>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, game_id, space_id, install_path, executable_path, launch_arguments,
                    working_directory, status, version, install_size_bytes, installed_at, fingerprint
             FROM installs
             WHERE space_id = ? AND install_path = ?"
        )?;

        let result = stmt.query_row(params![space_id, install_path], |row| {
            Ok(Install {
                id: row.get(0)?,
                game_id: row.get(1)?,
                space_id: row.get(2)?,
                install_path: row.get(3)?,
                executable_path: row.get(4)?,
                launch_arguments: row.get(5)?,
                working_directory: row.get(6)?,
                status: row.get(7)?,
                version: row.get(8)?,
                install_size_bytes: row.get(9)?,
                installed_at: row.get(10)?,
                fingerprint: row.get(11)?,
            })
        });

        match result {
            Ok(install) => Ok(Some(install)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Update install status and fingerprint
    pub fn update_install_status(&self, install_id: &str, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE installs SET status = ?, updated_at = datetime('now') WHERE id = ?",
            params![status, install_id],
        )?;
        Ok(())
    }

    /// Update install fingerprint
    pub fn update_install_fingerprint(&self, install_id: &str, fingerprint: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE installs SET fingerprint = ?, updated_at = datetime('now') WHERE id = ?",
            params![fingerprint, install_id],
        )?;
        Ok(())
    }

    /// Update both status and fingerprint
    pub fn update_install(
        &self,
        install_id: &str,
        status: &str,
        fingerprint: Option<&str>,
    ) -> Result<()> {
        if let Some(fp) = fingerprint {
            self.conn.execute(
                "UPDATE installs SET status = ?, fingerprint = ?, updated_at = datetime('now') WHERE id = ?",
                params![status, fp, install_id]
            )?;
        } else {
            self.conn.execute(
                "UPDATE installs SET status = ?, updated_at = datetime('now') WHERE id = ?",
                params![status, install_id],
            )?;
        }
        Ok(())
    }

    /// Get all installs for a specific source (by path prefix)
    /// This finds all installs where install_path starts with the source_path
    /// Uses range query (>= and <) for reliable prefix matching across platforms
    pub fn get_installs_for_source(
        &self,
        space_id: &str,
        source_path: &str,
    ) -> Result<Vec<Install>> {
        // Build prefix that ensures we only match subdirectories of source_path
        // We want install_path that starts with "source_path" followed by a separator
        let prefix = if source_path.ends_with('/') || source_path.ends_with('\\') {
            source_path.to_string()
        } else {
            format!("{}{}", source_path, std::path::MAIN_SEPARATOR)
        };
        
        // Use range query: >= prefix and < prefix + highest possible char
        // This is equivalent to "install_path LIKE 'prefix%'" but handles
        // backslashes correctly on Windows and is case-insensitive if needed
        let prefix_end = format!("{}~", prefix); // '~' is high ASCII, works for prefix range
        
        let mut stmt = self.conn.prepare(
            "SELECT id, game_id, space_id, install_path, executable_path, launch_arguments,
                    working_directory, status, version, install_size_bytes, installed_at, fingerprint
             FROM installs
             WHERE space_id = ? AND install_path >= ? AND install_path < ?"
        )?;

        let installs = stmt
            .query_map(params![space_id, prefix, prefix_end], |row| {
                Ok(Install {
                    id: row.get(0)?,
                    game_id: row.get(1)?,
                    space_id: row.get(2)?,
                    install_path: row.get(3)?,
                    executable_path: row.get(4)?,
                    launch_arguments: row.get(5)?,
                    working_directory: row.get(6)?,
                    status: row.get(7)?,
                    version: row.get(8)?,
                    install_size_bytes: row.get(9)?,
                    installed_at: row.get(10)?,
                    fingerprint: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(installs)
    }
}

#[derive(Debug)]
pub struct ActiveSessionRow {
    pub id: String,
    pub game_id: String,
    pub accumulated_seconds: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a temporary test database
    fn create_test_db() -> Result<Database> {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_ghub.db");
        // Clean up if exists
        let _ = fs::remove_file(&db_path);
        let db = Database::new(&db_path)?;
        Ok(db)
    }

    #[test]
    fn test_get_installs_for_source_range_query() {
        let db = create_test_db().expect("Failed to create test db");

        // Create a space
        let space_id = "test_space".to_string();
        let source_path = "C:\\Games\\"; // Windows path with trailing separator

        // Insert test installs with various paths
        let installs = vec![
            ("C:\\Games\\Game1", "inst1"),
            ("C:\\Games\\Game1\\SubDir", "inst2"),
            ("C:\\Games\\Game2", "inst3"),
            ("C:\\GamesOther", "inst4"), // Should NOT match (not under source)
            ("D:\\Games\\Game3", "inst5"), // Should NOT match (different drive)
        ];

        for (path, id) in installs {
            let game_id = format!("game_{}", id);
            // Insert game first
            let _ = db.conn.execute(
                "INSERT INTO games (id, title) VALUES (?, ?)",
                params![game_id, id],
            );
            // Insert install
            let _ = db.conn.execute(
                "INSERT INTO installs (id, game_id, space_id, install_path, status) VALUES (?, ?, ?, ?, ?)",
                params![id, game_id, space_id, path, "installed"],
            );
        }

        // Query installs for source
        let result = db.get_installs_for_source(&space_id, source_path).unwrap();

        // Should return only installs under C:\Games\
        assert_eq!(result.len(), 3, "Should find 3 installs under source path");

        let paths: Vec<&str> = result.iter().map(|i| i.install_path.as_str()).collect();
        assert!(paths.contains(&"C:\\Games\\Game1"));
        assert!(paths.contains(&"C:\\Games\\Game1\\SubDir"));
        assert!(paths.contains(&"C:\\Games\\Game2"));
        assert!(!paths.contains(&"C:\\GamesOther"));
        assert!(!paths.contains(&"D:\\Games\\Game3"));
    }

    #[test]
    fn test_get_installs_for_source_unix_paths() {
        let db = create_test_db().expect("Failed to create test db");

        let space_id = "test_space_unix".to_string();
        let source_path = "/home/games/"; // Unix path with trailing separator

        let installs = vec![
            ("/home/games/Game1", "inst1"),
            ("/home/games/Game1/subdir", "inst2"),
            ("/home/games/Game2", "inst3"),
            ("/home/games_other", "inst4"), // Should NOT match
            ("/var/games/Game3", "inst5"), // Should NOT match
        ];

        for (path, id) in installs {
            let game_id = format!("game_{}", id);
            let _ = db.conn.execute(
                "INSERT INTO games (id, title) VALUES (?, ?)",
                params![game_id, id],
            );
            let _ = db.conn.execute(
                "INSERT INTO installs (id, game_id, space_id, install_path, status) VALUES (?, ?, ?, ?, ?)",
                params![id, game_id, space_id, path, "installed"],
            );
        }

        let result = db.get_installs_for_source(&space_id, source_path).unwrap();

        assert_eq!(result.len(), 3);
        let paths: Vec<&str> = result.iter().map(|i| i.install_path.as_str()).collect();
        assert!(paths.contains(&"/home/games/Game1"));
        assert!(paths.contains(&"/home/games/Game1/subdir"));
        assert!(paths.contains(&"/home/games/Game2"));
        assert!(!paths.contains(&"/home/games_other"));
        assert!(!paths.contains(&"/var/games/Game3"));
    }

    #[test]
    fn test_get_game_by_fingerprint() {
        let db = create_test_db().expect("Failed to create test db");

        // Insert test games
        let game1_id = "game1".to_string();
        let game2_id = "game2".to_string();
        let game3_id = "game3".to_string();

        let _ = db.conn.execute(
            "INSERT INTO games (id, title, developer) VALUES (?, ?, ?)",
            params![game1_id, "My Game", "MyDev"],
        );
        let _ = db.conn.execute(
            "INSERT INTO games (id, title, developer) VALUES (?, ?, ?)",
            params![game2_id, "My Game", "OtherDev"],
        );
        let _ = db.conn.execute(
            "INSERT INTO games (id, title, developer) VALUES (?, ?, ?)",
            params![game3_id, "My Game", None::<String>],
        );

        // Test 1: Match with both title and developer (case-insensitive)
        let result = db.get_game_by_fingerprint("my game", Some("mydev")).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, game1_id);

        // Test 2: Match with title only (should return first match)
        let result = db.get_game_by_fingerprint("my game", None).unwrap();
        assert!(result.is_some());
        // Should return first game with that title (game1)
        assert_eq!(result.unwrap().id, game1_id);

        // Test 3: Different developer returns None
        let result = db.get_game_by_fingerprint("my game", Some("otherdev")).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, game2_id);

        // Test 4: Non-existent game returns None
        let result = db.get_game_by_fingerprint("nonexistent", None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_game_by_fingerprint_case_insensitive() {
        let db = create_test_db().expect("Failed to create test db");

        let game_id = "game1".to_string();
        let _ = db.conn.execute(
            "INSERT INTO games (id, title, developer) VALUES (?, ?, ?)",
            params![game_id, "MyGame", "MyDev"],
        );

        // Test various case combinations
        let cases = vec![
            ("mygame", Some("mydev")),
            ("MYGAME", Some("MYDEV")),
            ("MyGame", Some("MyDev")),
            ("mYgAmE", Some("mYdEv")),
        ];

        for (title, dev) in cases {
            let result = db.get_game_by_fingerprint(title, dev).unwrap();
            assert!(result.is_some(), "Should match case-insensitively for {:?}", (title, dev));
            assert_eq!(result.unwrap().id, game_id);
        }
    }

    #[test]
    fn test_get_installs_for_source_path_edge_cases() {
        let db = create_test_db().expect("Failed to create test db");

        // Test with path without trailing separator
        let space_id = "test_edge".to_string();
        let source_path = "C:\\Games"; // No trailing separator

        let installs = vec![
            ("C:\\Games\\Game1", "inst1"),
            ("C:\\Games\\Game1\\SubDir", "inst2"),
            ("C:\\GamesBeta\\Game3", "inst3"), // Should NOT match (prefix but not subdirectory)
        ];

        for (path, id) in installs {
            let game_id = format!("game_{}", id);
            let _ = db.conn.execute(
                "INSERT INTO games (id, title) VALUES (?, ?)",
                params![game_id, id],
            );
            let _ = db.conn.execute(
                "INSERT INTO installs (id, game_id, space_id, install_path, status) VALUES (?, ?, ?, ?, ?)",
                params![id, game_id, space_id, path, "installed"],
            );
        }

        let result = db.get_installs_for_source(&space_id, source_path).unwrap();

        // Should only match paths that have a separator after the prefix
        assert_eq!(result.len(), 2);
        let paths: Vec<&str> = result.iter().map(|i| i.install_path.as_str()).collect();
        assert!(paths.contains(&"C:\\Games\\Game1"));
        assert!(paths.contains(&"C:\\Games\\Game1\\SubDir"));
        assert!(!paths.contains(&"C:\\GamesBeta\\Game3"));
    }
}

