/**
 * Frontend Request Diagnostics
 *
 * Provides comprehensive logging and debugging capabilities for API requests
 */

import { getApiConfig } from './config';

// Diagnostic log levels
export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

// Diagnostic configuration
export interface DiagnosticsConfig {
  enabled: boolean;
  logLevel: LogLevel;
  logToConsole: boolean;
  logToStorage: boolean;
  maxStoredLogs: number;
}

const DEFAULT_CONFIG: DiagnosticsConfig = {
  enabled: import.meta.env.DEV, // Enable in development by default
  logLevel: 'debug',
  logToConsole: true,
  logToStorage: true,
  maxStoredLogs: 100,
};

// Diagnostic entry
export interface DiagnosticEntry {
  timestamp: string;
  level: LogLevel;
  type: 'request' | 'response' | 'error' | 'health';
  data: unknown;
}

// Current configuration
let config: DiagnosticsConfig = { ...DEFAULT_CONFIG };

// In-memory log storage
const logs: DiagnosticEntry[] = [];

// Storage key
const STORAGE_KEY = 'slab_api_diagnostics';

/**
 * Initialize diagnostics
 */
export function initDiagnostics(userConfig?: Partial<DiagnosticsConfig>) {
  config = { ...config, ...userConfig };

  // Load logs from storage if enabled
  if (config.logToStorage) {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const parsedLogs = JSON.parse(stored) as DiagnosticEntry[];
        logs.push(...parsedLogs);
      }
    } catch (error) {
      console.warn('[Diagnostics] Failed to load stored logs:', error);
    }
  }

  logInfo('health', { message: 'Diagnostics initialized', config });
}

/**
 * Set diagnostic configuration
 */
export function setDiagnosticsConfig(userConfig: Partial<DiagnosticsConfig>) {
  config = { ...config, ...userConfig };
  logInfo('health', { message: 'Diagnostics config updated', config });
}

/**
 * Get diagnostic configuration
 */
export function getDiagnosticsConfig(): DiagnosticsConfig {
  return { ...config };
}

/**
 * Log a diagnostic entry
 */
function log(entry: DiagnosticEntry) {
  if (!config.enabled) {
    return;
  }

  // Filter by log level
  const levels: LogLevel[] = ['debug', 'info', 'warn', 'error'];
  const currentLevelIndex = levels.indexOf(config.logLevel);
  const entryLevelIndex = levels.indexOf(entry.level);

  if (entryLevelIndex < currentLevelIndex) {
    return;
  }

  // Add to in-memory logs
  logs.push(entry);

  // Trim if exceeds max
  while (logs.length > config.maxStoredLogs) {
    logs.shift();
  }

  // Persist to storage
  if (config.logToStorage) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(logs));
    } catch (error) {
      console.warn('[Diagnostics] Failed to store logs:', error);
    }
  }

  // Log to console
  if (config.logToConsole) {
    const prefix = `[API ${entry.type.toUpperCase()}]`;
    const timestamp = new Date(entry.timestamp).toISOString();

    switch (entry.level) {
      case 'debug':
        console.debug(prefix, timestamp, entry.data);
        break;
      case 'info':
        console.info(prefix, timestamp, entry.data);
        break;
      case 'warn':
        console.warn(prefix, timestamp, entry.data);
        break;
      case 'error':
        console.error(prefix, timestamp, entry.data);
        break;
    }
  }
}

/**
 * Log a debug message
 */
export function logDebug(type: DiagnosticEntry['type'], data: unknown) {
  log({
    timestamp: new Date().toISOString(),
    level: 'debug',
    type,
    data,
  });
}

/**
 * Log an info message
 */
export function logInfo(type: DiagnosticEntry['type'], data: unknown) {
  log({
    timestamp: new Date().toISOString(),
    level: 'info',
    type,
    data,
  });
}

/**
 * Log a warning
 */
export function logWarn(type: DiagnosticEntry['type'], data: unknown) {
  log({
    timestamp: new Date().toISOString(),
    level: 'warn',
    type,
    data,
  });
}

/**
 * Log an error
 */
export function logError(type: DiagnosticEntry['type'], data: unknown) {
  log({
    timestamp: new Date().toISOString(),
    level: 'error',
    type,
    data,
  });
}

/**
 * Get all diagnostic logs
 */
export function getLogs(): DiagnosticEntry[] {
  return [...logs];
}

/**
 * Get logs filtered by type
 */
export function getLogsByType(type: DiagnosticEntry['type']): DiagnosticEntry[] {
  return logs.filter((log) => log.type === type);
}

/**
 * Get logs filtered by level
 */
export function getLogsByLevel(level: LogLevel): DiagnosticEntry[] {
  return logs.filter((log) => log.level === level);
}

/**
 * Clear all diagnostic logs
 */
export function clearLogs() {
  logs.length = 0;
  if (config.logToStorage) {
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch (error) {
      console.warn('[Diagnostics] Failed to clear stored logs:', error);
    }
  }
  logInfo('health', { message: 'Logs cleared' });
}

/**
 * Export logs as JSON
 */
export function exportLogs(): string {
  return JSON.stringify(logs, null, 2);
}

/**
 * Get diagnostic summary
 */
export function getDiagnosticSummary(): {
  totalLogs: number;
  logsByLevel: Record<LogLevel, number>;
  logsByType: Record<DiagnosticEntry['type'], number>;
  apiConfig: ReturnType<typeof getApiConfig>;
} {
  const logsByLevel: Record<LogLevel, number> = {
    debug: 0,
    info: 0,
    warn: 0,
    error: 0,
  };

  const logsByType: Record<DiagnosticEntry['type'], number> = {
    request: 0,
    response: 0,
    error: 0,
    health: 0,
  };

  for (const entry of logs) {
    logsByLevel[entry.level]++;
    logsByType[entry.type]++;
  }

  return {
    totalLogs: logs.length,
    logsByLevel,
    logsByType,
    apiConfig: getApiConfig(),
  };
}

