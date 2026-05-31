# Add Target Form State Seam

Date: 2026-05-30

## Goal

Complete the optional frontend seam by moving `AddTargetDialog` draft state helpers into the extracted form module while preserving UI and API behavior.

## Scope

Add pure helpers for blank condition drafts and reset defaults. Keep Svelte runes/local state in the component; avoid a larger `.svelte.ts` form object because the pure seam is already sufficient and lower risk.

## Verification

Run `cd web && npm test && npm run check`.
