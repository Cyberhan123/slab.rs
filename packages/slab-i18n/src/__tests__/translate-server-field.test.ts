import { beforeEach, describe, expect, it, vi } from 'vitest';

// Import directly from the source module: `server.ts` only has a type-only
// import (`import type { components }`), so importing it triggers none of the
// i18next load-time side effects that `index.ts` would.
import { translateServerField } from '../locales/server';
import type { ServerTranslate } from '../locales/server';

describe('translateServerField', () => {
  const t = vi.fn<ServerTranslate>();

  beforeEach(() => {
    t.mockReset();
  });

  it('returns the fallback when the field ref is missing', () => {
    expect(translateServerField(null, 'status', 'Default', t)).toBe('Default');
    expect(translateServerField({}, 'status', 'Default', t)).toBe('Default');
    expect(translateServerField({ other: { key: 'server.errors.notFound' } }, 'status', 'Default', t)).toBe(
      'Default',
    );
    expect(t).not.toHaveBeenCalled();
  });

  it('returns an empty string when the ref is missing and fallback is nullish', () => {
    expect(translateServerField(null, 'status', null, t)).toBe('');
    expect(translateServerField(null, 'status', undefined, t)).toBe('');
  });

  it('returns the translated value when t produces a non-key translation', () => {
    t.mockReturnValue('未找到');
    const i18n = { status: { key: 'server.errors.notFound' } };

    expect(translateServerField(i18n, 'status', 'Default', t)).toBe('未找到');
    expect(t).toHaveBeenCalledWith('server.errors.notFound', { defaultValue: '' });
  });

  it('falls back when t returns the key unchanged (treated as untranslated)', () => {
    t.mockImplementation((key) => key);
    const i18n = { status: { key: 'server.errors.notFound' } };

    expect(translateServerField(i18n, 'status', 'Default', t)).toBe('Default');
  });

  it('falls back when t returns an empty string', () => {
    t.mockReturnValue('');
    const i18n = { status: { key: 'server.errors.notFound' } };

    expect(translateServerField(i18n, 'status', 'Default', t)).toBe('Default');
  });

  it('spreads ref.params and injects defaultValue when calling t', () => {
    t.mockReturnValue('Not found: thing');
    const i18n = { status: { key: 'server.errors.notFound', params: { detail: 'thing' } } };

    expect(translateServerField(i18n, 'status', null, t)).toBe('Not found: thing');
    expect(t).toHaveBeenCalledWith('server.errors.notFound', { detail: 'thing', defaultValue: '' });
  });
});
