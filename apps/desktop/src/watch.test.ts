import { describe, expect, it, vi } from 'vitest';

import {
  RECENT_WATCH_EVENTS_COMMAND,
  WATCH_RUNTIME_STATUS_COMMAND,
  loadRecentWatchEvents,
  loadWatchRuntimeStatus,
  parseWatchEvent,
  parseWatchEvents,
  parseWatchRuntimeStatus,
  type WatchEventProgress,
  type WatchRuntimeStatus,
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

const runtime: WatchRuntimeStatus = {
  api_version: 'deskgraph.watch-runtime.v2',
  state: 'running',
  adapter: 'native_with_periodic_reconciliation',
  poll_interval_ms: 300_000,
  periodic_reconciliation_enabled: true,
  last_cycle_unix_ms: 1_000,
  authorized_scope_count: 2,
  active_event_count: 1,
  degraded_scope_count: 0,
  deferred_scope_count: 0,
  native_watched_scope_count: 2,
  native_overflow_count: 0,
  next_wake_unix_ms: 301_000,
  last_error_code: null,
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

  it('accepts only path-free v2 runtime states with periodic safety reconciliation', async () => {
    expect(parseWatchRuntimeStatus(runtime)).toEqual(runtime);
    expect(
      parseWatchRuntimeStatus({
        ...runtime,
        state: 'degraded',
        degraded_scope_count: 1,
        last_error_code: 'scope_canonicalization_failed',
      }).state,
    ).toBe('degraded');
    expect(
      parseWatchRuntimeStatus({
        ...runtime,
        state: 'degraded',
        adapter: 'periodic_reconciliation_only',
        native_watched_scope_count: 0,
        last_error_code: 'native_watch_adapter_unavailable',
      }).adapter,
    ).toBe('periodic_reconciliation_only');
    expect(() => parseWatchRuntimeStatus({ ...runtime, adapter: 'future_adapter' })).toThrow(
      'Invalid watch runtime response',
    );
    expect(() => parseWatchRuntimeStatus({ ...runtime, adapter: '/Users/private' })).toThrow(
      'Invalid watch runtime response',
    );
    expect(() =>
      parseWatchRuntimeStatus({ ...runtime, periodic_reconciliation_enabled: false }),
    ).toThrow('Invalid watch runtime response');
    expect(() =>
      parseWatchRuntimeStatus({
        ...runtime,
        state: 'degraded',
        last_error_code: '/private/error',
      }),
    ).toThrow('Invalid watch runtime response');
    expect(() => parseWatchRuntimeStatus({ ...runtime, poll_interval_ms: 100 })).toThrow(
      'Invalid watch runtime response',
    );

    const invokeCommand = vi.fn().mockResolvedValue(runtime);
    await expect(loadWatchRuntimeStatus(invokeCommand)).resolves.toEqual(runtime);
    expect(invokeCommand).toHaveBeenCalledWith(WATCH_RUNTIME_STATUS_COMMAND);
  });

  it('enforces coherent runtime lifecycle fields', () => {
    const startingRuntime = {
      ...runtime,
      state: 'starting' as const,
      last_cycle_unix_ms: null,
      authorized_scope_count: 0,
      active_event_count: 0,
      degraded_scope_count: 0,
      deferred_scope_count: 0,
      native_watched_scope_count: 0,
      native_overflow_count: 0,
      next_wake_unix_ms: null,
      last_error_code: null,
    };
    expect(parseWatchRuntimeStatus(startingRuntime).state).toBe('starting');

    for (const invalidRuntime of [
      { ...runtime, state: 'stopped', next_wake_unix_ms: null, last_error_code: 'watch_failed' },
      { ...runtime, state: 'stopped', next_wake_unix_ms: 301_000 },
      { ...startingRuntime, authorized_scope_count: 1 },
      { ...startingRuntime, active_event_count: 1 },
      { ...startingRuntime, degraded_scope_count: 1 },
      { ...startingRuntime, deferred_scope_count: 1 },
      { ...startingRuntime, native_watched_scope_count: 1 },
      { ...startingRuntime, native_overflow_count: 1 },
      { ...runtime, native_watched_scope_count: -1 },
      { ...runtime, native_overflow_count: -1 },
      { ...startingRuntime, last_cycle_unix_ms: 1 },
      { ...startingRuntime, next_wake_unix_ms: 1 },
      { ...startingRuntime, last_error_code: 'watch_failed' },
      { ...runtime, degraded_scope_count: 1 },
      { ...runtime, adapter: 'periodic_reconciliation_only' },
      { ...runtime, state: 'stopped', next_wake_unix_ms: null, deferred_scope_count: 1 },
      { ...runtime, state: 'stopped', next_wake_unix_ms: null, native_watched_scope_count: 1 },
      { ...runtime, next_wake_unix_ms: 999 },
      {
        ...runtime,
        state: 'degraded',
        degraded_scope_count: 1,
        last_error_code: 'watch_failed',
        next_wake_unix_ms: 999,
      },
    ]) {
      expect(() => parseWatchRuntimeStatus(invalidRuntime)).toThrow(
        'Invalid watch runtime response',
      );
    }
  });
});
