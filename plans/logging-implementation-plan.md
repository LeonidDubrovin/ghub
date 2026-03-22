# Logging Implementation Plan

## Current State Analysis

### Backend (Rust)
- Uses `env_logger` with `log` crate
- Logger initialized in `src-tauri/src/lib.rs` with `filter_level(log::LevelFilter::Debug)`
- **Issues:**
  - Inconsistent: mix of `log::debug/info/warn/error` and `println!` macros
  - No file-based logging (console only)
  - No log rotation
  - Same log level in dev and production (always Debug)

### Frontend (TypeScript/React)
- Uses raw `console.log/error/warn` directly in components
- No centralized logging
- No log level control
- No distinction between dev and production
- No way to suppress debug logs in production

## Proposed Unified Logging Strategy

### 1. Backend Logging (Rust)

**Goals:**
- Use consistent `log` macros throughout
- File-based logging with rotation
- Different log levels for dev vs production
- Structured logging with timestamps and module paths

**Implementation:**

1. **Replace all `println!` and `eprintln!` with proper `log` macros:**
   - `println!` → `info!` or `debug!` depending on context
   - `eprintln!` (errors) → `error!`
   - Add `warn!` where appropriate

2. **Upgrade logging to `fern` crate** for better control:
   - Console output in development
   - File output with rotation in production
   - Different log levels based on environment

3. **Add dependencies:**
   ```toml
   fern = "0.6"
   chrono = "0.4"  # already present, for timestamps
   ```

4. **Configure logger in `lib.rs`:**
   ```rust
   #[cfg(debug_assertions)]
   fn init_logger() {
       fern::Dispatch::new()
           .format(|out, message, record| {
               out.finish(format_args!(
                   "{}[{}][{}] {}",
                   chrono::Local::now().format("[%H:%M:%S]"),
                   record.target(),
                   record.level(),
                   message
               ))
           })
           .level(log::LevelFilter::Debug)
           .chain(std::io::stdout())
           .apply()
           .unwrap();
   }

   #[cfg(not(debug_assertions))]
   fn init_logger() {
       let log_dir = app_data_dir.join("logs");
       std::fs::create_dir_all(&log_dir).unwrap();

       fern::Dispatch::new()
           .format(|out, message, record| {
               out.finish(format_args!(
                   "{}[{}][{}] {}",
                   chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                   record.target(),
                   record.level(),
                   message
               ))
           })
           .level(log::LevelFilter::Info)  // No debug in production
           .chain(fern::log_file(log_dir.join("ghub.log")).unwrap())
           .chain(fern::log_file(log_dir.join("error.log")).unwrap().filter(log::LevelFilter::Error))
           .apply()
           .unwrap();
   }
   ```

5. **Replace `println!` calls:**
   - Systematically go through all `.rs` files
   - Replace with appropriate log level:
     - `error!` - failures, errors, exceptions
     - `warn!` - recoverable issues, deprecations
     - `info!` - significant operations (scan started/completed, game added)
     - `debug!` - detailed flow, loops, state changes (only in dev)

### 2. Frontend Logging (TypeScript)

**Goals:**
- Centralized logger with level control
- Debug logs only in development
- Consistent format with backend
- Optional: send errors to backend for persistence

**Implementation:**

1. **Create `src/lib/logger.ts`:**
   ```typescript
   type LogLevel = 'error' | 'warn' | 'info' | 'debug';

   const LOG_LEVELS: Record<LogLevel, number> = {
     error: 0,
     warn: 1,
     info: 2,
     debug: 3,
   };

   // In development, show all logs; in production, only error/warn/info
   const CURRENT_LEVEL = import.meta.env.DEV ? LOG_LEVELS.debug : LOG_LEVELS.info;

   interface Logger {
     error: (...args: any[]) => void;
     warn: (...args: any[]) => void;
     info: (...args: any[]) => void;
     debug: (...args: any[]) => void;
   }

   function createLogger(context: string): Logger {
     const prefix = `[${context}]`;

     const log = (level: LogLevel, args: any[]) => {
       if (LOG_LEVELS[level] <= CURRENT_LEVEL) {
         const timestamp = new Date().toISOString();
         console[level](`${timestamp}${prefix}`, ...args);

         // Optionally send errors/warnings to backend for logging
         if (level === 'error' || level === 'warn') {
           // Send to backend log (optional)
           invoke('log_frontend', { level, message: args.join(' '), context })
             .catch(() => {}); // Ignore errors in logging
         }
       }
     };

     return {
       error: (...args: any[]) => log('error', args),
       warn: (...args: any[]) => log('warn', args),
       info: (...args: any[]) => log('info', args),
       debug: (...args: any[]) => log('debug', args),
     };
   }

   export function createLoggerForComponent(name: string): Logger {
     return createLogger(name);
   }

   export function createServiceLogger(service: string): Logger {
     return createLogger(`Service:${service}`);
   }
   ```

