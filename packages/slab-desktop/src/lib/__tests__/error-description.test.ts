import { describe, expect, it } from 'vitest';

import { getErrorDescription } from '../error-description';

describe('error description', () => {
  it('uses Error messages before fallback text', () => {
    expect(getErrorDescription(new Error('failed'), 'fallback')).toBe('failed');
  });

  it('uses message and error fields from object errors', () => {
    expect(getErrorDescription({ message: 'bad request' }, 'fallback')).toBe('bad request');
    expect(getErrorDescription({ error: 'missing file' }, 'fallback')).toBe('missing file');
  });

  it('falls back when the error has no readable message', () => {
    expect(getErrorDescription({ message: '  ', error: null }, 'fallback')).toBe('fallback');
    expect(getErrorDescription(undefined, 'fallback')).toBe('fallback');
  });
});
