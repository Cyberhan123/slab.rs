/**
 * API Configuration for Slab App
 *
 * Supports multiple runtime modes:
 * - Web/HTTP mode: Connects to backend via HTTP
 * - Tauri/IPC mode: Connects via Tauri commands (future)
 */

import { getApiUrl as getTauriApiUrl } from '@/lib/tauri-api';

export type ApiMode = 'http' | 'ipc';

/**
 * Detect the current runtime mode
 */
export function detectApiMode(): ApiMode {
  // Check if running in Tauri
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    return 'ipc';
  }
  return 'http';
}

/**
 * Get the API base URL based on the current environment
 * This is a synchronous version for immediate use
 */
export function getApiBaseUrl(): string {
  const mode = detectApiMode();

  if (mode === 'ipc') {
    // In Tauri mode, check environment variable first
    if (import.meta.env.VITE_API_URL) {
      return import.meta.env.VITE_API_URL;
    }
    // Default to localhost for now
    // TODO: Make this async and call getTauriApiUrl() when needed
    return 'http://localhost:3000/';
  }

  // HTTP mode (web browser)
  return import.meta.env.VITE_API_URL || 'http://localhost:3000/';
}

/**
 * Get the API base URL asynchronously
 * This will call Tauri commands when in Tauri mode
 */
export async function getApiBaseUrlAsync(): Promise<string> {
  const mode = detectApiMode();

  if (mode === 'ipc') {
    return await getTauriApiUrl();
  }

  return import.meta.env.VITE_API_URL || 'http://localhost:3000/';
}

/**
 * Get API configuration
 */
export function getApiConfig() {
  return {
    mode: detectApiMode(),
    baseUrl: getApiBaseUrl(),
  };
}

/**
 * Get API configuration asynchronously
 */
export async function getApiConfigAsync() {
  return {
    mode: detectApiMode(),
    baseUrl: await getApiBaseUrlAsync(),
  };
}
