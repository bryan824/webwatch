import { describe, expect, it } from 'vitest';
import { blankCondition, fromWire, toWire, validateAndBuild } from './conditions';

it('maps structured price conditions to backend wire format', () => {
  const condition = blankCondition({
    cid: 'price-under',
    subject: 'price',
    op: 'below',
    value: '25.50',
    locator: { type: 'css', query: '.price' },
  });

  expect(toWire(condition)).toEqual({
    id: 'price-under',
    kind: 'price_below',
    price_selector: '.price',
    threshold_cents: 2550,
  });
});

describe('validateAndBuild', () => {
  it('rejects selector conditions without CSS selectors', () => {
    const result = validateAndBuild([
      blankCondition({ subject: 'element', op: 'exists', locator: { type: 'page', query: '' } }),
    ]);

    expect(result).toEqual({ ok: false, error: 'Element condition requires a CSS selector.' });
  });

  it('round-trips editable wire conditions', () => {
    const condition = fromWire({ id: 'stock', kind: 'selector_text_contains', selector: '.buy', value: 'Add to cart' });
    const result = validateAndBuild([condition]);

    expect(result).toEqual({
      ok: true,
      conditions: [{ id: 'stock', kind: 'selector_text_contains', selector: '.buy', value: 'Add to cart' }],
    });
  });

  it('generates a stable frontend id for legacy wire conditions without ids', () => {
    const condition = fromWire({ kind: 'text_appears', value: 'Add to cart' });

    expect(condition.cid).toMatch(/^c/);
    expect(toWire(condition).id).toBe(condition.cid);
  });
});
