import { describe, expect, it } from 'vitest';

import { MESSAGE_ANIMATIONS } from '../message-animations';

const PRESET_IDS = [
  'fade',
  'slide-up',
  'slide-side',
  'pop',
  'spring-bounce',
  'blur-fade',
  'scale-fade',
] as const;

describe('message animations', () => {
  it('exposes all seven presets keyed by id', () => {
    expect(Object.keys(MESSAGE_ANIMATIONS).toSorted()).toEqual([...PRESET_IDS].toSorted());
  });

  it.each(PRESET_IDS)('round-trips preset %s by id', (id) => {
    expect(MESSAGE_ANIMATIONS[id].id).toBe(id);
  });

  it('gives every preset a name and the initial/animate/exit variants', () => {
    for (const preset of Object.values(MESSAGE_ANIMATIONS)) {
      expect(typeof preset.name).toBe('string');
      expect(preset.name.length).toBeGreaterThan(0);
      expect(preset.variants).toHaveProperty('initial');
      expect(preset.variants).toHaveProperty('animate');
      expect(preset.variants).toHaveProperty('exit');
    }
  });
});
