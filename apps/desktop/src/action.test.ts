import { describe, expect, it, vi } from 'vitest';

import {
  CREATE_RENAME_PREVIEW_COMMAND,
  RECENT_ACTION_PLANS_COMMAND,
  createRenamePreview,
  loadRecentActionPlans,
  parseActionPlanPreview,
  parseActionPlanSummaries,
  type ActionPlanState,
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
  api_version: 'deskgraph.action-plan.v2',
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
  api_version: 'deskgraph.action-plan-summary.v2',
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

  it('accepts every durable history state and a later positive journal sequence', () => {
    const states: ActionPlanState[] = [
      'previewed',
      'execute_requested',
      'direct_rename_intent',
      'executed',
      'undo_requested',
      'undo_rename_intent',
      'undone',
      'needs_attention',
    ];

    expect(
      parseActionPlanSummaries(
        states.map((state, index) => ({ ...summary, state, journal_sequence: index + 1 })),
      ),
    ).toEqual(states.map((state, index) => ({ ...summary, state, journal_sequence: index + 1 })));
  });

  it('rejects unknown operations, invalid states, invalid journal shapes, and injected summary paths', () => {
    expect(() => parseActionPlanPreview({ ...preview, operation: 'delete' })).toThrow(
      'Invalid action preview response',
    );
    expect(() => parseActionPlanPreview({ ...preview, state: 'executed' })).toThrow(
      'Invalid action preview response',
    );
    expect(() => parseActionPlanPreview({ ...preview, filename: 'private.txt' })).toThrow(
      'Invalid action preview response',
    );
    expect(() => parseActionPlanPreview({ ...preview, journal_sequence: 0 })).toThrow(
      'Invalid action preview response',
    );
    expect(() =>
      parseActionPlanPreview({
        ...preview,
        policy: { ...preview.policy, checks: [...checks, 'llm_approved'] },
      }),
    ).toThrow('Invalid action policy response');
    expect(() =>
      parseActionPlanPreview({
        ...preview,
        policy: { ...preview.policy, source_path: '/private' },
      }),
    ).toThrow('Invalid action policy response');
    expect(() => parseActionPlanSummaries([{ ...summary, source_path: '/private' }])).toThrow(
      'Invalid action plan summary response',
    );
    expect(() =>
      parseActionPlanSummaries([{ ...summary, destination_display_path: '/private' }]),
    ).toThrow('Invalid action plan summary response');
    expect(() => parseActionPlanSummaries([{ ...summary, filename: 'private.txt' }])).toThrow(
      'Invalid action plan summary response',
    );
    expect(() => parseActionPlanSummaries([{ ...summary, state: 'delete_requested' }])).toThrow(
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
