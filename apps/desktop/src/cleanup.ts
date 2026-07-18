import { invoke } from '@tauri-apps/api/core';

export const REFRESH_CLEANUP_INBOX_COMMAND = 'refresh_cleanup_inbox';

export type CleanupSourceKind = 'exact_duplicate' | 'version' | 'screenshot_review_group';

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

/** Explicit, scoped, read-only evaluation. It never authorizes a file action. */
export async function refreshSmartCleanupInbox(
  scopeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<SmartCleanupInbox> {
  if (!isId(scopeId)) throw new Error('Invalid smart cleanup inbox scope');
  return parseSmartCleanupInbox(await invokeCommand(REFRESH_CLEANUP_INBOX_COMMAND, { scopeId }));
}
