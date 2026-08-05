import { describe, expect, it } from 'vitest';

import { DEFAULT_API_BASE_URL, normalizeApiBaseUrl } from '@slab/api/config';

// Static JSON imports resolve through Vite in Browser Mode (no `node:fs` /
// `node:path`, which are externalized and unavailable in the browser).
import workspacePkg from '../../../../../package.json';
import desktopPkg from '../../../package.json';
import apiPkg from '../../../../../packages/api/package.json';
import tauriConf from '../../../../../bin/slab-app/src-tauri/tauri.conf.json';

describe('desktop API base URL consistency', () => {
  it('keeps frontend defaults normalized to the desktop API origin', () => {
    expect(DEFAULT_API_BASE_URL).toBe('http://127.0.0.1:3000');
    expect(normalizeApiBaseUrl(undefined)).toBe(DEFAULT_API_BASE_URL);
    expect(normalizeApiBaseUrl('http://127.0.0.1:3000/')).toBe(DEFAULT_API_BASE_URL);
  });

  it('keeps static mirrors aligned across package and Tauri config', () => {
    const workspacePackageJson = workspacePkg as { scripts?: Record<string, string> };
    const desktopPackageJson = desktopPkg as { scripts?: Record<string, string> };
    const apiPackageJson = apiPkg as { scripts?: Record<string, string> };
    const tauriConfig = tauriConf as {
      app?: {
        security?: {
          csp?: {
            'connect-src'?: string[];
            'script-src'?: string;
          };
        };
      };
    };

    expect(workspacePackageJson.scripts?.['gen:api']).toMatch(/cargo.*build.*slab-server/);
    expect(desktopPackageJson.scripts?.api).toBeUndefined();
    expect(apiPackageJson.scripts?.api).toBeUndefined();
    expect(tauriConfig.app?.security?.csp?.['connect-src']).toContain(DEFAULT_API_BASE_URL);
    expect(tauriConfig.app?.security?.csp?.['connect-src']).toContain('http://ipc.localhost');
    expect(tauriConfig.app?.security?.csp?.['script-src']).toContain(DEFAULT_API_BASE_URL);
  });
});
