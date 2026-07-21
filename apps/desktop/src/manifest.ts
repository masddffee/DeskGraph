import { invoke } from '@tauri-apps/api/core';

export const MANIFEST_STATUS_COMMAND = 'manifest_status';
export const AUTHORIZED_SCOPES_COMMAND = 'authorized_scopes';
export const SELECT_AND_AUTHORIZE_SCOPES_COMMAND = 'select_and_authorize_scopes';
export const CREATE_SCAN_COMMAND = 'create_manifest_scan';
export const RUN_SCAN_COMMAND = 'run_manifest_scan';
export const SCAN_JOB_STATUS_COMMAND = 'scan_job_status';
export const RECENT_SCAN_JOBS_COMMAND = 'recent_scan_jobs';
export const PAUSE_SCAN_COMMAND = 'pause_manifest_scan';
export const RESUME_SCAN_COMMAND = 'resume_manifest_scan';
export const COVERAGE_POLICY_DETAIL_COMMAND = 'coverage_policy_detail';
export const SELECT_HARD_EXCLUSIONS_PREVIEW_COMMAND = 'select_hard_exclusions_preview';
export const CONFIRM_HARD_EXCLUSION_PREVIEW_COMMAND = 'confirm_hard_exclusion_preview';
export const DISCARD_HARD_EXCLUSION_PREVIEW_COMMAND = 'discard_hard_exclusion_preview';
export const PREVIEW_SCOPE_ROOT_REVOCATION_COMMAND = 'preview_scope_root_revocation';
export const CONFIRM_SCOPE_ROOT_REVOCATION_COMMAND = 'confirm_scope_root_revocation';
export const DISCARD_SCOPE_ROOT_REVOCATION_COMMAND = 'discard_scope_root_revocation';

export interface ManifestStats {
  api_version: 'deskgraph.manifest.v1';
  database_ready: true;
  authorized_scope_count: number;
  node_count: number;
  file_count: number;
  folder_count: number;
  active_location_count: number;
  issue_count: number;
  completed_scan_count: number;
}

export interface AuthorizedScope {
  id: number;
  display_path: string;
  created_at_unix_ms: number;
}

export type ScanStatus = 'running' | 'paused' | 'completed' | 'failed' | 'interrupted';

export interface ScanJobProgress {
  api_version: 'deskgraph.scan-job.v1';
  job_id: number;
  scope_id: number;
  status: ScanStatus;
  queued_entries: number;
  processed_entries: number;
  discovered_files: number;
  discovered_folders: number;
  skipped_entries: number;
  issue_count: number;
  elapsed_ms: number;
  pause_requested: boolean;
}

