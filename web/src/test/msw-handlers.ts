// web/src/test/msw-handlers.ts
import { http, HttpResponse } from 'msw';
import type { TargetStatus } from '$lib/api/types';

export const sampleTargets: TargetStatus[] = [
  {
    target_id: 'campfire-mug', name: 'Campfire Mug', url: 'https://example.com/products/campfire-mug',
    matched: true, engine_used: 'http', price_cents: 3800,
    evidence: ['"Add to cart" found'], condition_results: [
      { condition_id: 'condition-1', kind: 'text', matched: true, evidence: ['"Add to cart" found'], observed_price_cents: null, error: null }
    ],
    last_success_at: new Date().toISOString(), last_error_at: null, last_error: null, last_alert_at: null
  },
  {
    target_id: 'sale-price', name: 'Sale Price Watch', url: 'https://example.com/sale',
    matched: null, engine_used: null, price_cents: null, evidence: [], condition_results: [],
    last_success_at: null, last_error_at: null, last_error: null, last_alert_at: null
  }
];

export const handlers = [
  http.get('/targets', () => HttpResponse.json(sampleTargets)),
  http.get('/targets/:id/status', ({ params }) => {
    const t = sampleTargets.find((s) => s.target_id === params.id);
    return t ? HttpResponse.json(t) : HttpResponse.json({ error: 'target not found' }, { status: 404 });
  }),
  http.post('/targets/reload', () =>
    HttpResponse.json({ added: [], removed: [], changed: ['campfire-mug'], unchanged: ['sale-price'] })),
  http.post('/notify/status', () =>
    HttpResponse.json({ sent: true, summary: '2 targets checked', statuses: sampleTargets }))
];
