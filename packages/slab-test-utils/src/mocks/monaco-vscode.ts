/**
 * Centralized stubs for the fragile, version-coupled `@codingame/monaco-vscode-*`
 * deep imports. These packages pull in their own `.css` (which vitest cannot
 * load as native ESM), so the workspace logic tests stub only the runtime
 * primitives they touch; TypeScript still resolves the real types.
 *
 * Tests still issue the three `vi.mock` calls themselves (hoisting requires the
 * registration to live in the test file). Because both `vi.mock` and ESM
 * `import` require literal specifiers, the test keeps the literal module paths
 * and imports the stub factories from here — so the brittle parts that move
 * into one place are the stub bodies (the enums / `Emitter` class):
 *
 * ```ts
 * import { monacoUriStub } from '@slab/test-utils/mocks';
 * vi.mock('@codingame/monaco-vscode-api/vscode/vs/base/common/uri', () => monacoUriStub());
 * ```
 *
 * The `MONACO_*_PATH` constants below are reference values (the canonical
 * specifiers) — handy for matching/assertions; they cannot be used as the
 * `vi.mock` / `import` specifier itself, which must be a literal.
 */

export const MONACO_VSCODE_URI_PATH = '@codingame/monaco-vscode-api/vscode/vs/base/common/uri';
export const MONACO_VSCODE_EVENT_PATH = '@codingame/monaco-vscode-api/vscode/vs/base/common/event';
export const MONACO_FILES_SERVICE_OVERRIDE_PATH = '@codingame/monaco-vscode-files-service-override';

export function monacoUriStub() {
  return {
    URI: {
      parse: (value: string) => ({ toString: () => value }),
    },
  };
}

export function monacoEventStub() {
  return {
    Emitter: class TestEmitter<T> {
      private listeners: Array<(event: T) => void> = [];
      readonly event = (listener: (event: T) => void) => {
        this.listeners = [...this.listeners, listener];
        return {
          dispose: () => {
            this.listeners = this.listeners.filter((item) => item !== listener);
          },
        };
      };
      fire(event: T) {
        for (const listener of this.listeners) {
          listener(event);
        }
      }
    },
  };
}

export function monacoFilesServiceOverrideStub() {
  return {
    FileChangeType: { UPDATED: 0, ADDED: 1, DELETED: 2 },
    FileSystemProviderCapabilities: { FileReadWrite: 2 },
    FileSystemProviderError: { create: (message: string) => new Error(message) },
    FileSystemProviderErrorCode: {
      FileNotFound: 'EntryNotFound',
      NoPermissions: 'NoPermissions',
      Unavailable: 'Unavailable',
    },
    FileType: { Unknown: 0, File: 1, Directory: 2, SymbolicLink: 64 },
    OverlayFileSystemProvider: class {
      register() {
        return { dispose() { /* no-op */ } };
      }
    },
    registerFileSystemOverlay: () => ({ dispose() { /* no-op */ } }),
  };
}
