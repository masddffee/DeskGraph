import { invoke } from '@tauri-apps/api/core';

export const MANIFEST_STATUS_COMMAND = 'manifest_status';
export const AUTHORIZED_SCOPES_COMMAND = 'authorized_scopes';
export const AUTHORIZE_SCOPE_COMMAND = 'authorize_scope_path';
export const RUN_SCAN_COMMAND = 'run_manifest_scan';

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

export interface ScanReport {
  api_version: 'deskgraph.scan.v1';
  job_id: number;
  scope_id: number;
  status: 'completed';
  discovered_files: number;
  discovered_folders: number;
  skipped_entries: number;
  issue_count: number;
  elapsed_ms: number;
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

export function parseScanReport(value: unknown): ScanReport {
  if (!isRecord(value)) {
    throw new Error('Invalid scan response');
  }
  const valid =
    value.api_version === 'deskgraph.scan.v1' &&
    value.status === 'completed' &&
    isCount(value.job_id) &&
    isCount(value.scope_id) &&
    isCount(value.discovered_files) &&
    isCount(value.discovered_folders) &&
    isCount(value.skipped_entries) &&
    isCount(value.issue_count) &&
    isCount(value.elapsed_ms);
  if (!valid) {
    throw new Error('Invalid scan response');
  }
  return value as unknown as ScanReport;
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

export async function runManifestScan(
  scopeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ScanReport> {
  return parseScanReport(await invokeCommand(RUN_SCAN_COMMAND, { scopeId }));
}
