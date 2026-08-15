import { describe, expect, it } from 'vitest';

import {
  fileNameFromPath,
  isAbsoluteFsPath,
  normalizeFsPathForCompare,
  parentDirectoryPath,
  relativePathFromRoot,
} from '../workspace-path-utils';

describe('workspace path utils', () => {
  describe('fileNameFromPath', () => {
    it('returns the trailing segment for unix and windows paths', () => {
      expect(fileNameFromPath('/a/b/c.txt')).toBe('c.txt');
      expect(fileNameFromPath('C:\\proj\\src\\index.ts')).toBe('index.ts');
    });

    it('returns an empty string when there is no trailing segment', () => {
      expect(fileNameFromPath('/')).toBe('');
      expect(fileNameFromPath('')).toBe('');
    });
  });

  describe('parentDirectoryPath', () => {
    it('returns the directory portion before the last separator', () => {
      expect(parentDirectoryPath('/a/b/c.txt')).toBe('/a/b');
      expect(parentDirectoryPath('C:\\proj\\file.ts')).toBe('C:\\proj');
    });

    it('returns null when there is no parent separator', () => {
      expect(parentDirectoryPath('file.ts')).toBeNull();
      expect(parentDirectoryPath('')).toBeNull();
    });
  });

  describe('normalizeFsPathForCompare', () => {
    it('lowercases, swaps backslashes for slashes and trims trailing slashes', () => {
      expect(normalizeFsPathForCompare('C:\\Proj\\src\\')).toBe('c:/proj/src');
    });
  });

  describe('relativePathFromRoot', () => {
    it('strips the root prefix case-insensitively and normalizes separators', () => {
      expect(relativePathFromRoot('C:\\proj\\src\\a.ts', 'c:/proj')).toBe('src/a.ts');
    });

    it('returns null when the path is outside or equal to the root', () => {
      expect(relativePathFromRoot('C:/other/x.ts', 'c:/proj')).toBeNull();
      expect(relativePathFromRoot('C:/proj', 'c:/proj')).toBeNull();
    });
  });

  describe('isAbsoluteFsPath', () => {
    it('detects windows drive, unix and UNC paths', () => {
      expect(isAbsoluteFsPath('C:\\proj')).toBe(true);
      expect(isAbsoluteFsPath('c:/proj')).toBe(true);
      expect(isAbsoluteFsPath('/home/user')).toBe(true);
      expect(isAbsoluteFsPath('\\\\server\\share')).toBe(true);
      expect(isAbsoluteFsPath('relative/path')).toBe(false);
      expect(isAbsoluteFsPath('file.ts')).toBe(false);
    });
  });
});
