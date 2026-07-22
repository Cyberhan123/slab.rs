import { describe, expect, it, vi } from 'vitest';

import {
  supportsWorkspaceLsp,
  workspaceLspDefinitionTargetFromResult,
  workspaceLspFileUri,
  workspaceLspImportSpecifierPositionForTarget,
  workspaceLspModelUri,
  workspaceLspRelativePathFromUri,
  workspaceRootUri,
  workspaceVscodeDirtyCloseTarget,
  workspaceVscodeResourceStringFromEditorInput,
} from '../workspace-uri';

const ROOT = 'C:\\proj';

describe('supportsWorkspaceLsp', () => {
  it.each(['rust', 'typescript', 'javascriptreact', 'python', 'go', 'css'])(
    'treats %p as LSP-supported',
    (language) => {
      expect(supportsWorkspaceLsp(language)).toBe(true);
    },
  );

  it.each(['plaintext', 'markdown', 'dockerfile', '', 'unknown'])(
    'treats %p as not LSP-supported',
    (language) => {
      expect(supportsWorkspaceLsp(language)).toBe(false);
    },
  );
});

describe('workspaceLspFileUri', () => {
  it('builds a file:// uri from a Windows root and a relative path', () => {
    expect(workspaceLspFileUri(ROOT, 'src/main.ts')).toBe('file:///c:/proj/src/main.ts');
  });

  it('lowercases the drive letter and strips a leading slash before it', () => {
    expect(workspaceLspFileUri('D:\\Projects', 'a.ts')).toBe('file:///d:/Projects/a.ts');
  });

  it('flips backslashes and drops a leading slash on the relative path', () => {
    expect(workspaceLspFileUri(ROOT, '\\nested\\file.ts')).toBe('file:///c:/proj/nested/file.ts');
  });

  it('returns just the root uri when no relative path is given', () => {
    expect(workspaceLspFileUri(ROOT)).toBe('file:///c:/proj');
  });

  it('collapses a Windows UNC long path into a // host form', () => {
    expect(workspaceLspFileUri('\\\\?\\UNC\\server\\share', 'f.ts')).toBe(
      'file:////server/share/f.ts',
    );
  });
});

describe('workspaceLspRelativePathFromUri', () => {
  it('extracts the relative path under the root', () => {
    expect(workspaceLspRelativePathFromUri(ROOT, 'file:///c:/proj/src/main.ts')).toBe(
      'src/main.ts',
    );
  });

  it('returns an empty string when the uri is exactly the root', () => {
    expect(workspaceLspRelativePathFromUri(ROOT, 'file:///c:/proj')).toBe('');
  });

  it('returns null when the uri is outside the root', () => {
    expect(workspaceLspRelativePathFromUri(ROOT, 'file:///c:/other/x.ts')).toBeNull();
  });

  it('returns null for a non-file uri', () => {
    expect(workspaceLspRelativePathFromUri(ROOT, 'http://host/path')).toBeNull();
  });

  it('falls back to treating a path-like string as a pathname', () => {
    // A scheme-less path string is not a valid URL and throws; the helper
    // then treats the raw string as a pathname and still resolves it.
    expect(workspaceLspRelativePathFromUri(ROOT, '/c:/proj/src/main.ts')).toBe('src/main.ts');
  });
});

describe('workspaceLspDefinitionTargetFromResult', () => {
  it('maps an LSP location to a relative path plus 1-based selection', () => {
    expect(
      workspaceLspDefinitionTargetFromResult(ROOT, {
        uri: 'file:///c:/proj/src/main.ts',
        range: { start: { line: 0, character: 5 }, end: { line: 0, character: 10 } },
      }),
    ).toEqual({
      relativePath: 'src/main.ts',
      startColumn: 6,
      startLineNumber: 1,
      endColumn: 11,
      endLineNumber: 1,
    });
  });

  it('prefers targetUri/targetSelectionRange and falls back through range/targetRange', () => {
    expect(
      workspaceLspDefinitionTargetFromResult(ROOT, {
        targetUri: 'file:///c:/proj/src/main.ts',
        targetSelectionRange: { start: { line: 4, character: 0 } },
      }),
    ).toEqual({
      relativePath: 'src/main.ts',
      startColumn: 1,
      startLineNumber: 5,
      endColumn: 1,
      endLineNumber: 5,
    });
  });

  it('returns the first resolvable target from an array of definitions', () => {
    expect(
      workspaceLspDefinitionTargetFromResult(ROOT, [
        { uri: 'file:///c:/outside/x.ts', range: { start: { line: 0, character: 0 } } },
        { uri: 'file:///c:/proj/src/main.ts', range: { start: { line: 1, character: 1 } } },
      ]),
    ).toEqual({
      relativePath: 'src/main.ts',
      startColumn: 2,
      startLineNumber: 2,
      endColumn: 2,
      endLineNumber: 2,
    });
  });

  it('returns null for an empty/invalid result or an out-of-root uri', () => {
    expect(workspaceLspDefinitionTargetFromResult(ROOT, null)).toBeNull();
    expect(workspaceLspDefinitionTargetFromResult(ROOT, undefined)).toBeNull();
    expect(
      workspaceLspDefinitionTargetFromResult(ROOT, { uri: 'file:///c:/outside/x.ts' }),
    ).toBeNull();
    // A range without numeric line/character yields no selection coordinates.
    expect(
      workspaceLspDefinitionTargetFromResult(ROOT, {
        uri: 'file:///c:/proj/src/main.ts',
        range: { start: { line: 0 } },
      }),
    ).toEqual({ relativePath: 'src/main.ts' });
  });
});

