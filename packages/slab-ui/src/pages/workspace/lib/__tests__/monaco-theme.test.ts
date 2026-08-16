import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  applySlabMonacoTheme,
  buildSlabMonacoTheme,
  getWorkspaceThemeMode,
  registerSlabMonacoTheme,
  slabMonacoThemeId,
} from '../monaco-theme';

describe('slabMonacoThemeId', () => {
  it('maps each mode to its slab theme id', () => {
    expect(slabMonacoThemeId('dark')).toBe('slab-dark');
    expect(slabMonacoThemeId('light')).toBe('slab-light');
  });
});

describe('getWorkspaceThemeMode', () => {
  afterEach(() => {
    document.documentElement.classList.remove('dark');
  });

  it('reads dark mode off the document element class list', () => {
    expect(getWorkspaceThemeMode()).toBe('light');
    document.documentElement.classList.add('dark');
    expect(getWorkspaceThemeMode()).toBe('dark');
  });
});

describe('buildSlabMonacoTheme', () => {
  it('builds a vs-dark theme with token rules and editor colors', () => {
    // jsdom has no canvas context, so CSS colors fall back to the bundled
    // palette; assert structure rather than exact resolved colors.
    const theme = buildSlabMonacoTheme('dark');
    expect(theme.base).toBe('vs-dark');
    expect(theme.inherit).toBe(true);
    expect(theme.rules).toHaveLength(6);
    expect(theme.rules[0]).toMatchObject({ token: 'comment', fontStyle: 'italic' });
    expect(theme.colors).toHaveProperty('editor.background');
    expect(theme.colors['editor.background']).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it('builds a vs (light) theme for light mode', () => {
    expect(buildSlabMonacoTheme('light').base).toBe('vs');
  });
});

describe('registerSlabMonacoTheme', () => {
  it('defines the theme through the injected monaco editor', () => {
    const defineTheme = vi.fn<(id: string, data: unknown) => void>();
    const monaco = { editor: { defineTheme } } as never;

    registerSlabMonacoTheme(monaco, 'dark');

    expect(defineTheme).toHaveBeenCalledWith('slab-dark', expect.objectContaining({ base: 'vs-dark' }));
  });
});

describe('applySlabMonacoTheme', () => {
  it('registers and applies the slab theme, returning its id', () => {
    const defineTheme = vi.fn<(id: string, data: unknown) => void>();
    const setTheme = vi.fn<(id: string) => void>();
    const monaco = { editor: { defineTheme, setTheme } } as never;

    expect(applySlabMonacoTheme(monaco, 'dark')).toBe('slab-dark');
    expect(defineTheme).toHaveBeenCalledTimes(1);
    expect(setTheme).toHaveBeenCalledWith('slab-dark');
  });

  it('falls back to the stock theme when registration throws', () => {
    const defineTheme = vi.fn<(id: string, data: unknown) => void>(() => {
      throw new Error('boom');
    });
    const setTheme = vi.fn<(id: string) => void>();
    const monaco = { editor: { defineTheme, setTheme } } as never;

    expect(applySlabMonacoTheme(monaco, 'dark')).toBe('vs-dark');
    expect(setTheme).toHaveBeenCalledWith('vs-dark');
  });

  it('falls back when setTheme throws after a successful registration', () => {
    const defineTheme = vi.fn<(id: string, data: unknown) => void>();
    const setTheme = vi.fn<(id: string) => void>().mockImplementationOnce(() => {
      throw new Error('boom');
    });
    const monaco = { editor: { defineTheme, setTheme } } as never;

    expect(applySlabMonacoTheme(monaco, 'dark')).toBe('vs-dark');
    expect(setTheme).toHaveBeenNthCalledWith(1, 'slab-dark');
    expect(setTheme).toHaveBeenNthCalledWith(2, 'vs-dark');
  });
});