/**
 * Test API connectivity with health endpoint
 */
export async function testApiConnectivity(): Promise<{
  success: boolean;
  url: string;
  status?: number;
  error?: string;
  duration: number;
}> {
  const startTime = performance.now();
  const apiConfig = getApiConfig();
  const healthUrl = `${apiConfig.baseUrl}health`;

  logInfo('health', { message: 'Testing connectivity', url: healthUrl });

  try {
    const response = await fetch(healthUrl);
    const duration = performance.now() - startTime;

    const result = {
      success: response.ok,
      url: healthUrl,
      status: response.status,
      duration: Math.round(duration),
    };

    if (response.ok) {
      logInfo('health', { message: 'Health check successful', ...result });
    } else {
      logWarn('health', { message: 'Health check failed', ...result });
    }

    return result;
  } catch (error) {
    const duration = performance.now() - startTime;
    const errorResult = {
      success: false,
      url: healthUrl,
      error: error instanceof Error ? error.message : 'Unknown error',
      duration: Math.round(duration),
    };

    logError('health', { message: 'Health check error', ...errorResult });
    return errorResult;
  }
}

/**
 * Extract and log task ID from response
 */
export function logTaskId(operation: string, response: unknown): string | null {
  let taskId: string | null = null;

  // Try to extract task_id from response
  if (typeof response === 'object' && response !== null) {
    if ('task_id' in response && typeof response.task_id === 'string') {
      taskId = response.task_id;
    } else if ('id' in response && typeof response.id === 'string') {
      taskId = response.id;
    }
  }

  logInfo('response', {
    operation,
    taskId,
    hasTaskId: taskId !== null,
    response: taskId ? { task_id: taskId } : response,
  });

  return taskId;
}

/**
 * Log actual status values from task response
 */
export function logTaskStatus(operation: string, task: unknown): void {
  if (typeof task === 'object' && task !== null) {
    const status = 'status' in task ? String(task.status) : 'unknown';
    const taskType = 'task_type' in task ? String(task.task_type) : 'unknown';
    const id = 'id' in task ? String(task.id) : 'unknown';

    logInfo('response', {
      operation,
      taskId: id,
      status,
      taskType,
      rawTask: task,
    });
  } else {
    logWarn('response', {
      operation,
      message: 'Invalid task object',
      task,
    });
  }
}

/**
 * Verify API baseUrl is correct
 */
export function verifyApiConfig(): {
  correct: boolean;
  config: ReturnType<typeof getApiConfig>;
  issues: string[];
} {
  const apiConfig = getApiConfig();
  const issues: string[] = [];

  // Check if baseUrl ends with /
  if (!apiConfig.baseUrl.endsWith('/')) {
    issues.push('baseUrl should end with /');
  }

  // Check if baseUrl is a valid URL
  try {
    new URL(apiConfig.baseUrl);
  } catch {
    issues.push('baseUrl is not a valid URL');
  }

  // Check if using localhost in production
  if (apiConfig.baseUrl.includes('localhost') && !import.meta.env.DEV) {
    issues.push('Using localhost in production');
  }

  const correct = issues.length === 0;

  logInfo('health', {
    message: 'API config verification',
    correct,
    issues,
    config: apiConfig,
  });

  return { correct, config: apiConfig, issues };
}

/**
 * Create a diagnostic report
 */
export function createDiagnosticReport(): string {
  const summary = getDiagnosticSummary();
  const config = verifyApiConfig();

  let report = '=== Slab Frontend Diagnostics Report ===\n\n';
  report += `Generated: ${new Date().toISOString()}\n\n`;

  report += '--- API Configuration ---\n';
  report += `Mode: ${summary.apiConfig.mode}\n`;
  report += `Base URL: ${summary.apiConfig.baseUrl}\n`;
  report += `Config Valid: ${config.correct ? 'Yes' : 'No'}\n`;
  if (config.issues.length > 0) {
    report += `Issues:\n  ${config.issues.join('\n  ')}\n`;
  }
  report += '\n';

  report += '--- Log Summary ---\n';
  report += `Total Logs: ${summary.totalLogs}\n`;
  report += `By Level:\n`;
  report += `  Debug: ${summary.logsByLevel.debug}\n`;
  report += `  Info: ${summary.logsByLevel.info}\n`;
  report += `  Warn: ${summary.logsByLevel.warn}\n`;
  report += `  Error: ${summary.logsByLevel.error}\n`;
  report += `By Type:\n`;
  report += `  Requests: ${summary.logsByType.request}\n`;
  report += `  Responses: ${summary.logsByType.response}\n`;
  report += `  Errors: ${summary.logsByType.error}\n`;
  report += `  Health: ${summary.logsByType.health}\n`;
  report += '\n';

  report += '--- Recent Errors ---\n';
  const recentErrors = logs
    .filter((l) => l.level === 'error')
    .slice(-5)
    .map((l) => `[${l.timestamp}] ${JSON.stringify(l.data)}`)
    .join('\n');
  report += recentErrors || 'No errors\n';
  report += '\n';

  report += '--- Recent Requests ---\n';
  const recentRequests = logs
    .filter((l) => l.type === 'request')
    .slice(-5)
    .map((l) => `[${l.timestamp}] ${JSON.stringify(l.data)}`)
    .join('\n');
  report += recentRequests || 'No requests\n';

  return report;
}

// Auto-initialize on import
initDiagnostics();
