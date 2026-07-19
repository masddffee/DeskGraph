import { invoke } from '@tauri-apps/api/core';

export const REFRESH_CLEANUP_INBOX_COMMAND = 'refresh_cleanup_inbox';
export const GET_CLEANUP_SOURCE_DETAIL_COMMAND = 'get_cleanup_source_detail';
export const CREATE_CLEANUP_PREVIEW_COMMAND = 'create_cleanup_preview';

export type CleanupSourceKind = 'exact_duplicate' | 'version' | 'screenshot_review_group';
export type CleanupSourceMemberRole =
  'duplicate_candidate' | 'older_version' | 'newer_version' | 'screenshot_candidate';
export type CleanupSelectionRule =
  'either_member_is_target' | 'older_target_newer_keeper' | 'single_target_no_keeper';

export interface CleanupSourceDetailMember {
  node_id: number;
  display_path: string;
  size_bytes: number;
  role: CleanupSourceMemberRole;
}

export interface CleanupSourceDetail {
  api_version: 'deskgraph.cleanup-source-detail.v1';
  scope_id: number;
  source_kind: CleanupSourceKind;
  source_id: number;
  source_observation_id: number;
  members: CleanupSourceDetailMember[];
  selection_rule: CleanupSelectionRule;
  current_evidence: true;
  user_requested_paths: true;
  action_authorized: false;
  execution_available: false;
}

export type CleanupActionPolicyCheck =
  | 'explicit_authorized_scope'
  | 'active_scope_grant'
  | 'suggested_source'
  | 'exact_source_observation'
  | 'selected_member'
  | 'keeper_distinct_when_present'
  | 'present_manifest_file'
  | 'strong_target_identity'
  | 'read_only_handle_identity_matches'
  | 'target_hash_bound'
  | 'keeper_snapshot_and_hash_bound_when_present';

export interface CleanupActionPlanPreview {
  api_version: 'deskgraph.cleanup-action-plan-preview.v1';
  plan_id: number;
  operation: 'system_trash_preview';
  state: 'previewed';
  scope_id: number;
  source_kind: CleanupSourceKind;
  source_id: number;
  source_observation_id: number;
  keeper_node_id: number | null;
  target_node_id: number;
  expected_bytes: number;
  keeper_hash_bound: boolean;
  policy: {
    api_version: 'deskgraph.cleanup-action-policy.v1';
    checks: CleanupActionPolicyCheck[];
    confirmation_required: true;
    action_authorized: false;
    execution_available: false;
  };
  journal_sequence: 1;
  created_at_unix_ms: number;
}

export interface SmartCleanupInboxItem {
  source_kind: CleanupSourceKind;
  source_id: number;
  source_observation_id: number;
  scope_id: number;
  state: 'suggested';
  member_count: number;
  confidence_basis_points: number;
  observed_at_unix_ms: number;
  current_evidence: true;
  verification_required: true;
  review_assistance_only: true;
  cleanup_authorized: false;
}

export interface SmartCleanupInbox {
  api_version: 'deskgraph.smart-cleanup-inbox.v1';
  scope_id: number;
  items: SmartCleanupInboxItem[];
  evaluated_source_count: number;
  not_current_source_count: number;
  bounded_source_limit: number;
  evaluation_complete: boolean;
  action_authorized: false;
}

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isId(value: unknown): value is number {
  return isCount(value) && value > 0;
}

function isCleanupSourceKind(value: unknown): value is CleanupSourceKind {
  return value === 'exact_duplicate' || value === 'version' || value === 'screenshot_review_group';
}

function hasOnlyKeys(value: Record<string, unknown>, allowedKeys: readonly string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === allowedKeys.length && keys.every((key) => allowedKeys.includes(key));
}

function sourceShapeIsValid(value: Record<string, unknown>): boolean {
  if (value.source_kind === 'exact_duplicate') {
    return value.member_count === 2 && value.confidence_basis_points === 10_000;
  }
  if (value.source_kind === 'version') {
    return value.member_count === 2 && value.confidence_basis_points === 9_000;
  }
  if (value.source_kind === 'screenshot_review_group') {
    return (
      isCount(value.member_count) &&
      value.member_count >= 2 &&
      value.member_count <= 20 &&
      value.confidence_basis_points === 6_000
    );
  }
  return false;
}