export type HardExclusionEntryKind = 'file' | 'folder';
export interface HardExclusionItem {
  display_path: string;
  entry_kind: HardExclusionEntryKind;
  disposition: 'will_add' | 'already_excluded' | 'covered_by_selected_parent';
}
export interface HardExclusionImpact {
  location_count: number;
  content_chunk_count: number;
  graph_fact_count: number;
  derived_candidate_count: number;
  action_plan_count: number;
  cleanup_action_plan_count: number;
  pending_job_count: number;
  blocking_action_count: number;
}
export interface CoveragePolicyDetail {
  api_version: 'deskgraph.coverage-policy.v1';
  scope_id: number;
  root_display_path: string;
  policy_revision: number;
  exclusions: readonly {
    id: number;
    scope_id: number;
    display_path: string;
    entry_kind: HardExclusionEntryKind;
    created_at_unix_ms: number;
  }[];
}
export interface HardExclusionPreview {
  api_version: 'deskgraph.hard-exclusion-preview.v1';
  preview_id: string;
  scope_id: number;
  base_policy_revision: number;
  expires_at_unix_ms: number;
  items: readonly HardExclusionItem[];
  impact: HardExclusionImpact;
  confirmable: boolean;
  source_files_will_change: false;
}
export interface HardExclusionCommit {
  api_version: 'deskgraph.hard-exclusion-commit.v1';
  scope_id: number;
  policy_revision: number;
  exclusions: number;
  purge: HardExclusionImpact;
  source_files_changed: false;
  automatic_scans_started: 0;
  automatic_extractions_started: 0;
}
export interface ScopeRootRevocationPreview {
  api_version: 'deskgraph.scope-root-revocation-preview.v1';
  preview_id: string;
  scope_id: number;
  base_policy_revision: number;
  expires_at_unix_ms: number;
  impact: HardExclusionImpact;
  exclusion_count: number;
  confirmable: boolean;
  source_files_will_change: false;
}
export interface ScopeRootRevocationCommit {
  api_version: 'deskgraph.scope-root-revocation-commit.v1';
  scope_id: number;
  policy_revision: number;
  purged: HardExclusionImpact;
  exclusions_removed: number;
  runtime_capability_dropped: true;
  native_watch_sync_confirmed: boolean;
  native_watch_callback_retired: boolean;
  watch_runtime_stopped: boolean;
  source_files_changed: false;
  revoked_scope_scans_started: 0;
  revoked_scope_extractions_started: 0;
}

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isPositiveId(value: unknown): value is number {
  return isCount(value) && value > 0;
}

function isPositiveRevision(value: unknown): value is number {
  return isPositiveId(value);
}

function isDisplayPath(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function isEntryKind(value: unknown): value is HardExclusionEntryKind {
  return value === 'file' || value === 'folder';
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  return (
    actual.length === keys.length && actual.every((key, index) => key === [...keys].sort()[index])
  );
}

function parseHardExclusionImpact(value: unknown): HardExclusionImpact {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      'location_count',
      'content_chunk_count',
      'graph_fact_count',
      'derived_candidate_count',
      'action_plan_count',
      'cleanup_action_plan_count',
      'pending_job_count',
      'blocking_action_count',
    ]) ||
    !Object.values(value).every(isCount)
  )
    throw new Error('Invalid hard exclusion impact response');
  return value as unknown as HardExclusionImpact;
}

function parseHardExclusionItem(value: unknown): HardExclusionItem {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ['display_path', 'entry_kind', 'disposition']) ||
    !isDisplayPath(value.display_path) ||
    !isEntryKind(value.entry_kind) ||
    (value.disposition !== 'will_add' &&
      value.disposition !== 'already_excluded' &&
      value.disposition !== 'covered_by_selected_parent')
  )
    throw new Error('Invalid hard exclusion item response');
  return value as unknown as HardExclusionItem;
}

export function parseCoveragePolicyDetail(value: unknown): CoveragePolicyDetail {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      'api_version',
      'scope_id',
      'root_display_path',
      'policy_revision',
      'exclusions',
    ]) ||
    value.api_version !== 'deskgraph.coverage-policy.v1' ||
    !isPositiveId(value.scope_id) ||
    !isDisplayPath(value.root_display_path) ||
    !isPositiveRevision(value.policy_revision) ||
    !Array.isArray(value.exclusions)
  ) {
    throw new Error('Invalid coverage policy detail response');
  }
  for (const exclusion of value.exclusions) {
    if (
      !isRecord(exclusion) ||
      !hasExactKeys(exclusion, [
        'id',
        'scope_id',
        'display_path',
        'entry_kind',
        'created_at_unix_ms',
      ]) ||
      !isPositiveId(exclusion.id) ||
      exclusion.scope_id !== value.scope_id ||
      !isDisplayPath(exclusion.display_path) ||
      !isEntryKind(exclusion.entry_kind) ||
      !isCount(exclusion.created_at_unix_ms)
    ) {
      throw new Error('Invalid coverage policy detail response');
    }
  }
  return value as unknown as CoveragePolicyDetail;
}

