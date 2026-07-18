import { invoke } from '@tauri-apps/api/core';

export const CREATE_RENAME_PREVIEW_COMMAND = 'create_rename_preview';
export const RECENT_ACTION_PLANS_COMMAND = 'recent_action_plans';

export type ActionExecutionStrategy = 'direct' | 'case_only_staged';
export type ActionPlanState =
  | 'previewed'
  | 'execute_requested'
  | 'direct_rename_intent'
  | 'executed'
  | 'undo_requested'
  | 'undo_rename_intent'
  | 'undone'
  | 'needs_attention';
export type ActionPolicyCheck =
  | 'explicit_authorized_scope'
  | 'present_manifest_file'
  | 'canonical_source_contained'
  | 'source_identity_matches'
  | 'read_only_handle_identity_matches'
  | 'portable_single_component_name'
  | 'same_canonical_parent'
  | 'destination_contained'
  | 'destination_available';

export interface ActionPolicyReport {
  api_version: 'deskgraph.action-policy.v1';
  decision: 'allowed';
  checks: ActionPolicyCheck[];
}

export interface ActionPlanPreview {
  api_version: 'deskgraph.action-plan.v2';
  plan_id: number;
  operation: 'rename';
  state: 'previewed';
  scope_id: number;
  node_id: number;
  source_path: string;
  destination_path: string;
  execution_strategy: ActionExecutionStrategy;
  policy: ActionPolicyReport;
  journal_sequence: number;
  created_at_unix_ms: number;
}

export interface ActionPlanSummary {
  api_version: 'deskgraph.action-plan-summary.v2';
  plan_id: number;
  operation: 'rename';
  state: ActionPlanState;
  scope_id: number;
  node_id: number;
  execution_strategy: ActionExecutionStrategy;
  journal_sequence: number;
  created_at_unix_ms: number;
}

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const POLICY_CHECKS: readonly ActionPolicyCheck[] = [
  'explicit_authorized_scope',
  'present_manifest_file',
  'canonical_source_contained',
  'source_identity_matches',
  'read_only_handle_identity_matches',
  'portable_single_component_name',
  'same_canonical_parent',
  'destination_contained',
  'destination_available',
] as const;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isId(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0;
}

function isTimestamp(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isExecutionStrategy(value: unknown): value is ActionExecutionStrategy {
  return value === 'direct' || value === 'case_only_staged';
}

function isActionPlanState(value: unknown): value is ActionPlanState {
  return (
    value === 'previewed' ||
    value === 'execute_requested' ||
    value === 'direct_rename_intent' ||
    value === 'executed' ||
    value === 'undo_requested' ||
    value === 'undo_rename_intent' ||
    value === 'undone' ||
    value === 'needs_attention'
  );
}

const SUMMARY_FIELDS = [
  'api_version',
  'plan_id',
  'operation',
  'state',
  'scope_id',
  'node_id',
  'execution_strategy',
  'journal_sequence',
  'created_at_unix_ms',
] as const;

const PREVIEW_FIELDS = [...SUMMARY_FIELDS, 'source_path', 'destination_path', 'policy'] as const;

function hasExactFields(value: Record<string, unknown>, fields: readonly string[]): boolean {
  const allowedFields = new Set(fields);
  return (
    Object.keys(value).length === allowedFields.size &&
    Object.keys(value).every((key) => allowedFields.has(key))
  );
}

function parsePolicy(value: unknown): ActionPolicyReport {
  if (!isRecord(value) || !hasExactFields(value, ['api_version', 'decision', 'checks'])) {
    throw new Error('Invalid action policy response');
  }
  const checks = value.checks;
  if (!Array.isArray(checks)) throw new Error('Invalid action policy response');
  const checksAreExact =
    checks.length === POLICY_CHECKS.length &&
    POLICY_CHECKS.every((check) => checks.includes(check)) &&
    new Set(checks).size === POLICY_CHECKS.length;
  if (
    value.api_version !== 'deskgraph.action-policy.v1' ||
    value.decision !== 'allowed' ||
    !checksAreExact
  ) {
    throw new Error('Invalid action policy response');
  }
  return value as unknown as ActionPolicyReport;
}

function hasValidCommonFields(value: Record<string, unknown>): boolean {
  return (
    isId(value.plan_id) &&
    value.operation === 'rename' &&
    isActionPlanState(value.state) &&
    isId(value.scope_id) &&
    isId(value.node_id) &&
    isExecutionStrategy(value.execution_strategy) &&
    isId(value.journal_sequence) &&
    isTimestamp(value.created_at_unix_ms)
  );
}

export function parseActionPlanPreview(value: unknown): ActionPlanPreview {
  if (!isRecord(value) || !hasExactFields(value, PREVIEW_FIELDS) || !hasValidCommonFields(value)) {
    throw new Error('Invalid action preview response');
  }
  if (
    value.api_version !== 'deskgraph.action-plan.v2' ||
    value.state !== 'previewed' ||
    typeof value.source_path !== 'string' ||
    value.source_path.length === 0 ||
    typeof value.destination_path !== 'string' ||
    value.destination_path.length === 0 ||
    value.source_path === value.destination_path
  ) {
    throw new Error('Invalid action preview response');
  }
  parsePolicy(value.policy);
  return value as unknown as ActionPlanPreview;
}

export function parseActionPlanSummary(value: unknown): ActionPlanSummary {
  if (
    !isRecord(value) ||
    value.api_version !== 'deskgraph.action-plan-summary.v2' ||
    !hasExactFields(value, SUMMARY_FIELDS) ||
    !hasValidCommonFields(value)
  ) {
    throw new Error('Invalid action plan summary response');
  }
  return value as unknown as ActionPlanSummary;
}

export function parseActionPlanSummaries(value: unknown): ActionPlanSummary[] {
  if (!Array.isArray(value)) throw new Error('Invalid action plan list response');
  return value.map(parseActionPlanSummary);
}

export async function createRenamePreview(
  scopeId: number,
  sourcePath: string,
  newName: string,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ActionPlanPreview> {
  return parseActionPlanPreview(
    await invokeCommand(CREATE_RENAME_PREVIEW_COMMAND, { scopeId, sourcePath, newName }),
  );
}

export async function loadRecentActionPlans(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ActionPlanSummary[]> {
  return parseActionPlanSummaries(await invokeCommand(RECENT_ACTION_PLANS_COMMAND));
}
