import { describe, expect, it, vi } from 'vitest';

import {
  RECENT_WATCH_EVENTS_COMMAND,
  loadRecentWatchEvents,
  parseWatchEvent,
  parseWatchEvents,
  type WatchEventProgress,
} from './watch';

const event: WatchEventProgress = {
  api_version: 'deskgraph.watch-event.v1',
  event_id: 7,
  scope_id: 2,
  status: 'reconciling',
  observation_count: 3,
  stable_after_unix_ms: 1_000,
  scan_job_id: 9,
  reason: null,
};

describe('watch event contract', () => {
  it('accepts closed path-free states', () => {
    expect(parseWatchEvent(event)).toEqual(event);
    expect(
      parseWatchEvent({
        ...event,
        status: 'ignored',
        scan_job_id: null,
        reason: 'temporary_download',
      }).status,
    ).toBe('ignored');
    expect(parseWatchEvents([event])).toEqual([event]);
  });

  it('rejects unknown states and inconsistent reason or scan contracts', () => {
    expect(() => parseWatchEvent({ ...event, status: 'moving_files' })).toThrow(
      'Invalid watch event response',
    );
    expect(() => parseWatchEvent({ ...event, scan_job_id: null })).toThrow(
      'Invalid watch event response',
    );
    expect(() => parseWatchEvent({ ...event, reason: 'private-path' })).toThrow(
      'Invalid watch event response',
    );
  });

  it('uses one narrow read-only dashboard command', async () => {
    const invokeCommand = vi.fn().mockResolvedValue([event]);
    await expect(loadRecentWatchEvents(invokeCommand)).resolves.toEqual([event]);
    expect(invokeCommand).toHaveBeenCalledWith(RECENT_WATCH_EVENTS_COMMAND);
  });
});
