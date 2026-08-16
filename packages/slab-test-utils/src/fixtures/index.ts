/**
 * Build a typed fixture factory from a defaults object. Mirrors the existing
 * gold-standard `createBackend(overrides)` / `fileEntry(overrides)` idiom
 * (default object + `Partial<T>` spread).
 *
 * ```ts
 * const makeFile = defineFixture({ kind: 'file', name: 'a.ts', relativePath: 'src/a.ts' });
 * makeFile({ name: 'b.ts' }); // → { kind: 'file', name: 'b.ts', relativePath: 'src/a.ts' }
 * ```
 */
export function defineFixture<T>(defaults: T): (overrides?: Partial<T>) => T {
  return (overrides: Partial<T> = {}): T => ({ ...defaults, ...overrides });
}
