import { invoke } from '@tauri-apps/api/core';

export const MANIFEST_STATUS_COMMAND = 'manifest_status';
export const AUTHORIZED_SCOPES_COMMAND = 'authorized_scopes';
export const AUTHORIZE_SCOPE_COMMAND = 'authorize_scope_path';
export const CREATE_SCAN_COMMAND = 'create_manifest_scan';
export const RUN_SCAN_COMMAND = 'run_manifest_scan';
export const SCAN_JOB_STATUS_COMMAND = 'scan_job_status';
export const RECENT_SCAN_JOBS_COMMAND = 'recent_scan_jobs';
export const PAUSE_SCAN_COMMAND = 'pause_manifest_scan';
export const RESUME_SCAN_COMMAND = 'resume_manifest_scan';

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

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
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
    !isCount(value.id) ||
    typeof value.display_path !== 'string' ||
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

export async function addAuthorizedScope(
  path: string,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<AuthorizedScope> {
  return parseAuthorizedScope(await invokeCommand(AUTHORIZE_SCOPE_COMMAND, { path }));
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
