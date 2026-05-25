// web/src/lib/api/types.ts
export type EngineUsed = 'http' | 'browser_cdp';
export type ConditionKind = 'text' | 'selector' | 'selector_text' | 'price' | 'price_observed';

export interface ConditionResult {
  condition_id: string;
  kind: ConditionKind;
  matched: boolean;
  evidence: string[];
  observed_price_cents: number | null;
  error: string | null;
}

export interface TargetStatus {
  target_id: string;
  name: string;
  url: string;
  matched: boolean | null;
  engine_used: EngineUsed | null;
  price_cents: number | null;
  evidence: string[];
  condition_results: ConditionResult[];
  last_success_at: string | null;
  last_error_at: string | null;
  last_error: string | null;
  last_alert_at: string | null;
}

export interface HealthResponse {
  status: string;
  persistence_backend: string;
}

export interface ReloadReport {
  added: string[];
  removed: string[];
  changed: string[];
  unchanged: string[];
}

export interface NotifyStatusResponse {
  sent: boolean;
  summary: string;
  statuses: TargetStatus[];
}