export function parseSmartCleanupInboxItem(value: unknown): SmartCleanupInboxItem {
  if (!isRecord(value)) throw new Error('Invalid smart cleanup inbox item response');
  const valid =
    hasOnlyKeys(value, [
      'source_kind',
      'source_id',
      'source_observation_id',
      'scope_id',
      'state',
      'member_count',
      'confidence_basis_points',
      'observed_at_unix_ms',
      'current_evidence',
      'verification_required',
      'review_assistance_only',
      'cleanup_authorized',
    ]) &&
    sourceShapeIsValid(value) &&
    isId(value.source_id) &&
    isId(value.source_observation_id) &&
    isId(value.scope_id) &&
    value.state === 'suggested' &&
    isCount(value.observed_at_unix_ms) &&
    value.current_evidence === true &&
    value.verification_required === true &&
    value.review_assistance_only === true &&
    value.cleanup_authorized === false;
  if (!valid) throw new Error('Invalid smart cleanup inbox item response');
  return value as unknown as SmartCleanupInboxItem;
}

export function parseSmartCleanupInbox(value: unknown): SmartCleanupInbox {
  if (!isRecord(value)) throw new Error('Invalid smart cleanup inbox response');
  const valid =
    hasOnlyKeys(value, [
      'api_version',
      'scope_id',
      'items',
      'evaluated_source_count',
      'not_current_source_count',
      'bounded_source_limit',
      'evaluation_complete',
      'action_authorized',
    ]) &&
    value.api_version === 'deskgraph.smart-cleanup-inbox.v1' &&
    isId(value.scope_id) &&
    Array.isArray(value.items) &&
    value.items.every((item) => {
      try {
        return parseSmartCleanupInboxItem(item).scope_id === value.scope_id;
      } catch {
        return false;
      }
    }) &&
    isCount(value.evaluated_source_count) &&
    isCount(value.not_current_source_count) &&
    value.bounded_source_limit === 20 &&
    value.items.length <= value.evaluated_source_count &&
    value.evaluated_source_count <= value.bounded_source_limit &&
    value.not_current_source_count <= value.evaluated_source_count &&
    typeof value.evaluation_complete === 'boolean' &&
    value.action_authorized === false;
  if (!valid) throw new Error('Invalid smart cleanup inbox response');
  return value as unknown as SmartCleanupInbox;
}

function parseCleanupSourceDetailMember(value: unknown): CleanupSourceDetailMember {
  if (!isRecord(value)) throw new Error('Invalid cleanup source detail member response');
  const valid =
    hasOnlyKeys(value, ['node_id', 'display_path', 'size_bytes', 'role']) &&
    isId(value.node_id) &&
    typeof value.display_path === 'string' &&
    value.display_path.length > 0 &&
    value.display_path.length <= 65_536 &&
    isCount(value.size_bytes) &&
    value.size_bytes <= 8 * 1024 * 1024 * 1024 &&
    (value.role === 'duplicate_candidate' ||
      value.role === 'older_version' ||
      value.role === 'newer_version' ||
      value.role === 'screenshot_candidate');
  if (!valid) throw new Error('Invalid cleanup source detail member response');
  return value as unknown as CleanupSourceDetailMember;
}

