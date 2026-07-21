import { describe, expect, it, vi } from 'vitest';

import {
  CREATE_SCAN_COMMAND,
  MANIFEST_STATUS_COMMAND,
  PAUSE_SCAN_COMMAND,
  RECENT_SCAN_JOBS_COMMAND,
  RESUME_SCAN_COMMAND,
  RUN_SCAN_COMMAND,
  SCAN_JOB_STATUS_COMMAND,
  SELECT_AND_AUTHORIZE_SCOPES_COMMAND,
  CONFIRM_HARD_EXCLUSION_PREVIEW_COMMAND,
  CONFIRM_SCOPE_ROOT_REVOCATION_COMMAND,
  COVERAGE_POLICY_DETAIL_COMMAND,
  DISCARD_HARD_EXCLUSION_PREVIEW_COMMAND,
  DISCARD_SCOPE_ROOT_REVOCATION_COMMAND,
  PREVIEW_SCOPE_ROOT_REVOCATION_COMMAND,
  SELECT_HARD_EXCLUSIONS_PREVIEW_COMMAND,
  confirmHardExclusionPreview,
  confirmScopeRootRevocation,
  discardHardExclusionPreview,
  discardScopeRootRevocation,
  loadCoveragePolicyDetail,
  createManifestScan,
  loadRecentScanJobs,
  loadScanJobStatus,
  loadManifestStatus,
  mergeAuthorizedScopes,
  parseManifestStats,
  parseSelectedAuthorizedScopes,
  parseScanJobProgress,
  parseScanJobs,
  parseCoveragePolicyDetail,
  parseHardExclusionCommit,
  parseHardExclusionPreview,
  parseScopeRootRevocationCommit,
  parseScopeRootRevocationPreview,
  pauseManifestScan,
  resumeManifestScan,
  runManifestScan,
  selectAndAuthorizeScopes,
  selectHardExclusionsPreview,
  previewScopeRootRevocation,
  type ManifestStats,
  type ScanJobProgress,
} from './manifest';

