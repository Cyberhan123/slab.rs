/**
 * Tauri API wrapper
 *
 * Provides type-safe access to Tauri commands from the frontend.
 */

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '@/hooks/use-tauri';

/**
 * Get the API base URL from Tauri
 */
export async function getApiUrl(): Promise<string> {
  if (!isTauri()) {
    // Fallback to environment variable in web mode
    return import.meta.env.VITE_API_URL || 'http://localhost:3000/';
  }

  try {
    return await invoke<string>('get_api_url');
  } catch (error) {
    console.error('Failed to get API URL from Tauri:', error);
    return 'http://localhost:3000/';
  }
}

/**
 * Check if the backend server is running
 */
export async function checkBackendStatus(): Promise<boolean> {
  if (!isTauri()) {
    // In web mode, just try to fetch the health endpoint
    try {
      const apiUrl = import.meta.env.VITE_API_URL || 'http://localhost:3000/';
      const response = await fetch(`${apiUrl}health`);
      return response.ok;
    } catch {
      return false;
    }
  }

  try {
    return await invoke<boolean>('check_backend_status');
  } catch (error) {
    console.error('Failed to check backend status:', error);
    return false;
  }
}

/**
 * Get system information
 */
export async function getSystemInfo(): Promise<string> {
  if (!isTauri()) {
    return 'OS: Web\nArch: Unknown';
  }

  try {
    return await invoke<string>('get_system_info');
  } catch (error) {
    console.error('Failed to get system info:', error);
    return 'Unknown system';
  }
}

/**
 * Greet command (demo)
 */
export async function greet(name: string): Promise<string> {
  if (!isTauri()) {
    return `Hello, ${name}! (Web mode)`;
  }

  try {
    return await invoke<string>('greet', { name });
  } catch (error) {
    console.error('Failed to greet:', error);
    return `Hello, ${name}!`;
  }
}