export function parseHardExclusionPreview(value: unknown): HardExclusionPreview | null {
  if (value === null) return null;
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      'api_version',
      'preview_id',
      'scope_id',
      'base_policy_revision',
      'expires_at_unix_ms',
      'items',
      'impact',
      'confirmable',
      'source_files_will_change',
    ]) ||
    value.api_version !== 'deskgraph.hard-exclusion-preview.v1' ||
    typeof value.preview_id !== 'string' ||
    value.preview_id.trim().length === 0 ||
    value.preview_id.length > 128 ||
    !isPositiveId(value.scope_id) ||
    !isPositiveRevision(value.base_policy_revision) ||
    !isCount(value.expires_at_unix_ms) ||
    !Array.isArray(value.items) ||
    typeof value.confirmable !== 'boolean' ||
    value.source_files_will_change !== false
  )
    throw new Error('Invalid hard exclusion preview response');
  value.items.forEach(parseHardExclusionItem);
  if (value.confirmable && value.items.length === 0)
    throw new Error('Invalid hard exclusion preview response');
  parseHardExclusionImpact(value.impact);
  return value as unknown as HardExclusionPreview;
}

export function parseHardExclusionCommit(value: unknown): HardExclusionCommit {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      'api_version',
      'scope_id',
      'policy_revision',
      'exclusions',
      'purge',
      'source_files_changed',
      'automatic_scans_started',
      'automatic_extractions_started',
    ]) ||
    value.api_version !== 'deskgraph.hard-exclusion-commit.v1' ||
    !isPositiveId(value.scope_id) ||
    !isPositiveRevision(value.policy_revision) ||
    !isPositiveId(value.exclusions) ||
    value.source_files_changed !== false ||
    value.automatic_scans_started !== 0 ||
    value.automatic_extractions_started !== 0
  ) {
    throw new Error('Invalid hard exclusion commit response');
  }
  parseHardExclusionImpact(value.purge);
  return value as unknown as HardExclusionCommit;
}

export function parseScopeRootRevocationPreview(value: unknown): ScopeRootRevocationPreview {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      'api_version',
      'preview_id',
      'scope_id',
      'base_policy_revision',
      'expires_at_unix_ms',
      'impact',
      'exclusion_count',
      'confirmable',
      'source_files_will_change',
    ]) ||
    value.api_version !== 'deskgraph.scope-root-revocation-preview.v1' ||
    typeof value.preview_id !== 'string' ||
    value.preview_id.trim().length === 0 ||
    value.preview_id.length > 128 ||
    !isPositiveId(value.scope_id) ||
    !isPositiveRevision(value.base_policy_revision) ||
    !isCount(value.expires_at_unix_ms) ||
    !isCount(value.exclusion_count) ||
    typeof value.confirmable !== 'boolean' ||
    value.source_files_will_change !== false
  ) {
    throw new Error('Invalid scope root revocation preview response');
  }
  const impact = parseHardExclusionImpact(value.impact);
  if (value.confirmable !== (impact.blocking_action_count === 0)) {
    throw new Error('Invalid scope root revocation preview response');
  }
  return value as unknown as ScopeRootRevocationPreview;
}

export function parseScopeRootRevocationCommit(value: unknown): ScopeRootRevocationCommit {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      'api_version',
      'scope_id',
      'policy_revision',
      'purged',
      'exclusions_removed',
      'runtime_capability_dropped',
      'native_watch_sync_confirmed',
      'native_watch_callback_retired',
      'watch_runtime_stopped',
      'source_files_changed',
      'revoked_scope_scans_started',
      'revoked_scope_extractions_started',
    ]) ||
    value.api_version !== 'deskgraph.scope-root-revocation-commit.v1' ||
    !isPositiveId(value.scope_id) ||
    !isPositiveRevision(value.policy_revision) ||
    !isCount(value.exclusions_removed) ||
    value.runtime_capability_dropped !== true ||
    typeof value.native_watch_sync_confirmed !== 'boolean' ||
    typeof value.native_watch_callback_retired !== 'boolean' ||
    typeof value.watch_runtime_stopped !== 'boolean' ||
    (value.native_watch_sync_confirmed &&
      (value.native_watch_callback_retired || value.watch_runtime_stopped)) ||
    (value.watch_runtime_stopped && !value.native_watch_callback_retired) ||
    value.source_files_changed !== false ||
    value.revoked_scope_scans_started !== 0 ||
    value.revoked_scope_extractions_started !== 0
  ) {
    throw new Error('Invalid scope root revocation commit response');
  }
  const purged = parseHardExclusionImpact(value.purged);
  if (purged.blocking_action_count !== 0) {
    throw new Error('Invalid scope root revocation commit response');
  }
  return value as unknown as ScopeRootRevocationCommit;
}

