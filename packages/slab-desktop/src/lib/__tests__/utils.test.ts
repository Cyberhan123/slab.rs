import { describe, it, expect } from 'vitest';
import { cn } from '../utils';

describe('cn (className utility)', () => {
  it('should merge class names correctly', () => {
    expect(cn('text-red-500', 'bg-blue-500')).toBe('text-red-500 bg-blue-500');
  });

  it('should handle conflicting tailwind classes', () => {
    expect(cn('text-red-500', 'text-blue-500')).toBe('text-blue-500');
    expect(cn('p-4', 'p-8')).toBe('p-8');
  });

  it('should handle conditional classes', () => {
    expect(cn('base-class', true && 'active', false && 'inactive')).toBe('base-class active');
  });

  it('should handle undefined and null values', () => {
    expect(cn('base-class', undefined, null, 'another-class')).toBe('base-class another-class');
  });

  it('should handle empty strings', () => {
    expect(cn('base-class', '', 'another-class')).toBe('base-class another-class');
  });

  it('should handle arrays of classes', () => {
    expect(cn(['class1', 'class2'], 'class3')).toBe('class1 class2 class3');
  });

  it('should handle objects with boolean values', () => {
    expect(cn({ 'class1': true, 'class2': false, 'class3': true })).toBe('class1 class3');
  });

  it('should handle complex combinations', () => {
    const isActive = true;
    const size = 'lg';
    expect(
      cn('base-class', isActive && 'active', size === 'lg' && 'text-lg', ['array-class'])
    ).toBe('base-class active text-lg array-class');
  });

  it('should handle no arguments', () => {
    expect(cn()).toBe('');
  });

  it('should not remove duplicates (tailwind-merge behavior)', () => {
    // tailwind-merge doesn't remove general duplicates, only Tailwind conflicts
    expect(cn('class1', 'class2', 'class1')).toBe('class1 class2 class1');
  });

  it('should handle Tailwind conflict resolution', () => {
    expect(cn('text-sm text-lg', 'text-xl')).toBe('text-xl');
    expect(cn('p-4 px-2', 'px-8')).toBe('p-4 px-8');
  });
});
