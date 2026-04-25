import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { DEFAULT_API_BASE_URL, normalizeApiBaseUrl } from '@slab/api/config';

describe('desktop API base URL consistency', () => {
  it('keeps frontend defaults normalized to the desktop API origin', () => {
    expect(DEFAULT_API_BASE_URL).toBe('http://127.0.0.1:3000');
    expect(normalizeApiBaseUrl(undefined)).toBe(DEFAULT_API_BASE_URL);
    expect(normalizeApiBaseUrl('http://127.0.0.1:3000/')).toBe(DEFAULT_API_BASE_URL);
  });

  it('keeps static mirrors aligned across package and Tauri config', () => {
    const testDir = dirname(fileURLToPath(import.meta.url));
    const workspacePackageJsonPath = resolve(testDir, '../../../../../package.json');
    const desktopPackageJsonPath = resolve(testDir, '../../../package.json');
    const apiPackageJsonPath = resolve(testDir, '../../../../../packages/api/package.json');
    const tauriConfigPath = resolve(
      testDir,
      '../../../../../bin/slab-app/src-tauri/tauri.conf.json'
    );

    const workspacePackageJson = JSON.parse(readFileSync(workspacePackageJsonPath, 'utf8')) as {
      scripts?: Record<string, string>;
    };
    const desktopPackageJson = JSON.parse(readFileSync(desktopPackageJsonPath, 'utf8')) as {
      scripts?: Record<string, string>;
    };
    const apiPackageJson = JSON.parse(readFileSync(apiPackageJsonPath, 'utf8')) as {
      scripts?: Record<string, string>;
    };
    const tauriConfig = JSON.parse(readFileSync(tauriConfigPath, 'utf8')) as {
      app?: {
        security?: {
          csp?: {
            'connect-src'?: string[];
            'script-src'?: string;
          };
        };
      };
    };

    expect(workspacePackageJson.scripts?.['gen:api']).toBe('bun ./scripts/gen/generate-openapi.ts');
    expect(desktopPackageJson.scripts?.api).toBeUndefined();
    expect(apiPackageJson.scripts?.api).toBeUndefined();
    expect(tauriConfig.app?.security?.csp?.['connect-src']).toContain(DEFAULT_API_BASE_URL);
    expect(tauriConfig.app?.security?.csp?.['script-src']).toContain(DEFAULT_API_BASE_URL);
  });
});
