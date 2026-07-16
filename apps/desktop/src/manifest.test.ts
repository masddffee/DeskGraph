import { describe, expect, it, vi } from 'vitest';

import {
  AUTHORIZE_SCOPE_COMMAND,
  MANIFEST_STATUS_COMMAND,
  RUN_SCAN_COMMAND,
  addAuthorizedScope,
  loadManifestStatus,
  parseManifestStats,
  parseScanReport,
  runManifestScan,
  type ManifestStats,
  type ScanReport,
} from './manifest';

const stats: ManifestStats = {
  api_version: 'deskgraph.manifest.v1',
  database_ready: true,
  authorized_scope_count: 1,
  node_count: 3,
  file_count: 2,
  folder_count: 1,
  active_location_count: 3,
  issue_count: 0,
  completed_scan_count: 1,
};

const report: ScanReport = {
  api_version: 'deskgraph.scan.v1',
  job_id: 1,
  scope_id: 4,
  status: 'completed',
  discovered_files: 2,
  discovered_folders: 1,
  skipped_entries: 0,
  issue_count: 0,
  elapsed_ms: 8,
};

describe('manifest contract', () => {
  it('validates manifest statistics and rejects negative counts', () => {
    expect(parseManifestStats(stats)).toEqual(stats);
    expect(() => parseManifestStats({ ...stats, node_count: -1 })).toThrow(
      'Invalid manifest status response',
    );
  });

  it('validates completed scan reports', () => {
    expect(parseScanReport(report)).toEqual(report);
    expect(() => parseScanReport({ ...report, status: 'running' })).toThrow(
      'Invalid scan response',
    );
  });

  it('uses narrow commands and explicit arguments', async () => {
    const invokeCommand = vi.fn().mockImplementation((command: string) => {
      if (command === MANIFEST_STATUS_COMMAND) return Promise.resolve(stats);
      if (command === AUTHORIZE_SCOPE_COMMAND) {
        return Promise.resolve({ id: 4, display_path: '/explicit', created_at_unix_ms: 1 });
      }
      return Promise.resolve(report);
    });

    await expect(loadManifestStatus(invokeCommand)).resolves.toEqual(stats);
    await expect(addAuthorizedScope('/explicit', invokeCommand)).resolves.toMatchObject({ id: 4 });
    await expect(runManifestScan(4, invokeCommand)).resolves.toEqual(report);
    expect(invokeCommand).toHaveBeenNthCalledWith(1, MANIFEST_STATUS_COMMAND);
    expect(invokeCommand).toHaveBeenNthCalledWith(2, AUTHORIZE_SCOPE_COMMAND, {
      path: '/explicit',
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(3, RUN_SCAN_COMMAND, { scopeId: 4 });
  });
});
