import { describe, expect, it } from 'vitest';

import { parseOptionalFloat, parseOptionalInt } from '../audio-value-parsing';

// The translator is injected (not imported), so a key-only stub is enough to
// assert which validation message a throw surfaces.
const t = (key: string): string => key;

describe('parseOptionalInt', () => {
  it.each([
    ['', 0, undefined],
    ['   ', 0, undefined],
    ['5', 0, 5],
    ['42', 1, 42],
    ['-7', -10, -7],
  ])('parses %p with min %p into %p', (raw, min, expected) => {
    expect(parseOptionalInt(raw, 'field', min, t)).toBe(expected);
  });

  it.each([
    ['1.5', 0],
    ['abc', 0],
    ['NaN', 0],
  ])('throws the integer error for non-integer %p', (raw, min) => {
    expect(() => parseOptionalInt(raw, 'field', min, t)).toThrow(
      'pages.audio.validation.integer',
    );
  });

  it('throws the min error below the floor (no clamping)', () => {
    expect(() => parseOptionalInt('3', 'field', 10, t)).toThrow('pages.audio.validation.min');
    expect(() => parseOptionalInt('-1', 'field', 0, t)).toThrow('pages.audio.validation.min');
  });
});

describe('parseOptionalFloat', () => {
  it.each([
    ['', {}, undefined],
    ['   ', {}, undefined],
    ['1.5', {}, 1.5],
    ['3', {}, 3],
    ['-0.25', {}, -0.25],
    ['0.1', { exclusiveMin: 0 }, 0.1],
  ])('parses %p with options %p into %p', (raw, options, expected) => {
    expect(parseOptionalFloat(raw, 'field', t, options)).toBe(expected);
  });

  it.each([
    ['abc', {}, 'pages.audio.validation.number'],
    ['Infinity', {}, 'pages.audio.validation.number'],
    ['1e999', {}, 'pages.audio.validation.number'],
  ])('throws %p for non-finite input %p', (raw, options, message) => {
    expect(() => parseOptionalFloat(raw, 'field', t, options)).toThrow(message);
  });

  it('enforces min, max, and exclusiveMin bounds (no clamping)', () => {
    expect(() => parseOptionalFloat('0.5', 'field', t, { min: 1 })).toThrow(
      'pages.audio.validation.min',
    );
    expect(() => parseOptionalFloat('5', 'field', t, { max: 3 })).toThrow(
      'pages.audio.validation.max',
    );
    expect(() => parseOptionalFloat('0', 'field', t, { exclusiveMin: 0 })).toThrow(
      'pages.audio.validation.exclusiveMin',
    );
  });
});