function detailShapeIsValid(value: Record<string, unknown>): boolean {
  if (!Array.isArray(value.members) || !isCleanupSourceKind(value.source_kind)) return false;
  let members: CleanupSourceDetailMember[];
  try {
    members = value.members.map(parseCleanupSourceDetailMember);
  } catch {
    return false;
  }
  if (new Set(members.map((member) => member.node_id)).size !== members.length) return false;
  if (
    value.source_kind === 'exact_duplicate' &&
    (value.selection_rule !== 'either_member_is_target' ||
      members.length !== 2 ||
      members.some((member) => member.role !== 'duplicate_candidate'))
  ) {
    return false;
  }
  if (
    value.source_kind === 'version' &&
    (value.selection_rule !== 'older_target_newer_keeper' ||
      members.length !== 2 ||
      members.filter((member) => member.role === 'older_version').length !== 1 ||
      members.filter((member) => member.role === 'newer_version').length !== 1)
  ) {
    return false;
  }
  return !(
    value.source_kind === 'screenshot_review_group' &&
    (value.selection_rule !== 'single_target_no_keeper' ||
      members.length < 2 ||
      members.length > 20 ||
      members.some((member) => member.role !== 'screenshot_candidate'))
  );
}

export function parseCleanupSourceDetail(value: unknown): CleanupSourceDetail {
  if (!isRecord(value)) throw new Error('Invalid cleanup source detail response');
  const valid =
    hasOnlyKeys(value, [
      'api_version',
      'scope_id',
      'source_kind',
      'source_id',
      'source_observation_id',
      'members',
      'selection_rule',
      'current_evidence',
      'user_requested_paths',
      'action_authorized',
      'execution_available',
    ]) &&
    value.api_version === 'deskgraph.cleanup-source-detail.v1' &&
    isId(value.scope_id) &&
    isId(value.source_id) &&
    isId(value.source_observation_id) &&
    value.current_evidence === true &&
    value.user_requested_paths === true &&
    value.action_authorized === false &&
    value.execution_available === false &&
    detailShapeIsValid(value);
  if (!valid) throw new Error('Invalid cleanup source detail response');
  return value as unknown as CleanupSourceDetail;
}

const cleanupPolicyChecks: readonly CleanupActionPolicyCheck[] = [
  'explicit_authorized_scope',
  'active_scope_grant',
  'suggested_source',
  'exact_source_observation',
  'selected_member',
  'keeper_distinct_when_present',
  'present_manifest_file',
  'strong_target_identity',
  'read_only_handle_identity_matches',
  'target_hash_bound',
  'keeper_snapshot_and_hash_bound_when_present',
];

function parseCleanupActionPolicy(value: unknown): CleanupActionPlanPreview['policy'] {
  if (!isRecord(value)) throw new Error('Invalid cleanup action policy response');
  const checks = Array.isArray(value.checks) ? value.checks : null;
  const valid =
    hasOnlyKeys(value, [
      'api_version',
      'checks',
      'confirmation_required',
      'action_authorized',
      'execution_available',
    ]) &&
    value.api_version === 'deskgraph.cleanup-action-policy.v1' &&
    checks !== null &&
    checks.length === cleanupPolicyChecks.length &&
    cleanupPolicyChecks.every((check, index) => checks[index] === check) &&
    value.confirmation_required === true &&
    value.action_authorized === false &&
    value.execution_available === false;
  if (!valid) throw new Error('Invalid cleanup action policy response');
  return value as unknown as CleanupActionPlanPreview['policy'];
}

export function parseCleanupActionPlanPreview(value: unknown): CleanupActionPlanPreview {
  if (!isRecord(value)) throw new Error('Invalid cleanup action preview response');
  const valid =
    hasOnlyKeys(value, [
      'api_version',
      'plan_id',
      'operation',
      'state',
      'scope_id',
      'source_kind',
      'source_id',
      'source_observation_id',
      'keeper_node_id',
      'target_node_id',
      'expected_bytes',
      'keeper_hash_bound',
      'policy',
      'journal_sequence',
      'created_at_unix_ms',
    ]) &&
    value.api_version === 'deskgraph.cleanup-action-plan-preview.v1' &&
    value.operation === 'system_trash_preview' &&
    value.state === 'previewed' &&
    isId(value.plan_id) &&
    isId(value.scope_id) &&
    isCleanupSourceKind(value.source_kind) &&
    isId(value.source_id) &&
    isId(value.source_observation_id) &&
    (value.keeper_node_id === null || isId(value.keeper_node_id)) &&
    isId(value.target_node_id) &&
    value.keeper_node_id !== value.target_node_id &&
    isCount(value.expected_bytes) &&
    value.expected_bytes <= 8 * 1024 * 1024 * 1024 &&
    typeof value.keeper_hash_bound === 'boolean' &&
    (value.source_kind === 'screenshot_review_group'
      ? value.keeper_node_id === null && value.keeper_hash_bound === false
      : isId(value.keeper_node_id) && value.keeper_hash_bound === true) &&
    value.journal_sequence === 1 &&
    isCount(value.created_at_unix_ms);
  if (!valid) throw new Error('Invalid cleanup action preview response');
  parseCleanupActionPolicy(value.policy);
  return value as unknown as CleanupActionPlanPreview;
}

