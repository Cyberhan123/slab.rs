import { describe, expect, it } from 'vitest';

import type { WorkspaceFileEntry } from '@slab/core/workspace/bridge';
import type { WorkspaceFileTab } from '@/store/useWorkspaceUiStore';

import {
  SLAB_DIR_NAME,
  directoryAncestors,
  entryToTreeNode,
  insertChildren,
  languageForFile,
  lspLanguageForFile,
  sortDirectoryPaths,
  upsertFileTab,
} from '../workspace-page-utils';

function fileEntry(relativePath: string, overrides: Partial<WorkspaceFileEntry> = {}): WorkspaceFileEntry {
  return {
    id: relativePath,
    name: relativePath.split('/').pop() ?? relativePath,
    relativePath,
    kind: 'file',
    hasChildren: false,
    ...overrides,
  };
}

function dirEntry(relativePath: string, overrides: Partial<WorkspaceFileEntry> = {}): WorkspaceFileEntry {
  return {
    id: relativePath,
    name: relativePath.split('/').pop() ?? relativePath,
    relativePath,
    kind: 'directory',
    hasChildren: true,
    ...overrides,
  };
}

function tab(relativePath: string, name = relativePath.split('/').pop() ?? relativePath): WorkspaceFileTab {
  return { relativePath, name };
}

describe('entryToTreeNode', () => {
  it('marks files loaded with no children and directories unloaded with empty children', () => {
    const fileNode = entryToTreeNode(fileEntry('a/b.ts'));
    expect(fileNode.loaded).toBe(true);
    expect(fileNode.children).toBeUndefined();

    const dirNode = entryToTreeNode(dirEntry('src'));
    expect(dirNode.loaded).toBe(false);
    expect(dirNode.children).toEqual([]);
  });
});

describe('insertChildren', () => {
  it('attaches children to the matching node at the top level', () => {
    const tree = [entryToTreeNode(dirEntry('src'))];
    const child = entryToTreeNode(fileEntry('src/main.ts'));
    const result = insertChildren(tree, 'src', [child]);
    expect(result[0].children).toEqual([child]);
    expect(result[0].loaded).toBe(true);
  });

  it('recurses into nested directories to find the target', () => {
    const root = entryToTreeNode(dirEntry('src'));
    const nested = entryToTreeNode(dirEntry('src/nested'));
    root.children = [nested];
    const leaf = entryToTreeNode(fileEntry('src/nested/main.ts'));

    const result = insertChildren([root], 'src/nested', [leaf]);
    expect(result[0].children?.[0].children).toEqual([leaf]);
    expect(result[0].children?.[0].loaded).toBe(true);
  });
});

describe('languageForFile', () => {
  it.each([
    ['main.ts', 'typescript'],
    ['main.tsx', 'typescript'],
    ['app.js', 'javascript'],
    ['app.jsx', 'javascript'],
    ['app.mjs', 'javascript'],
    ['lib.rs', 'rust'],
    ['script.py', 'python'],
    ['main.go', 'go'],
    ['App.java', 'java'],
    ['style.css', 'css'],
    ['dark.scss', 'scss'],
    ['reset.less', 'less'],
    ['data.json', 'json'],
    ['readme.md', 'markdown'],
    ['config.toml', 'toml'],
    ['deploy.sh', 'shell'],
    ['Dockerfile', 'dockerfile'],
    ['Makefile', 'makefile'],
    ['.env', 'dotenv'],
    ['.env.local', 'dotenv'],
    ['unknown.xyz', 'plaintext'],
    ['noext', 'plaintext'],
  ])('maps %p to %p', (fileName, language) => {
    expect(languageForFile(fileName)).toBe(language);
  });
});

describe('lspLanguageForFile', () => {
  it('overrides tsx/jsx with the react LSP language and delegates otherwise', () => {
    expect(lspLanguageForFile('a.tsx')).toBe('typescriptreact');
    expect(lspLanguageForFile('a.jsx')).toBe('javascriptreact');
    expect(lspLanguageForFile('a.ts')).toBe('typescript');
    expect(lspLanguageForFile('a.rs')).toBe('rust');
  });
});

describe('upsertFileTab', () => {
  it('replaces a tab with a matching path and appends a new one', () => {
    const tabs = [tab('a')];
    expect(upsertFileTab(tabs, tab('a', 'renamed'))).toEqual([tab('a', 'renamed')]);
    expect(upsertFileTab(tabs, tab('b'))).toEqual([tab('a'), tab('b')]);
  });
});

describe('sortDirectoryPaths', () => {
  it('dedupes, drops empties, and sorts by depth then locale', () => {
    expect(sortDirectoryPaths(['b/c', 'a', 'b', '', 'a'])).toEqual(['a', 'b', 'b/c']);
  });
});

describe('directoryAncestors', () => {
  it('lists ancestor directories, optionally including self', () => {
    expect(directoryAncestors('a/b/c')).toEqual(['a', 'a/b']);
    expect(directoryAncestors('a/b/c', true)).toEqual(['a', 'a/b', 'a/b/c']);
    expect(directoryAncestors('')).toEqual([]);
    expect(directoryAncestors('a')).toEqual([]);
    expect(directoryAncestors('a', true)).toEqual(['a']);
  });
});

describe('SLAB_DIR_NAME', () => {
  it('is the .slab directory', () => {
    expect(SLAB_DIR_NAME).toBe('.slab');
  });
});
