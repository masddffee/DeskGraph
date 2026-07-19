import { describe, expect, it, vi } from 'vitest';

import {
  CREATE_CLEANUP_PREVIEW_COMMAND,
  GET_CLEANUP_SOURCE_DETAIL_COMMAND,
  REFRESH_CLEANUP_INBOX_COMMAND,
  createCleanupActionPreview,
  getCleanupSourceDetail,
  parseCleanupActionPlanPreview,
  parseCleanupSourceDetail,
  parseSmartCleanupInbox,
  parseSmartCleanupInboxItem,
  refreshSmartCleanupInbox,
  type CleanupActionPlanPreview,
  type CleanupSourceDetail,
  type SmartCleanupInbox,
} from './cleanup';

const inbox: SmartCleanupInbox = {
  api_version: 'deskgraph.smart-cleanup-inbox.v1',
  scope_id: 2,
  items: [
    {
      source_kind: 'exact_duplicate',
      source_id: 7,
      source_observation_id: 9,
      scope_id: 2,
      state: 'suggested',
      member_count: 2,
      confidence_basis_points: 10_000,
      observed_at_unix_ms: 1_700_000_000_000,
      current_evidence: true,
      verification_required: true,
      review_assistance_only: true,
      cleanup_authorized: false,
    },
  ],
  evaluated_source_count: 1,
  not_current_source_count: 0,
  bounded_source_limit: 20,
  evaluation_complete: true,
  action_authorized: false,
};

const detail: CleanupSourceDetail = {
  api_version: 'deskgraph.cleanup-source-detail.v1',
  scope_id: 2,
  source_kind: 'exact_duplicate',
  source_id: 7,
  source_observation_id: 10,
  members: [
    {
      node_id: 11,
      display_path: '/private/authorized/report.md',
      size_bytes: 12,
      role: 'duplicate_candidate',
    },
    {
      node_id: 12,
      display_path: '/private/authorized/report copy.md',
      size_bytes: 12,
      role: 'duplicate_candidate',
    },
  ],
  selection_rule: 'either_member_is_target',
  current_evidence: true,
  user_requested_paths: true,
  action_authorized: false,
  execution_available: false,
};

const preview: CleanupActionPlanPreview = {
  api_version: 'deskgraph.cleanup-action-plan-preview.v1',
  plan_id: 19,
  operation: 'system_trash_preview',
  state: 'previewed',
  scope_id: 2,
  source_kind: 'exact_duplicate',
  source_id: 7,
  source_observation_id: 10,
  keeper_node_id: 12,
  target_node_id: 11,
  expected_bytes: 12,
  keeper_hash_bound: true,
  policy: {
    api_version: 'deskgraph.cleanup-action-policy.v1',
    checks: [
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
    ],
    confirmation_required: true,
    action_authorized: false,
    execution_available: false,
  },
  journal_sequence: 1,
  created_at_unix_ms: 1_700_000_000_000,
};

describe('smart cleanup inbox IPC contract', () => {
  it('accepts only the narrow, path-free suggestion DTO', () => {
    expect(parseSmartCleanupInbox(inbox)).toEqual(inbox);
    expect(parseSmartCleanupInboxItem(inbox.items[0])).toEqual(inbox.items[0]);
    expect(() => parseSmartCleanupInbox({ ...inbox, display_path: '/private/file' })).toThrow(
      'Invalid smart cleanup inbox response',
    );
    expect(() => parseSmartCleanupInboxItem({ ...inbox.items[0], text: 'untrusted' })).toThrow(
      'Invalid smart cleanup inbox item response',
    );
  });

  it('rejects action authorization, stale evidence, and invalid bounded counts', () => {
    expect(() => parseSmartCleanupInbox({ ...inbox, action_authorized: true })).toThrow();
    expect(() =>
      parseSmartCleanupInboxItem({ ...inbox.items[0], current_evidence: false }),
    ).toThrow();
    expect(() => parseSmartCleanupInbox({ ...inbox, evaluated_source_count: 21 })).toThrow();
    expect(() =>
      parseSmartCleanupInboxItem({ ...inbox.items[0], confidence_basis_points: 9_000 }),
    ).toThrow();
    expect(() => parseSmartCleanupInbox({ ...inbox, bounded_source_limit: 64 })).toThrow();
    expect(() =>
      parseSmartCleanupInbox({ ...inbox, items: [{ ...inbox.items[0], scope_id: 3 }] }),
    ).toThrow();
  });

  it('preserves a bounded partial evaluation without claiming it is complete', () => {
    expect(parseSmartCleanupInbox({ ...inbox, evaluation_complete: false })).toMatchObject({
      evaluation_complete: false,
    });
  });

  it('uses an explicit opaque scope refresh and rejects invalid scope IDs locally', async () => {
    const invokeCommand = vi.fn().mockResolvedValue(inbox);
    await expect(refreshSmartCleanupInbox(2, invokeCommand)).resolves.toEqual(inbox);
    expect(invokeCommand).toHaveBeenCalledWith(REFRESH_CLEANUP_INBOX_COMMAND, { scopeId: 2 });
    await expect(refreshSmartCleanupInbox(0, invokeCommand)).rejects.toThrow(
      'Invalid smart cleanup inbox scope',
    );
    expect(invokeCommand).toHaveBeenCalledTimes(1);
  });
});