/** Explicit, scoped, read-only evaluation. It never authorizes a file action. */
export async function refreshSmartCleanupInbox(
  scopeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<SmartCleanupInbox> {
  if (!isId(scopeId)) throw new Error('Invalid smart cleanup inbox scope');
  return parseSmartCleanupInbox(await invokeCommand(REFRESH_CLEANUP_INBOX_COMMAND, { scopeId }));
}

/** Explicit transient detail. Paths may be displayed only in the current local review surface. */
export async function getCleanupSourceDetail(
  item: SmartCleanupInboxItem,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<CleanupSourceDetail> {
  const parsedItem = parseSmartCleanupInboxItem(item);
  const detail = parseCleanupSourceDetail(
    await invokeCommand(GET_CLEANUP_SOURCE_DETAIL_COMMAND, {
      scopeId: parsedItem.scope_id,
      sourceKind: parsedItem.source_kind,
      sourceId: parsedItem.source_id,
      sourceObservationId: parsedItem.source_observation_id,
    }),
  );
  if (
    detail.scope_id !== parsedItem.scope_id ||
    detail.source_kind !== parsedItem.source_kind ||
    detail.source_id !== parsedItem.source_id
  ) {
    throw new Error('Cleanup source detail did not match the requested source');
  }
  return detail;
}

/** Creates one immutable preview. It does not confirm or authorize a file action. */
export async function createCleanupActionPreview(
  detail: CleanupSourceDetail,
  targetNodeId: number,
  keeperNodeId: number | null,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<CleanupActionPlanPreview> {
  const parsedDetail = parseCleanupSourceDetail(detail);
  const target = parsedDetail.members.find((member) => member.node_id === targetNodeId);
  const keeper =
    keeperNodeId === null
      ? null
      : parsedDetail.members.find((member) => member.node_id === keeperNodeId);
  if (!target || (keeperNodeId !== null && !keeper) || targetNodeId === keeperNodeId) {
    throw new Error('Invalid cleanup preview selection');
  }
  if (
    (parsedDetail.selection_rule === 'either_member_is_target' && keeperNodeId === null) ||
    (parsedDetail.selection_rule === 'older_target_newer_keeper' &&
      (target.role !== 'older_version' || keeper?.role !== 'newer_version')) ||
    (parsedDetail.selection_rule === 'single_target_no_keeper' && keeperNodeId !== null)
  ) {
    throw new Error('Invalid cleanup preview selection');
  }
  const preview = parseCleanupActionPlanPreview(
    await invokeCommand(CREATE_CLEANUP_PREVIEW_COMMAND, {
      scopeId: parsedDetail.scope_id,
      sourceKind: parsedDetail.source_kind,
      sourceId: parsedDetail.source_id,
      sourceObservationId: parsedDetail.source_observation_id,
      targetNodeId,
      keeperNodeId,
    }),
  );
  if (
    preview.scope_id !== parsedDetail.scope_id ||
    preview.source_kind !== parsedDetail.source_kind ||
    preview.source_id !== parsedDetail.source_id ||
    preview.source_observation_id !== parsedDetail.source_observation_id ||
    preview.target_node_id !== targetNodeId ||
    preview.keeper_node_id !== keeperNodeId
  ) {
    throw new Error('Cleanup action preview did not match the selected source');
  }
  return preview;
}
