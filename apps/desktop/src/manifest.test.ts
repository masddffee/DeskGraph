import { describe, expect, it, vi } from 'vitest';

import {
  AUTHORIZE_SCOPE_COMMAND,
  CREATE_SCAN_COMMAND,
  MANIFEST_STATUS_COMMAND,
  PAUSE_SCAN_COMMAND,
  RECENT_SCAN_JOBS_COMMAND,
  RESUME_SCAN_COMMAND,
  RUN_SCAN_COMMAND,
  SCAN_JOB_STATUS_COMMAND,
  addAuthorizedScope,
  createManifestScan,
  loadRecentScanJobs,
  loadScanJobStatus,
  loadManifestStatus,
  parseManifestStats,
  parseScanJobProgress,
  parseScanJobs,
  pauseManifestScan,
  resumeManifestScan,
  runManifestScan,
  type ManifestStats,
  type ScanJobProgress,
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

const progress: ScanJobProgress = {
  api_version: 'deskgraph.scan-job.v1',
  job_id: 1,
  scope_id: 4,
  status: 'running',
  queued_entries: 8,
  processed_entries: 3,
  discovered_files: 2,
  discovered_folders: 1,
  skipped_entries: 0,
  issue_count: 0,
  elapsed_ms: 8,
  pause_requested: false,
};

describe('manifest contract', () => {
  it('validates manifest statistics and rejects negative counts', () => {
    expect(parseManifestStats(stats)).toEqual(stats);
    expect(() => parseManifestStats({ ...stats, node_count: -1 })).toThrow(
      'Invalid manifest status response',
    );
  });

  it('validates every durable scan state and progress count', () => {
    for (const status of ['running', 'paused', 'completed', 'failed', 'interrupted'] as const) {
      expect(parseScanJobProgress({ ...progress, status }).status).toBe(status);
    }
    expect(parseScanJobs([progress])).toEqual([progress]);
    expect(() => parseScanJobProgress({ ...progress, processed_entries: -1 })).toThrow(
      'Invalid scan response',
    );
    expect(() => parseScanJobs({})).toThrow('Invalid scan list response');
  });

  it('uses narrow commands and explicit arguments', async () => {
    const invokeCommand = vi.fn().mockImplementation((command: string) => {
      if (command === MANIFEST_STATUS_COMMAND) return Promise.resolve(stats);
      if (command === AUTHORIZE_SCOPE_COMMAND) {
        return Promise.resolve({ id: 4, display_path: '/explicit', created_at_unix_ms: 1 });
      }
      if (command === RECENT_SCAN_JOBS_COMMAND) return Promise.resolve([progress]);
      return Promise.resolve(progress);
    });

    await expect(loadManifestStatus(invokeCommand)).resolves.toEqual(stats);
    await expect(addAuthorizedScope('/explicit', invokeCommand)).resolves.toMatchObject({ id: 4 });
    await expect(createManifestScan(4, invokeCommand)).resolves.toEqual(progress);
    await expect(runManifestScan(1, invokeCommand)).resolves.toEqual(progress);
    await expect(loadScanJobStatus(1, invokeCommand)).resolves.toEqual(progress);
    await expect(loadRecentScanJobs(invokeCommand)).resolves.toEqual([progress]);
    await expect(pauseManifestScan(1, invokeCommand)).resolves.toEqual(progress);
    await expect(resumeManifestScan(1, invokeCommand)).resolves.toEqual(progress);
    expect(invokeCommand).toHaveBeenNthCalledWith(1, MANIFEST_STATUS_COMMAND);
    expect(invokeCommand).toHaveBeenNthCalledWith(2, AUTHORIZE_SCOPE_COMMAND, {
      path: '/explicit',
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(3, CREATE_SCAN_COMMAND, { scopeId: 4 });
    expect(invokeCommand).toHaveBeenNthCalledWith(4, RUN_SCAN_COMMAND, { jobId: 1 });
    expect(invokeCommand).toHaveBeenNthCalledWith(5, SCAN_JOB_STATUS_COMMAND, { jobId: 1 });
    expect(invokeCommand).toHaveBeenNthCalledWith(6, RECENT_SCAN_JOBS_COMMAND);
    expect(invokeCommand).toHaveBeenNthCalledWith(7, PAUSE_SCAN_COMMAND, { jobId: 1 });
    expect(invokeCommand).toHaveBeenNthCalledWith(8, RESUME_SCAN_COMMAND, { jobId: 1 });
  });
});
