import { describe, expect, it } from 'vitest';
import {
  blankAddTargetDefaults,
  blankConditionDraft,
  buildTargetInput,
  fieldsForCondition,
  type AddTargetDraft
} from './addTargetForm';

function draft(overrides: Partial<AddTargetDraft> = {}): AddTargetDraft {
  return {
    name: 'Campfire Mug',
    url: 'https://example.com/product',
    enabled: true,
    interval: '',
    conditions: [
      {
        kind: 'text_appears',
        value: 'Add to cart',
        selector: '',
        price: '',
        price_selector: ''
      }
    ],
    ...overrides
  };
}

describe('addTargetForm', () => {
  it('reports visible fields for each condition family', () => {
    expect(fieldsForCondition('text_appears')).toMatchObject({ value: true, selector: false });
    expect(fieldsForCondition('selector_exists')).toMatchObject({ value: false, selector: true });
    expect(fieldsForCondition('selector_text_contains')).toMatchObject({ value: true, selector: true });
    expect(fieldsForCondition('price_below')).toMatchObject({ threshold: true, priceSelector: true });
    expect(fieldsForCondition('price_changed')).toMatchObject({ threshold: false, priceSelector: true });
  });


  it('builds blank drafts and default form state', () => {
    expect(blankConditionDraft(7)).toEqual({
      id: 7,
      kind: 'text_appears',
      value: '',
      selector: '',
      price: '',
      price_selector: ''
    });
    expect(blankAddTargetDefaults(8)).toEqual({
      name: '',
      url: '',
      enabled: true,
      interval: '',
      conditions: [blankConditionDraft(8)]
    });
  });

  it('builds and trims a minimal text target', () => {
    const result = buildTargetInput(
      draft({
        name: ' Campfire Mug ',
        url: ' https://example.com/product ',
        conditions: [
          {
            kind: 'text_appears',
            value: ' Add to cart ',
            selector: '',
            price: '',
            price_selector: ''
          }
        ]
      })
    );

    expect(result).toEqual({
      ok: true,
      input: {
        name: 'Campfire Mug',
        url: 'https://example.com/product',
        enabled: true,
        conditions: [{ kind: 'text_appears', value: 'Add to cart' }]
      }
    });
  });

  it('rejects missing name and invalid URL', () => {
    expect(buildTargetInput(draft({ name: ' ' }))).toEqual({ ok: false, error: 'Name is required.' });
    expect(buildTargetInput(draft({ url: 'example.com' }))).toEqual({
      ok: false,
      error: 'Enter a valid absolute URL (https://…).'
    });
  });

  it('rejects missing required condition fields', () => {
    expect(
      buildTargetInput(draft({ conditions: [{ kind: 'text_appears', value: ' ', selector: '', price: '', price_selector: '' }] }))
    ).toEqual({ ok: false, error: 'A text value is required for the selected condition.' });

    expect(
      buildTargetInput(draft({ conditions: [{ kind: 'selector_exists', value: '', selector: ' ', price: '', price_selector: '' }] }))
    ).toEqual({ ok: false, error: 'A CSS selector is required for the selected condition.' });

    expect(
      buildTargetInput(draft({ conditions: [{ kind: 'price_below', value: '', selector: '', price: '', price_selector: '' }] }))
    ).toEqual({ ok: false, error: 'A price threshold (USD) is required.' });
  });

  it('builds selector text and preserves condition order', () => {
    const result = buildTargetInput(
      draft({
        conditions: [
          { kind: 'selector_exists', value: '', selector: ' button.buy ', price: '', price_selector: '' },
          { kind: 'selector_text_contains', value: ' Add to cart ', selector: ' button ', price: '', price_selector: '' }
        ]
      })
    );

    expect(result.ok && result.input.conditions).toEqual([
      { kind: 'selector_exists', selector: 'button.buy' },
      { kind: 'selector_text_contains', selector: 'button', value: 'Add to cart' }
    ]);
  });

  it('converts price dollars to cents and includes trimmed price selector', () => {
    const result = buildTargetInput(
      draft({
        conditions: [
          { kind: 'price_below', value: '', selector: '', price: '19.995', price_selector: ' .price ' }
        ]
      })
    );

    expect(result.ok && result.input.conditions[0]).toEqual({
      kind: 'price_below',
      threshold_cents: 2000,
      price_selector: '.price'
    });
  });

  it('converts positive interval minutes and omits blank invalid or non-positive intervals', () => {
    expect(buildTargetInput(draft({ interval: '15' }))).toMatchObject({
      ok: true,
      input: { interval_secs: 900 }
    });
    expect(buildTargetInput(draft({ interval: '' }))).toEqual(
      expect.objectContaining({ ok: true, input: expect.not.objectContaining({ interval_secs: expect.anything() }) })
    );
    expect(buildTargetInput(draft({ interval: 'nope' }))).toEqual(
      expect.objectContaining({ ok: true, input: expect.not.objectContaining({ interval_secs: expect.anything() }) })
    );
    expect(buildTargetInput(draft({ interval: '0' }))).toEqual(
      expect.objectContaining({ ok: true, input: expect.not.objectContaining({ interval_secs: expect.anything() }) })
    );
  });

  it('allows price_changed without a price selector and omits blank selector', () => {
    const result = buildTargetInput(
      draft({ conditions: [{ kind: 'price_changed', value: '', selector: '', price: '', price_selector: ' ' }] })
    );

    expect(result).toEqual({
      ok: true,
      input: {
        name: 'Campfire Mug',
        url: 'https://example.com/product',
        enabled: true,
        conditions: [{ kind: 'price_changed' }]
      }
    });
  });
});