describe('workspaceLspImportSpecifierPositionForTarget', () => {
  it('locates the import specifier quote on a from-import line', () => {
    const target = {
      relativePath: 'src/main.ts',
      startLineNumber: 1,
      startColumn: 1,
      endColumn: 1,
    };
    expect(workspaceLspImportSpecifierPositionForTarget("from 'a'", target)).toEqual({
      column: 7,
      lineNumber: 1,
    });
  });

  it('returns null when the target has no usable position', () => {
    expect(
      workspaceLspImportSpecifierPositionForTarget("from 'a'", { relativePath: 'x.ts' }),
    ).toBeNull();
  });

  it('returns null when the line has no from-import', () => {
    const target = { relativePath: 'x.ts', startLineNumber: 1, startColumn: 1 };
    expect(workspaceLspImportSpecifierPositionForTarget('const x = 1', target)).toBeNull();
  });
});

describe('workspaceVscodeResourceStringFromEditorInput', () => {
  it('reads resource.toString() from a typed editor input', () => {
    expect(
      workspaceVscodeResourceStringFromEditorInput({
        resource: { toString: () => 'file:///c:/proj/src/main.ts' },
      }),
    ).toBe('file:///c:/proj/src/main.ts');
  });

  it('reads resource off toUntyped() as a fallback', () => {
    expect(
      workspaceVscodeResourceStringFromEditorInput({
        toUntyped: () => ({ resource: { toString: () => 'file:///c:/proj/src/main.ts' } }),
      }),
    ).toBe('file:///c:/proj/src/main.ts');
  });

  it('returns null for inputs without a resolvable resource', () => {
    expect(workspaceVscodeResourceStringFromEditorInput(null)).toBeNull();
    expect(workspaceVscodeResourceStringFromEditorInput({})).toBeNull();
    expect(workspaceVscodeResourceStringFromEditorInput({ resource: 'plain string' })).toBeNull();
  });
});

describe('workspaceVscodeDirtyCloseTarget', () => {
  const input = { resource: { toString: () => 'file:///c:/proj/src/main.ts' } };

  it('returns the relative path when the resource is dirty', () => {
    expect(workspaceVscodeDirtyCloseTarget(ROOT, input, () => true)).toBe('src/main.ts');
  });

  it('returns null when the resource is clean', () => {
    expect(workspaceVscodeDirtyCloseTarget(ROOT, input, () => false)).toBeNull();
  });

  it('returns null when the input has no resolvable resource', () => {
    expect(workspaceVscodeDirtyCloseTarget(ROOT, {}, () => true)).toBeNull();
  });

  it('returns null when the resource is outside the workspace root', () => {
    expect(
      workspaceVscodeDirtyCloseTarget(
        ROOT,
        { resource: { toString: () => 'file:///c:/other/x.ts' } },
        () => true,
      ),
    ).toBeNull();
  });
});

describe('workspaceRootUri', () => {
  it('parses the root file uri into a monaco URI', () => {
    // Monaco's URI re-encodes the drive-letter colon on toString.
    const uri = workspaceRootUri(ROOT);
    expect(uri.toString()).toBe('file:///c%3A/proj');
  });
});

describe('workspaceLspModelUri', () => {
  it('defers to the injected monaco Uri.parse', () => {
    const parse = vi.fn<(value: string) => string>().mockImplementation((value) => `parsed:${value}`);
    const monaco = { Uri: { parse } } as never;

    const uri = workspaceLspModelUri(monaco, ROOT, 'src/main.ts');

    expect(parse).toHaveBeenCalledWith('file:///c:/proj/src/main.ts');
    expect(uri).toBe('parsed:file:///c:/proj/src/main.ts');
  });
});
