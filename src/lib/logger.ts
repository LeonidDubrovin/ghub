import { invoke } from '@tauri-apps/api/core';

export type LogLevel = 'error' | 'warn' | 'info' | 'debug';

const LOG_LEVELS: Record<LogLevel, number> = {
  error: 0,
  warn: 1,
  info: 2,
  debug: 3,
};

// Check if we're in development mode
// Vite provides import.meta.env.DEV, but we need to handle TypeScript typing
const isDev = typeof import.meta !== 'undefined' && (import.meta as any).env?.DEV;
const CURRENT_LEVEL = isDev ? LOG_LEVELS.debug : LOG_LEVELS.info;

interface Logger {
  error: (...args: any[]) => void;
  warn: (...args: any[]) => void;
  info: (...args: any[]) => void;
  debug: (...args: any[]) => void;
}

/**
 * Create a logger with the given context prefix
 * Logs are sent to console and optionally forwarded to backend
 */
export function createLogger(context: string): Logger {
  const prefix = `[${context}]`;

  const log = (level: LogLevel, args: any[]) => {
    if (LOG_LEVELS[level] <= CURRENT_LEVEL) {
      const timestamp = new Date().toISOString();
      console[level](`${timestamp}${prefix}`, ...args);

      // Forward errors and warnings to backend for persistent logging
      if (level === 'error' || level === 'warn') {
        invoke('log_frontend', {
          level,
          message: args.map(a => String(a)).join(' '),
          context,
        }).catch(() => {
          // Ignore errors in logging - we don't want logging to break the app
        });
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

/**
 * Create a logger for a React component
 * Usage: const logger = createLoggerForComponent('GameGrid');
 */
export function createLoggerForComponent(componentName: string): Logger {
  return createLogger(`Component:${componentName}`);
}

/**
 * Create a logger for a service or utility
 * Usage: const logger = createServiceLogger('games');
 */
export function createServiceLogger(serviceName: string): Logger {
  return createLogger(`Service:${serviceName}`);
}
