import { describe, expect, it } from 'vitest';

import { normalizeWorkspaceArtifactPath } from '../artifact-path';

describe('normalizeWorkspaceArtifactPath', () => {
  it.each([
    ['a/b', 'a/b'],
    ['a/b/c', 'a/b/c'],
    ['  a/b  ', 'a/b'],
    ['a\\b\\c', 'a/b/c'],
    ['a/./b', 'a/b'],
    ['a//b', 'a/b'],
    ['a/b/', 'a/b'],
  ])('accepts %p and normalizes to %p', (input, expected) => {
    expect(normalizeWorkspaceArtifactPath(input)).toBe(expected);
  });

  it.each([
    ['null', null],
    ['undefined', undefined],
    ['empty', ''],
    ['whitespace', '   '],
    ['absolute posix', '/a/b'],
    ['absolute backslash', '\\a/b'],
    ['windows drive path', 'C:\\a'],
    ['windows drive slash', 'C:/a'],
    ['drive-letter only', 'C:foo'],
    ['parent traversal', 'a/../b'],
    ['leading parent', '../a'],
    ['trailing parent', 'a/..'],
  ])('rejects %p', (_label, input) => {
    expect(normalizeWorkspaceArtifactPath(input)).toBeNull();
  });
});