export function parseManifestStats(value: unknown): ManifestStats {
  if (!isRecord(value)) {
    throw new Error('Invalid manifest status response');
  }
  const valid =
    value.api_version === 'deskgraph.manifest.v1' &&
    value.database_ready === true &&
    isCount(value.authorized_scope_count) &&
    isCount(value.node_count) &&
    isCount(value.file_count) &&
    isCount(value.folder_count) &&
    isCount(value.active_location_count) &&
    isCount(value.issue_count) &&
    isCount(value.completed_scan_count);
  if (!valid) {
    throw new Error('Invalid manifest status response');
  }
  return value as unknown as ManifestStats;
}

export function parseAuthorizedScope(value: unknown): AuthorizedScope {
  if (
    !isRecord(value) ||
    Object.keys(value).length !== 3 ||
    !isCount(value.id) ||
    value.id === 0 ||
    typeof value.display_path !== 'string' ||
    value.display_path.trim().length === 0 ||
    !isCount(value.created_at_unix_ms)
  ) {
    throw new Error('Invalid authorized scope response');
  }
  return value as unknown as AuthorizedScope;
}

export function parseAuthorizedScopes(value: unknown): AuthorizedScope[] {
  if (!Array.isArray(value)) {
    throw new Error('Invalid authorized scopes response');
  }
  return value.map(parseAuthorizedScope);
}

export function mergeAuthorizedScopes(
  current: AuthorizedScope[],
  authorized: AuthorizedScope[],
): AuthorizedScope[] {
  const merged = new Map(current.map((scope) => [scope.id, scope]));
  for (const scope of authorized) merged.set(scope.id, scope);
  return [...merged.values()].sort((left, right) => left.id - right.id);
}

/**
 * The native picker command is intentionally parameterless. A cancelled picker
 * is a normal outcome, while every non-null response must still satisfy the
 * same durable scope contract as data loaded from the backend.
 */
export function parseSelectedAuthorizedScopes(value: unknown): AuthorizedScope[] | null {
  if (value === null) return null;
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error('Invalid authorized coverage response');
  }
  const scopes = value.map(parseAuthorizedScope);
  if (new Set(scopes.map((scope) => scope.id)).size !== scopes.length) {
    throw new Error('Invalid authorized coverage response');
  }
  return scopes;
}

export function parseScanJobProgress(value: unknown): ScanJobProgress {
  if (!isRecord(value)) {
    throw new Error('Invalid scan response');
  }
  const validStatus =
    value.status === 'running' ||
    value.status === 'paused' ||
    value.status === 'completed' ||
    value.status === 'failed' ||
    value.status === 'interrupted';
  const valid =
    value.api_version === 'deskgraph.scan-job.v1' &&
    validStatus &&
    isCount(value.job_id) &&
    isCount(value.scope_id) &&
    isCount(value.queued_entries) &&
    isCount(value.processed_entries) &&
    isCount(value.discovered_files) &&
    isCount(value.discovered_folders) &&
    isCount(value.skipped_entries) &&
    isCount(value.issue_count) &&
    isCount(value.elapsed_ms) &&
    typeof value.pause_requested === 'boolean';
  if (!valid) {
    throw new Error('Invalid scan response');
  }
  return value as unknown as ScanJobProgress;
}

