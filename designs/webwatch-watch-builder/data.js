// data.js — mocked "world" the prototype reads.
//
// PAGES stand in for the live web. The dry-run engine resolves a condition's
// locator against a page's `elements` (precise selector => exact read) and falls
// back to `text` (whole-page => keyword/AI). `jsRendered` drives the http-vs-browser
// engine badge. In production these come from a real fetch; here they're fixed so
// the dry-run is deterministic and demoable.

export const PAGES = [
  {
    key: 'campfire-mug',
    url: 'store.example.com/products/campfire-mug',
    title: 'Campfire Enamel Mug',
    jsRendered: false,
    elements: [
      { selector: '.add-to-cart', xpath: "//button[@id='buy']", text: 'Add to cart', within: ['.product', 'main', 'body'] },
      { selector: '.price', xpath: "//span[@class='price']", text: '$24.00', price: true, within: ['.product', '.product__buy', 'main', 'body'] },
      { selector: '.product-title', text: 'Campfire Enamel Mug', within: ['.product', 'main', 'body'] },
      { selector: '.availability', text: 'In stock', within: ['.product', 'main', 'body'] },
    ],
    text: 'Campfire Enamel Mug. In stock. Add to cart — $24.00. Free shipping over $35. Enamel over steel, 12oz.',
  },
  {
    key: 'rtx-4070',
    url: 'shop.example.com/gpu/rtx-4070',
    title: 'GeForce RTX 4070 Founders Edition',
    jsRendered: false,
    elements: [
      { selector: '.add-to-cart', text: 'Add to cart', within: ['.buybox', 'main', 'body'] },
      { selector: '.product-price', xpath: "//div[@class='product-price']", text: '$429.99', price: true, within: ['.buybox', 'main', 'body'] },
      { selector: '.product-price .was', text: '$549.99', price: true, within: ['.buybox', 'main', 'body'] },
      { selector: '.title', text: 'GeForce RTX 4070 Founders Edition', within: ['main', 'body'] },
    ],
    text: 'GeForce RTX 4070 Founders Edition. Was $549.99 now $429.99. Add to cart. In stock at 3 stores.',
  },
  {
    key: 'concert-tickets',
    url: 'tickets.example.org/events/12345',
    title: 'Live at the Greek — Aug 14',
    jsRendered: true, // client-rendered: forces the browser engine
    elements: [
      { selector: '.availability', text: 'Sold out', within: ['.event', 'main', 'body'] },
      { selector: '.event-title', text: 'Live at the Greek', within: ['.event', 'main', 'body'] },
      { selector: '.price-from', text: '$78.50', price: true, within: ['.event', 'main', 'body'] },
    ],
    text: 'Live at the Greek — Aug 14. Sold out. Resale from $78.50. Join the waitlist.',
  },
  {
    key: 'careers-rust',
    url: 'careers.example.net/teams/platform',
    title: 'Platform — Open Roles',
    jsRendered: false,
    elements: [
      { selector: '.role', text: 'Senior Frontend Engineer', within: ['.roles', 'main', 'body'] },
      { selector: '.role', text: 'Engineering Manager, Data', within: ['.roles', 'main', 'body'] },
      { selector: '.team-title', text: 'Platform — Open Roles', within: ['main', 'body'] },
      // NOTE: no a[href*='rust-engineer'] — an "element is present" check fails here.
    ],
    text: 'Platform — Open Roles. Senior Frontend Engineer. Engineering Manager, Data. See all teams.',
  },
];

export const PAGE_BY_URL = Object.fromEntries(PAGES.map((p) => [p.url, p]));
export const PAGE_BY_KEY = Object.fromEntries(PAGES.map((p) => [p.key, p]));

// Resolve whatever the user typed in the URL field to a known mock page.
export function resolvePage(rawUrl) {
  if (!rawUrl) return null;
  const u = rawUrl.trim().replace(/^https?:\/\//, '').replace(/\/$/, '');
  return PAGE_BY_URL[u] || PAGES.find((p) => u && (p.url.includes(u) || u.includes(p.url))) || null;
}

// Saved watches that populate the list rail (each references a mock page).
export const SEED_WATCHES = [
  {
    id: 'campfire-mug',
    name: 'Campfire Mug — back in stock & under $25',
    url: 'store.example.com/products/campfire-mug',
    pageKey: 'campfire-mug',
    enabled: true,
    intervalSecs: 900,
    state: 'matched',
    lastCheck: '2m ago',
    lastEvidence: 'page contains “Add to cart” · price $24.00',
    conditions: [
      { cid: 's1a', subject: 'text', op: 'contains', value: 'Add to cart', negate: false, locator: { type: 'page', query: '' }, strategy: 'auto', result: null },
      { cid: 's1b', subject: 'price', op: 'below', value: '25.00', negate: false, locator: { type: 'css', query: '.price' }, strategy: 'auto', result: null },
    ],
  },
  {
    id: 'rtx-4070',
    name: 'RTX 4070 — under $400',
    url: 'shop.example.com/gpu/rtx-4070',
    pageKey: 'rtx-4070',
    enabled: true,
    intervalSecs: 900,
    state: 'watching',
    lastCheck: '4m ago',
    lastEvidence: 'price $429.99 — above $400.00',
    conditions: [
      { cid: 's2a', subject: 'price', op: 'below', value: '400.00', negate: false, locator: { type: 'css', query: '.product-price' }, strategy: 'auto', result: null },
    ],
  },
  {
    id: 'concert-tickets',
    name: 'Greek tickets — on sale',
    url: 'tickets.example.org/events/12345',
    pageKey: 'concert-tickets',
    enabled: true,
    intervalSecs: 300,
    state: 'browser',
    lastCheck: '1m ago',
    lastEvidence: '“Sold out” — uses browser engine',
    conditions: [
      { cid: 's3a', subject: 'value', op: 'contains', value: 'On sale', negate: false, locator: { type: 'css', query: '.availability' }, strategy: 'auto', result: null },
    ],
  },
  {
    id: 'careers-rust',
    name: 'Platform — Rust role posted',
    url: 'careers.example.net/teams/platform',
    pageKey: 'careers-rust',
    enabled: false,
    intervalSecs: 3600,
    state: 'paused',
    lastCheck: '—',
    lastEvidence: 'disabled',
    conditions: [
      { cid: 's4a', subject: 'element', op: 'exists', value: '', negate: false, locator: { type: 'css', query: "a[href*='rust-engineer']" }, strategy: 'auto', result: null },
    ],
  },
];
