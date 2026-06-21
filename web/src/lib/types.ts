export type EngineUsed = 'http' | 'browser_cdp';
export type ConditionKind =
  | 'text'
  | 'selector'
  | 'selector_text'
  | 'price'
  | 'price_observed'
  | 'redirect';

export interface ConditionResult {
  condition_id: string;
  kind: ConditionKind;
  matched: boolean;
  evidence: string[];
  observed_price_cents: number | null;
  error: string | null;
  scenario_id?: string | null;
  scenario_label?: string | null;
}

export interface TargetStatus {
  target_id: string;
  name: string;
  url: string;
  enabled: boolean;
  render: RenderPlan;
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
  discord_configured: boolean;
  renderer_enabled: boolean;
  renderer_configured: boolean;
  renderer_backend: 'cloakbrowser' | 'cdp';
}

export interface IntegrationCheck {
  name: string;
  ok: boolean;
  message: string;
}

export interface IntegrationTestResponse {
  configured: boolean;
  ok: boolean;
  message: string;
  checks: IntegrationCheck[];
}

export interface RendererTestResponse extends IntegrationTestResponse {
  enabled: boolean;
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

export type ConditionWireKind =
  | 'text_appears'
  | 'text_disappears'
  | 'selector_exists'
  | 'selector_missing'
  | 'selector_text_contains'
  | 'selector_text_not_contains'
  | 'price_below'
  | 'price_above'
  | 'price_changed'
  | 'url_redirects'
  | 'url_unchanged';

export interface ConditionInput {
  id?: string | null;
  kind: ConditionWireKind;
  value?: string;
  selector?: string;
  threshold_cents?: number;
  price_selector?: string;
}

export type RenderPolicy = 'http_only' | 'auto' | 'render_first';
export type ScenarioMatch = 'any' | 'all';
export type RenderOperation = 'wait_for' | 'wait_for_text' | 'click' | 'select';

export interface RenderStep {
  op: RenderOperation;
  selector?: string | null;
  text?: string | null;
  option_text?: string | null;
  option_value?: string | null;
  value?: string | null;
  timeout_ms?: number | null;
  settle_ms?: number | null;
}

export interface RenderScenario {
  id: string;
  label: string;
  steps: RenderStep[];
}

export interface RenderPlan {
  policy: RenderPolicy;
  fingerprint_seed?: string | null;
  wait_ms?: number | null;
  scenario_match: ScenarioMatch;
  steps: RenderStep[];
  scenarios: RenderScenario[];
}

export interface TargetInput {
  name: string;
  url: string;
  enabled?: boolean;
  interval_secs?: number;
  render?: RenderPlan;
  conditions: ConditionInput[];
}

export interface WatchConfig {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  interval_secs: number | null;
  render: RenderPlan;
  conditions: ConditionInput[];
}

export interface WatchDetailResponse {
  config: WatchConfig;
  status: TargetStatus;
}

export interface DryRunDiagnostic {
  kind: string;
  message: string;
}

export interface DryRunResponse {
  matched: boolean | null;
  engine_used: EngineUsed | null;
  duration_ms: number;
  final_url: string | null;
  evidence: string[];
  condition_results: ConditionResult[];
  diagnostics: DryRunDiagnostic[];
  artifacts: {
    html_url?: string | null;
    screenshot_url?: string | null;
  };
  error: string | null;
}

export interface CheckRun {
  checked_at: string;
  matched: boolean | null;
  engine_used: EngineUsed | null;
  price_cents: number | null;
  evidence: string[];
  condition_results: ConditionResult[];
  error: string | null;
}

export interface OpsResponse {
  status: string;
  persistence_backend: string;
  discord_configured: boolean;
  renderer_enabled: boolean;
  renderer_configured: boolean;
  renderer_available: boolean;
  renderer_backend: 'cloakbrowser' | 'cdp';
  scheduler: {
    running_targets: number;
    renderer_available: boolean;
  };
  targets: {
    total: number;
    enabled: number;
    matched: number;
    no_match: number;
    error: number;
    unknown: number;
    disabled: number;
  };
  recent_errors: Array<{
    target_id: string;
    name: string;
    error: string;
    at: string | null;
  }>;
}