export function parseScanJobs(value: unknown): ScanJobProgress[] {
  if (!Array.isArray(value)) {
    throw new Error('Invalid scan list response');
  }
  return value.map(parseScanJobProgress);
}

export async function loadManifestStatus(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ManifestStats> {
  return parseManifestStats(await invokeCommand(MANIFEST_STATUS_COMMAND));
}

export async function loadAuthorizedScopes(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<AuthorizedScope[]> {
  return parseAuthorizedScopes(await invokeCommand(AUTHORIZED_SCOPES_COMMAND));
}

export async function selectAndAuthorizeScopes(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<AuthorizedScope[] | null> {
  return parseSelectedAuthorizedScopes(await invokeCommand(SELECT_AND_AUTHORIZE_SCOPES_COMMAND));
}

export async function createManifestScan(
  scopeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ScanJobProgress> {
  return parseScanJobProgress(await invokeCommand(CREATE_SCAN_COMMAND, { scopeId }));
}

export async function runManifestScan(
  jobId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ScanJobProgress> {
  return parseScanJobProgress(await invokeCommand(RUN_SCAN_COMMAND, { jobId }));
}

export async function loadScanJobStatus(
  jobId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ScanJobProgress> {
  return parseScanJobProgress(await invokeCommand(SCAN_JOB_STATUS_COMMAND, { jobId }));
}

export async function loadRecentScanJobs(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ScanJobProgress[]> {
  return parseScanJobs(await invokeCommand(RECENT_SCAN_JOBS_COMMAND));
}

export async function pauseManifestScan(
  jobId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ScanJobProgress> {
  return parseScanJobProgress(await invokeCommand(PAUSE_SCAN_COMMAND, { jobId }));
}

export async function resumeManifestScan(
  jobId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ScanJobProgress> {
  return parseScanJobProgress(await invokeCommand(RESUME_SCAN_COMMAND, { jobId }));
}

export async function loadCoveragePolicyDetail(
  scopeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<CoveragePolicyDetail> {
  return parseCoveragePolicyDetail(
    await invokeCommand(COVERAGE_POLICY_DETAIL_COMMAND, { scopeId }),
  );
}

/** Native selection only: paths are never sent from the WebView. */
export async function selectHardExclusionsPreview(
  scopeId: number,
  entryKind: HardExclusionEntryKind,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<HardExclusionPreview | null> {
  return parseHardExclusionPreview(
    await invokeCommand(SELECT_HARD_EXCLUSIONS_PREVIEW_COMMAND, { scopeId, entryKind }),
  );
}

export async function confirmHardExclusionPreview(
  previewId: string,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<HardExclusionCommit> {
  return parseHardExclusionCommit(
    await invokeCommand(CONFIRM_HARD_EXCLUSION_PREVIEW_COMMAND, { previewId }),
  );
}

export async function discardHardExclusionPreview(
  previewId: string,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<void> {
  await invokeCommand(DISCARD_HARD_EXCLUSION_PREVIEW_COMMAND, { previewId });
}

export async function previewScopeRootRevocation(
  scopeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ScopeRootRevocationPreview> {
  return parseScopeRootRevocationPreview(
    await invokeCommand(PREVIEW_SCOPE_ROOT_REVOCATION_COMMAND, { scopeId }),
  );
}

export async function confirmScopeRootRevocation(
  previewId: string,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ScopeRootRevocationCommit> {
  return parseScopeRootRevocationCommit(
    await invokeCommand(CONFIRM_SCOPE_ROOT_REVOCATION_COMMAND, { previewId }),
  );
}

export async function discardScopeRootRevocation(
  previewId: string,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<void> {
  await invokeCommand(DISCARD_SCOPE_ROOT_REVOCATION_COMMAND, { previewId });
}
