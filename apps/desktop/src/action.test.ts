import { describe, expect, it, vi } from 'vitest';

import {
  CREATE_RENAME_PREVIEW_COMMAND,
  RECENT_ACTION_PLANS_COMMAND,
  createRenamePreview,
  loadRecentActionPlans,
  parseActionPlanPreview,
  parseActionPlanSummaries,
  type ActionPlanPreview,
  type ActionPlanSummary,
} from './action';

const checks = [
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

const preview: ActionPlanPreview = {
  api_version: 'deskgraph.action-plan.v1',
  plan_id: 8,
  operation: 'rename',
  state: 'previewed',
  scope_id: 2,
  node_id: 9,
  source_path: '/authorized/private-draft.md',
  destination_path: '/authorized/private-final.md',
  execution_strategy: 'direct',
  policy: {
    api_version: 'deskgraph.action-policy.v1',
    decision: 'allowed',
    checks: [...checks],
  },
  journal_sequence: 1,
  created_at_unix_ms: 10,
};

const summary: ActionPlanSummary = {
  api_version: 'deskgraph.action-plan-summary.v1',
  plan_id: 8,
  operation: 'rename',
  state: 'previewed',
  scope_id: 2,
  node_id: 9,
  execution_strategy: 'direct',
  journal_sequence: 1,
  created_at_unix_ms: 10,
};

describe('action preview contract', () => {
  it('accepts an explicit before/after preview and path-free summaries', () => {
    expect(parseActionPlanPreview(preview)).toEqual(preview);
    expect(parseActionPlanSummaries([summary])).toEqual([summary]);
    expect(JSON.stringify(summary)).not.toContain('private-draft.md');
    expect(JSON.stringify(summary)).not.toContain('private-final.md');
  });

  it('rejects unknown operations, mutable journal shapes, and injected summary paths', () => {
    expect(() => parseActionPlanPreview({ ...preview, operation: 'delete' })).toThrow(
      'Invalid action preview response',
    );
    expect(() => parseActionPlanPreview({ ...preview, journal_sequence: 2 })).toThrow(
      'Invalid action preview response',
    );
    expect(() =>
      parseActionPlanPreview({
        ...preview,
        policy: { ...preview.policy, checks: [...checks, 'llm_approved'] },
      }),
    ).toThrow('Invalid action policy response');
    expect(() => parseActionPlanSummaries([{ ...summary, source_path: '/private' }])).toThrow(
      'Invalid action plan summary response',
    );
  });

  it('uses narrow preview-create and path-free history commands', async () => {
    const invokeCommand = vi.fn().mockImplementation((command: string) => {
      if (command === CREATE_RENAME_PREVIEW_COMMAND) return Promise.resolve(preview);
      return Promise.resolve([summary]);
    });

    await expect(
      createRenamePreview(2, '/authorized/private-draft.md', 'private-final.md', invokeCommand),
    ).resolves.toEqual(preview);
    await expect(loadRecentActionPlans(invokeCommand)).resolves.toEqual([summary]);
    expect(invokeCommand).toHaveBeenNthCalledWith(1, CREATE_RENAME_PREVIEW_COMMAND, {
      scopeId: 2,
      sourcePath: '/authorized/private-draft.md',
      newName: 'private-final.md',
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(2, RECENT_ACTION_PLANS_COMMAND);
  });
});