const policyDetail = {
  api_version: 'deskgraph.coverage-policy.v1',
  scope_id: 4,
  root_display_path: '/explicit',
  policy_revision: 2,
  exclusions: [
    {
      id: 9,
      scope_id: 4,
      display_path: '/explicit/private',
      entry_kind: 'folder',
      created_at_unix_ms: 4,
    },
  ],
} as const;
const purge = {
  location_count: 1,
  content_chunk_count: 2,
  graph_fact_count: 3,
  derived_candidate_count: 4,
  action_plan_count: 5,
  cleanup_action_plan_count: 6,
  pending_job_count: 7,
  blocking_action_count: 8,
} as const;
const exclusionPreview = {
  api_version: 'deskgraph.hard-exclusion-preview.v1',
  preview_id: 'opaque-preview',
  scope_id: 4,
  base_policy_revision: 2,
  expires_at_unix_ms: 99,
  items: [{ display_path: '/explicit/private', entry_kind: 'folder', disposition: 'will_add' }],
  impact: purge,
  confirmable: true,
  source_files_will_change: false,
} as const;
const exclusionCommit = {
  api_version: 'deskgraph.hard-exclusion-commit.v1',
  scope_id: 4,
  policy_revision: 3,
  exclusions: 1,
  purge,
  source_files_changed: false,
  automatic_scans_started: 0,
  automatic_extractions_started: 0,
} as const;
const rootPurge = { ...purge, blocking_action_count: 0 } as const;
const rootRevocationPreview = {
  api_version: 'deskgraph.scope-root-revocation-preview.v1',
  preview_id: 'opaque-root-preview',
  scope_id: 4,
  base_policy_revision: 2,
  expires_at_unix_ms: 99,
  impact: rootPurge,
  exclusion_count: 0,
  confirmable: true,
  source_files_will_change: false,
} as const;
const rootRevocationCommit = {
  api_version: 'deskgraph.scope-root-revocation-commit.v1',
  scope_id: 4,
  policy_revision: 3,
  purged: rootPurge,
  exclusions_removed: 0,
  runtime_capability_dropped: true,
  native_watch_sync_confirmed: true,
  native_watch_callback_retired: false,
  watch_runtime_stopped: false,
  source_files_changed: false,
  revoked_scope_scans_started: 0,
  revoked_scope_extractions_started: 0,
} as const;

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
      if (command === SELECT_AND_AUTHORIZE_SCOPES_COMMAND) {
        return Promise.resolve([
          { id: 4, display_path: '/explicit/Desktop', created_at_unix_ms: 1 },
          { id: 5, display_path: '/explicit/Documents', created_at_unix_ms: 2 },
        ]);
      }
      if (command === RECENT_SCAN_JOBS_COMMAND) return Promise.resolve([progress]);
      return Promise.resolve(progress);
    });

    await expect(loadManifestStatus(invokeCommand)).resolves.toEqual(stats);
    await expect(selectAndAuthorizeScopes(invokeCommand)).resolves.toEqual([
      { id: 4, display_path: '/explicit/Desktop', created_at_unix_ms: 1 },
      { id: 5, display_path: '/explicit/Documents', created_at_unix_ms: 2 },
    ]);
    await expect(createManifestScan(4, invokeCommand)).resolves.toEqual(progress);
    await expect(runManifestScan(1, invokeCommand)).resolves.toEqual(progress);
    await expect(loadScanJobStatus(1, invokeCommand)).resolves.toEqual(progress);
    await expect(loadRecentScanJobs(invokeCommand)).resolves.toEqual([progress]);
    await expect(pauseManifestScan(1, invokeCommand)).resolves.toEqual(progress);
    await expect(resumeManifestScan(1, invokeCommand)).resolves.toEqual(progress);
    expect(invokeCommand).toHaveBeenNthCalledWith(1, MANIFEST_STATUS_COMMAND);
    expect(invokeCommand).toHaveBeenNthCalledWith(2, SELECT_AND_AUTHORIZE_SCOPES_COMMAND);
    expect(invokeCommand).toHaveBeenNthCalledWith(3, CREATE_SCAN_COMMAND, { scopeId: 4 });
    expect(invokeCommand).toHaveBeenNthCalledWith(4, RUN_SCAN_COMMAND, { jobId: 1 });
    expect(invokeCommand).toHaveBeenNthCalledWith(5, SCAN_JOB_STATUS_COMMAND, { jobId: 1 });
    expect(invokeCommand).toHaveBeenNthCalledWith(6, RECENT_SCAN_JOBS_COMMAND);
    expect(invokeCommand).toHaveBeenNthCalledWith(7, PAUSE_SCAN_COMMAND, { jobId: 1 });
    expect(invokeCommand).toHaveBeenNthCalledWith(8, RESUME_SCAN_COMMAND, { jobId: 1 });
  });

  it('treats picker cancellation as a normal no-op and rejects malformed picker results', async () => {
    expect(parseSelectedAuthorizedScopes(null)).toBeNull();
    await expect(selectAndAuthorizeScopes(vi.fn().mockResolvedValue(null))).resolves.toBeNull();
    expect(() => parseSelectedAuthorizedScopes([])).toThrow('Invalid authorized coverage response');
    expect(() =>
      parseSelectedAuthorizedScopes({
        id: 4,
        display_path: '/legacy-single-object',
        created_at_unix_ms: 1,
      }),
    ).toThrow('Invalid authorized coverage response');
    expect(() =>
      parseSelectedAuthorizedScopes([
        { id: 4, display_path: '/explicit', created_at_unix_ms: 1 },
        { id: 5, display_path: '', created_at_unix_ms: 2 },
      ]),
    ).toThrow('Invalid authorized scope response');
    expect(() =>
      parseSelectedAuthorizedScopes([
        { id: 4, display_path: '/explicit', created_at_unix_ms: 1 },
        { id: 4, display_path: '/duplicate-id', created_at_unix_ms: 2 },
      ]),
    ).toThrow('Invalid authorized coverage response');
    expect(() =>
      parseSelectedAuthorizedScopes([
        {
          id: 4,
          display_path: '/explicit',
          created_at_unix_ms: 1,
          opaque_grant: 'must-not-cross-ipc',
        },
      ]),
    ).toThrow('Invalid authorized scope response');
  });

  it('merges a committed coverage response without losing existing local scopes', () => {
    expect(
      mergeAuthorizedScopes(
        [
          { id: 1, display_path: '/existing', created_at_unix_ms: 1 },
          { id: 4, display_path: '/stale-path', created_at_unix_ms: 1 },
        ],
        [
          { id: 4, display_path: '/reauthorized', created_at_unix_ms: 1 },
          { id: 2, display_path: '/new', created_at_unix_ms: 2 },
        ],
      ),
    ).toEqual([
      { id: 1, display_path: '/existing', created_at_unix_ms: 1 },
      { id: 2, display_path: '/new', created_at_unix_ms: 2 },
      { id: 4, display_path: '/reauthorized', created_at_unix_ms: 1 },
    ]);
  });

  it('accepts only exact hard-exclusion policy, preview, and commit payloads', () => {
    expect(parseCoveragePolicyDetail(policyDetail)).toEqual(policyDetail);
    expect(parseHardExclusionPreview(null)).toBeNull();
    expect(parseHardExclusionPreview(exclusionPreview)).toEqual(exclusionPreview);
    expect(parseHardExclusionCommit(exclusionCommit)).toEqual(exclusionCommit);
    expect(() =>
      parseCoveragePolicyDetail({ ...policyDetail, path: '/must-not-arrive' }),
    ).toThrow();
    expect(() =>
      parseHardExclusionPreview({ ...exclusionPreview, source_files_will_change: true }),
    ).toThrow();
    expect(() =>
      parseHardExclusionPreview({
        ...exclusionPreview,
        items: [{ ...exclusionPreview.items[0], disposition: 'unknown' }],
      }),
    ).toThrow();
    expect(() => parseCoveragePolicyDetail({ ...policyDetail, policy_revision: 0 })).toThrow();
    expect(() =>
      parseHardExclusionPreview({ ...exclusionPreview, base_policy_revision: 0 }),
    ).toThrow();
    expect(() =>
      parseHardExclusionPreview({ ...exclusionPreview, preview_id: ' '.repeat(129) }),
    ).toThrow();
    expect(() =>
      parseHardExclusionPreview({ ...exclusionPreview, items: [], confirmable: true }),
    ).toThrow();
    expect(() =>
      parseHardExclusionCommit({ ...exclusionCommit, automatic_scans_started: 1 }),
    ).toThrow();
    expect(() => parseHardExclusionCommit({ ...exclusionCommit, policy_revision: 0 })).toThrow();
    expect(() => parseHardExclusionCommit({ ...exclusionCommit, exclusions: 0 })).toThrow();
  });

  it('uses native selection and opaque preview IDs without forwarding a WebView path', async () => {
    const invokeCommand = vi.fn().mockImplementation((command: string) => {
      if (command === COVERAGE_POLICY_DETAIL_COMMAND) return Promise.resolve(policyDetail);
      if (command === SELECT_HARD_EXCLUSIONS_PREVIEW_COMMAND)
        return Promise.resolve(exclusionPreview);
      if (command === CONFIRM_HARD_EXCLUSION_PREVIEW_COMMAND)
        return Promise.resolve(exclusionCommit);
      return Promise.resolve(undefined);
    });
    await expect(loadCoveragePolicyDetail(4, invokeCommand)).resolves.toEqual(policyDetail);
    await expect(selectHardExclusionsPreview(4, 'folder', invokeCommand)).resolves.toEqual(
      exclusionPreview,
    );
    await expect(confirmHardExclusionPreview('opaque-preview', invokeCommand)).resolves.toEqual(
      exclusionCommit,
    );
    await discardHardExclusionPreview('opaque-preview', invokeCommand);
    expect(invokeCommand).toHaveBeenNthCalledWith(1, COVERAGE_POLICY_DETAIL_COMMAND, {
      scopeId: 4,
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(2, SELECT_HARD_EXCLUSIONS_PREVIEW_COMMAND, {
      scopeId: 4,
      entryKind: 'folder',
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(3, CONFIRM_HARD_EXCLUSION_PREVIEW_COMMAND, {
      previewId: 'opaque-preview',
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(4, DISCARD_HARD_EXCLUSION_PREVIEW_COMMAND, {
      previewId: 'opaque-preview',
    });
  });

  it('accepts exact root revocation preview and commit contracts with zero-or-positive purge counts', () => {
    expect(parseScopeRootRevocationPreview(rootRevocationPreview)).toEqual(rootRevocationPreview);
    expect(parseScopeRootRevocationCommit(rootRevocationCommit)).toEqual(rootRevocationCommit);
    expect(
      parseScopeRootRevocationCommit({
        ...rootRevocationCommit,
        native_watch_sync_confirmed: false,
      }),
    ).toEqual({
      ...rootRevocationCommit,
      native_watch_sync_confirmed: false,
    });
    expect(
      parseScopeRootRevocationCommit({
        ...rootRevocationCommit,
        native_watch_sync_confirmed: false,
        native_watch_callback_retired: true,
        watch_runtime_stopped: true,
      }),
    ).toEqual({
      ...rootRevocationCommit,
      native_watch_sync_confirmed: false,
      native_watch_callback_retired: true,
      watch_runtime_stopped: true,
    });
    expect(() =>
      parseScopeRootRevocationPreview({ ...rootRevocationPreview, scope_id: 0 }),
    ).toThrow('Invalid scope root revocation preview response');
    expect(() =>
      parseScopeRootRevocationPreview({ ...rootRevocationPreview, exclusion_count: -1 }),
    ).toThrow('Invalid scope root revocation preview response');
    expect(() =>
      parseScopeRootRevocationPreview({
        ...rootRevocationPreview,
        impact: { ...rootPurge, action_plan_count: -1 },
      }),
    ).toThrow('Invalid hard exclusion impact response');
    expect(() =>
      parseScopeRootRevocationPreview({
        ...rootRevocationPreview,
        impact: { ...rootPurge, cleanup_action_plan_count: undefined },
      }),
    ).toThrow('Invalid hard exclusion impact response');
    expect(() =>
      parseScopeRootRevocationPreview({
        ...rootRevocationPreview,
        confirmable: true,
        impact: { ...rootPurge, blocking_action_count: 1 },
      }),
    ).toThrow('Invalid scope root revocation preview response');
    expect(() =>
      parseScopeRootRevocationPreview({ ...rootRevocationPreview, confirmable: false }),
    ).toThrow('Invalid scope root revocation preview response');
    expect(() =>
      parseScopeRootRevocationPreview({
        ...rootRevocationPreview,
        source_path: '/must-not-arrive',
      }),
    ).toThrow('Invalid scope root revocation preview response');
    expect(() =>
      parseScopeRootRevocationCommit({ ...rootRevocationCommit, exclusions_removed: -1 }),
    ).toThrow('Invalid scope root revocation commit response');
    expect(() =>
      parseScopeRootRevocationCommit({
        ...rootRevocationCommit,
        runtime_capability_dropped: false,
      }),
    ).toThrow('Invalid scope root revocation commit response');
    expect(() =>
      parseScopeRootRevocationCommit({
        ...rootRevocationCommit,
        native_watch_sync_confirmed: undefined,
      }),
    ).toThrow('Invalid scope root revocation commit response');
    expect(() =>
      parseScopeRootRevocationCommit({
        ...rootRevocationCommit,
        native_watch_callback_retired: true,
      }),
    ).toThrow('Invalid scope root revocation commit response');
    expect(() =>
      parseScopeRootRevocationCommit({
        ...rootRevocationCommit,
        native_watch_sync_confirmed: false,
        watch_runtime_stopped: true,
      }),
    ).toThrow('Invalid scope root revocation commit response');
    expect(() =>
      parseScopeRootRevocationCommit({ ...rootRevocationCommit, revoked_scope_scans_started: 1 }),
    ).toThrow('Invalid scope root revocation commit response');
    expect(() =>
      parseScopeRootRevocationCommit({
        ...rootRevocationCommit,
        purged: { ...rootPurge, blocking_action_count: 1 },
      }),
    ).toThrow('Invalid scope root revocation commit response');
    expect(() =>
      parseScopeRootRevocationCommit({
        ...rootRevocationCommit,
        purged: { ...rootPurge, cleanup_action_plan_count: -1 },
      }),
    ).toThrow('Invalid hard exclusion impact response');
  });

  it('uses only an opaque preview ID after a scope-ID root revocation preview', async () => {
    const invokeCommand = vi.fn().mockImplementation((command: string) => {
      if (command === PREVIEW_SCOPE_ROOT_REVOCATION_COMMAND)
        return Promise.resolve(rootRevocationPreview);
      if (command === CONFIRM_SCOPE_ROOT_REVOCATION_COMMAND)
        return Promise.resolve(rootRevocationCommit);
      return Promise.resolve(undefined);
    });

    await expect(previewScopeRootRevocation(4, invokeCommand)).resolves.toEqual(
      rootRevocationPreview,
    );
    await expect(confirmScopeRootRevocation('opaque-root-preview', invokeCommand)).resolves.toEqual(
      rootRevocationCommit,
    );
    await discardScopeRootRevocation('opaque-root-preview', invokeCommand);

    expect(invokeCommand).toHaveBeenNthCalledWith(1, PREVIEW_SCOPE_ROOT_REVOCATION_COMMAND, {
      scopeId: 4,
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(2, CONFIRM_SCOPE_ROOT_REVOCATION_COMMAND, {
      previewId: 'opaque-root-preview',
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(3, DISCARD_SCOPE_ROOT_REVOCATION_COMMAND, {
      previewId: 'opaque-root-preview',
    });
    expect(JSON.stringify(invokeCommand.mock.calls)).not.toContain('/must-not-arrive');
  });
});