describe('explicit cleanup preview IPC contract', () => {
  it('accepts only an explicit transient path detail with a closed selection rule', () => {
    expect(parseCleanupSourceDetail(detail)).toEqual(detail);
    expect(() => parseCleanupSourceDetail({ ...detail, sha256: 'secret' })).toThrow(
      'Invalid cleanup source detail response',
    );
    expect(() => parseCleanupSourceDetail({ ...detail, action_authorized: true })).toThrow();
    expect(() =>
      parseCleanupSourceDetail({
        ...detail,
        members: [{ ...detail.members[0] }, { ...detail.members[0] }],
      }),
    ).toThrow();
    expect(() =>
      parseCleanupSourceDetail({
        ...detail,
        source_kind: 'version',
        selection_rule: 'either_member_is_target',
      }),
    ).toThrow();
  });

  it('accepts only the immutable path-free, non-authorizing preview receipt', () => {
    expect(parseCleanupActionPlanPreview(preview)).toEqual(preview);
    expect(() =>
      parseCleanupActionPlanPreview({ ...preview, source_path: '/private/authorized/report.md' }),
    ).toThrow('Invalid cleanup action preview response');
    expect(() =>
      parseCleanupActionPlanPreview({
        ...preview,
        policy: { ...preview.policy, action_authorized: true },
      }),
    ).toThrow();
    expect(() => parseCleanupActionPlanPreview({ ...preview, journal_sequence: 2 })).toThrow();
    expect(() => parseCleanupActionPlanPreview({ ...preview, keeper_node_id: null })).toThrow();
  });

  it('requests detail with source IDs only and accepts a refreshed observation', async () => {
    const invokeCommand = vi.fn().mockResolvedValue(detail);
    await expect(getCleanupSourceDetail(inbox.items[0], invokeCommand)).resolves.toEqual(detail);
    expect(invokeCommand).toHaveBeenCalledWith(GET_CLEANUP_SOURCE_DETAIL_COMMAND, {
      scopeId: 2,
      sourceKind: 'exact_duplicate',
      sourceId: 7,
      sourceObservationId: 9,
    });
  });

  it('creates a durable preview with IDs only and rejects invalid local selections', async () => {
    const invokeCommand = vi.fn().mockResolvedValue(preview);
    await expect(createCleanupActionPreview(detail, 11, 12, invokeCommand)).resolves.toEqual(
      preview,
    );
    expect(invokeCommand).toHaveBeenCalledWith(CREATE_CLEANUP_PREVIEW_COMMAND, {
      scopeId: 2,
      sourceKind: 'exact_duplicate',
      sourceId: 7,
      sourceObservationId: 10,
      targetNodeId: 11,
      keeperNodeId: 12,
    });
    await expect(createCleanupActionPreview(detail, 11, null, invokeCommand)).rejects.toThrow(
      'Invalid cleanup preview selection',
    );
    await expect(createCleanupActionPreview(detail, 11, 11, invokeCommand)).rejects.toThrow(
      'Invalid cleanup preview selection',
    );
    expect(invokeCommand).toHaveBeenCalledTimes(1);
  });

  it('locks version direction and screenshot previews to one target without a keeper', async () => {
    const versionDetail: CleanupSourceDetail = {
      ...detail,
      source_kind: 'version',
      selection_rule: 'older_target_newer_keeper',
      members: [
        { ...detail.members[0], role: 'older_version' },
        { ...detail.members[1], role: 'newer_version' },
      ],
    };
    const screenshotDetail: CleanupSourceDetail = {
      ...detail,
      source_kind: 'screenshot_review_group',
      selection_rule: 'single_target_no_keeper',
      members: detail.members.map((member) => ({ ...member, role: 'screenshot_candidate' })),
    };
    const invokeCommand = vi.fn();
    await expect(createCleanupActionPreview(versionDetail, 12, 11, invokeCommand)).rejects.toThrow(
      'Invalid cleanup preview selection',
    );
    await expect(
      createCleanupActionPreview(screenshotDetail, 11, 12, invokeCommand),
    ).rejects.toThrow('Invalid cleanup preview selection');
    expect(invokeCommand).not.toHaveBeenCalled();
  });
});
