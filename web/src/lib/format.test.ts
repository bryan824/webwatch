import { describe, expect, it } from 'vitest';
import { formatAbsolute } from './format';

describe('formatAbsolute', () => {
  it('renders absolute local date and time instead of relative labels', () => {
    const formatted = formatAbsolute('2026-06-21T05:04:47.000Z', 'en-US', 'UTC');

    expect(formatted).toBe('Jun 21, 2026, 05:04:47 UTC');
    expect(formatted).not.toMatch(/ago|yesterday|minute|hour|second/i);
  });

  it('keeps missing and invalid values readable', () => {
    expect(formatAbsolute(null)).toBe('never');
    expect(formatAbsolute('not-a-date')).toBe('not-a-date');
  });
});
