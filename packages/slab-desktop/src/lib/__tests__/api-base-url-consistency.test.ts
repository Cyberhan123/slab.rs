import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { DEFAULT_API_BASE_URL, normalizeApiBaseUrl } from '@slab/api/config';

describe('desktop API base URL consistency', () => {
  it('keeps frontend defaults normalized to the desktop API origin', () => {
    expect(DEFAULT_API_BASE_URL).toBe('http://127.0.0.1:3000');
    expect(normalizeApiBaseUrl(undefined)).toBe(DEFAULT_API_BASE_URL);
    expect(normalizeApiBaseUrl('http://127.0.0.1:3000/')).toBe(DEFAULT_API_BASE_URL);
  });

  it('keeps static mirrors aligned across package and Tauri config', () => {
    const packageJsonPath = resolve(process.cwd(), 'package.json');
    const apiPackageJsonPath = resolve(process.cwd(), '../api/package.json');
    const tauriConfigPath = resolve(process.cwd(), '../../bin/slab-app/src-tauri/tauri.conf.json');

    const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8')) as {
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

    expect(packageJson.scripts?.api).toContain('../api/src/v1.d.ts');
    expect(apiPackageJson.scripts?.api).toContain('http://127.0.0.1:3000/api-docs/openapi.json');
    expect(tauriConfig.app?.security?.csp?.['connect-src']).toContain(DEFAULT_API_BASE_URL);
    expect(tauriConfig.app?.security?.csp?.['script-src']).toContain(DEFAULT_API_BASE_URL);
  });
});
