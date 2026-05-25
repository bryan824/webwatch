// web/src/lib/format.test.ts
import { describe, it, expect } from 'vitest';
import { formatPrice, formatRelative } from './format';

describe('formatPrice', () => {
  it('formats cents to USD', () => expect(formatPrice(3800)).toBe('$38.00'));
  it('handles null', () => expect(formatPrice(null)).toBe('—'));
});

describe('formatRelative', () => {
  it('handles null', () => expect(formatRelative(null)).toBe('never'));
  it('returns a string for a valid iso', () => {
    expect(typeof formatRelative(new Date(Date.now() - 60_000).toISOString())).toBe('string');
  });
});