2. **Add backend command for frontend logs** (optional):
   - In `src-tauri/src/commands/mod.rs` add:
     ```rust
     #[tauri::command]
     pub fn log_frontend(level: String, message: String, context: String) {
         match level.as_str() {
             "error" => error!("[Frontend:{}] {}", context, message),
             "warn" => warn!("[Frontend:{}] {}", context, message),
             "info" => info!("[Frontend:{}] {}", context, message),
             "debug" => debug!("[Frontend:{}] {}", context, message),
             _ => info!("[Frontend:{}] {}", context, message),
         }
     }
     ```

3. **Update frontend code:**
   - Replace `console.log/error/warn` with logger instances
   - Create logger at component start:
     ```typescript
     const logger = createLoggerForComponent('App');
     logger.info('Component mounted');
     ```
   - For services/hooks, use `createServiceLogger('games')` etc.

4. **Update `main.tsx`:**
   ```typescript
   import { createLoggerForComponent } from './lib/logger';
   const logger = createLoggerForComponent('Main');
   logger.info('Starting application...');
   ```

### 3. Log Levels Definition

- **Error**: Critical issues that break functionality. Always logged.
- **Warn**: Problems that don't break functionality but need attention. Always logged.
- **Info**: Important operational events (scans complete, games added/removed, settings changed). Always logged.
- **Debug**: Detailed diagnostic info (loop iterations, state changes, API responses). Dev only.

### 4. Migration Tasks

**Backend:**
1. Add `fern` dependency to `Cargo.toml`
2. Replace logger initialization in `lib.rs`
3. Systematically replace `println!` in all files:
   - `src-tauri/src/lib.rs` (1 occurrence)
   - `src-tauri/src/scanning_service.rs` (2 occurrences)
   - `src-tauri/src/playtime.rs` (1 occurrence)
   - `src-tauri/src/metadata/aggregator.rs` (6 occurrences)
   - `src-tauri/src/commands/metadata.rs` (many)
   - `src-tauri/src/commands/scanning.rs` (check)
   - `src-tauri/src/commands/spaces.rs` (1 occurrence)
   - `src-tauri/src/meta_service.rs` (commented println, can remove)
   - `src-tauri/src/commands/backup.rs` (check)
   - `src-tauri/src/commands/games.rs` (check)
   - `src-tauri/src/commands/downloads.rs` (check)
   - `src-tauri/src/commands/playtime.rs` (check)

4. Add `use log::{debug, error, info, warn};` where missing

**Frontend:**
1. Create `src/lib/logger.ts`
2. Add `log_frontend` command to backend (optional)
3. Update components to use logger:
   - `src/main.tsx`
   - `src/App.tsx`
   - All custom hooks in `src/hooks/`
   - All components with console logging
4. Remove direct `console.*` calls

### 5. Configuration

**Environment-based:**
- Development: `RUST_LOG=debug` (but our init sets Debug anyway)
- Production: `RUST_LOG=info` (our init will set Info)

**Tauri config:**
- No changes needed; logger initialized early in `run()`

## Benefits

1. **Consistency**: All logs use same format and levels
2. **Debug control**: Debug logs only in development
3. **Persistence**: Production logs saved to files with rotation
4. **Centralization**: Frontend logs can be captured by backend
5. **Maintainability**: Easy to add logs, clear standards
6. **Diagnostics**: Timestamps and module paths help debugging

## Risks & Mitigations

- **Risk**: Over-logging in production could impact performance
  - **Mitigation**: Set appropriate log level (Info), use lazy evaluation with `log` macros
- **Risk**: Large log files
  - **Mitigation**: Use rotating file logs (can add later with `fern` + `file-rotate`)
- **Risk**: Breaking existing functionality during migration
  - **Mitigation**: Test thoroughly, keep `println!` as comments initially

## Implementation Order

1. Backend: Add `fern`, update logger initialization
2. Backend: Replace `println!` in one file, test, then continue
3. Frontend: Create logger utility
4. Frontend: Update components incrementally
5. Optional: Add frontend-to-backend log forwarding
6. Test in dev and production builds

## Notes

- The `log` crate macros are zero-cost when disabled by level filter
- `fern` provides flexible configuration and is widely used
- For file rotation, consider `fern+file-rotate` if needed later
- Frontend logger uses `import.meta.env.DEV` (Vite) to detect dev mode
