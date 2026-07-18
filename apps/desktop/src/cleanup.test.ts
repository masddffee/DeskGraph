import { describe, expect, it, vi } from 'vitest';

import {
  REFRESH_CLEANUP_INBOX_COMMAND,
  parseSmartCleanupInbox,
  parseSmartCleanupInboxItem,
  refreshSmartCleanupInbox,
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
