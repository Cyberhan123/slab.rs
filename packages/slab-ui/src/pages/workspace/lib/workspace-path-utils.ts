/**
 * Filesystem path helpers used by the workspace page. Pure string math with no
 * React, state, or side effects. Extracted from use-workspace-page.ts for
 * direct unit testing. Note: isAbsoluteFsPath treats UNC paths (\\\\) as
 * absolute and differs slightly from lib/workspace-artifact-path.ts (single
 * backslash); the two are intentionally not consolidated yet.
 */

export function fileNameFromPath(path: string): string {
  return path.match(/[^/\\]+$/)?.[0] ?? '';
}

export function parentDirectoryPath(path: string): string | null {
  const normalized = path.trim();
  const separatorIndex = Math.max(normalized.lastIndexOf('\\'), normalized.lastIndexOf('/'));
  return separatorIndex > 0 ? normalized.slice(0, separatorIndex) : null;
}

export function normalizeFsPathForCompare(path: string): string {
  return path.replaceAll('\\', '/').replace(/\/+$/, '').toLowerCase();
}

export function relativePathFromRoot(path: string, rootPath: string): string | null {
  const comparablePath = normalizeFsPathForCompare(path);
  const comparableRoot = normalizeFsPathForCompare(rootPath);
  if (comparablePath === comparableRoot || !comparablePath.startsWith(`${comparableRoot}/`)) {
    return null;
  }

  return path.replaceAll('\\', '/').replace(/\/+$/, '').slice(comparableRoot.length + 1);
}

export function isAbsoluteFsPath(path: string): boolean {
  return /^[a-zA-Z]:[\\/]/.test(path) || path.startsWith('/') || path.startsWith('\\\\');
}
