import { invoke } from '@tauri-apps/api/core';

export const RECENT_WATCH_EVENTS_COMMAND = 'recent_watch_events';
export const WATCH_RUNTIME_STATUS_COMMAND = 'watch_runtime_status';

export type WatchEventStatus = 'stabilizing' | 'reconciling' | 'completed' | 'ignored' | 'failed';
export type WatchEventReason =
  | 'temporary_download'
  | 'hidden_entry'
  | 'unsupported_entry'
  | 'source_unavailable'
  | 'reconcile_failed';

export interface WatchEventProgress {
  api_version: 'deskgraph.watch-event.v1';
  event_id: number;
  scope_id: number;
  status: WatchEventStatus;
  observation_count: number;
  stable_after_unix_ms: number;
  scan_job_id: number | null;
  reason: WatchEventReason | null;
}

export type WatchRuntimeState = 'starting' | 'running' | 'degraded' | 'stopped';

export interface WatchRuntimeStatus {
  api_version: 'deskgraph.watch-runtime.v1';
  state: WatchRuntimeState;
  adapter: 'bounded_metadata_polling';
  poll_interval_ms: number;
  last_cycle_unix_ms: number | null;
  authorized_scope_count: number;
  active_event_count: number;
  degraded_scope_count: number;
  deferred_scope_count: number;
  next_wake_unix_ms: number | null;
  last_error_code: string | null;
}

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isId(value: unknown): value is number {
  return isCount(value) && value > 0;
}

function isStatus(value: unknown): value is WatchEventStatus {
  return (
    value === 'stabilizing' ||
    value === 'reconciling' ||
    value === 'completed' ||
    value === 'ignored' ||
    value === 'failed'
  );
}

function isRuntimeState(value: unknown): value is WatchRuntimeState {
  return value === 'starting' || value === 'running' || value === 'degraded' || value === 'stopped';
}

function isFixedErrorCode(value: unknown): value is string | null {
  return value === null || (typeof value === 'string' && /^[a-z][a-z0-9_]{0,63}$/.test(value));
}

function isReason(value: unknown): value is WatchEventReason | null {
  return (
    value === null ||
    value === 'temporary_download' ||
    value === 'hidden_entry' ||
    value === 'unsupported_entry' ||
    value === 'source_unavailable' ||
    value === 'reconcile_failed'
  );
}

export function parseWatchEvent(value: unknown): WatchEventProgress {
  if (!isRecord(value)) throw new Error('Invalid watch event response');
  const valid =
    value.api_version === 'deskgraph.watch-event.v1' &&
    isId(value.event_id) &&
    isId(value.scope_id) &&
    isStatus(value.status) &&
    isId(value.observation_count) &&
    isCount(value.stable_after_unix_ms) &&
    (value.scan_job_id === null || isId(value.scan_job_id)) &&
    isReason(value.reason) &&
    (value.status === 'reconciling' ? isId(value.scan_job_id) : true) &&
    (value.status === 'ignored' || value.status === 'failed'
      ? value.reason !== null
      : value.reason === null);
  if (!valid) throw new Error('Invalid watch event response');
  return value as unknown as WatchEventProgress;
}

export function parseWatchEvents(value: unknown): WatchEventProgress[] {
  if (!Array.isArray(value)) throw new Error('Invalid watch event list response');
  return value.map(parseWatchEvent);
}

export function parseWatchRuntimeStatus(value: unknown): WatchRuntimeStatus {
  if (!isRecord(value)) throw new Error('Invalid watch runtime response');
  const state = value.state;
  const lastCycle = value.last_cycle_unix_ms;
  const nextWake = value.next_wake_unix_ms;
  const hasNonDecreasingWake =
    lastCycle === null ||
    nextWake === null ||
    (isCount(lastCycle) && isCount(nextWake) && nextWake >= lastCycle);
  const hasCoherentState =
    (state === 'starting'
      ? value.authorized_scope_count === 0 &&
        value.active_event_count === 0 &&
        value.degraded_scope_count === 0 &&
        value.deferred_scope_count === 0 &&
        lastCycle === null &&
        nextWake === null &&
        value.last_error_code === null
      : state === 'running'
        ? value.degraded_scope_count === 0 && value.last_error_code === null
        : state === 'degraded'
          ? value.last_error_code !== null
          : state === 'stopped'
            ? value.degraded_scope_count === 0 &&
              value.deferred_scope_count === 0 &&
              nextWake === null &&
              value.last_error_code === null
            : false) && (state === 'running' || state === 'degraded' ? hasNonDecreasingWake : true);
  const valid =
    value.api_version === 'deskgraph.watch-runtime.v1' &&
    isRuntimeState(state) &&
    value.adapter === 'bounded_metadata_polling' &&
    isCount(value.poll_interval_ms) &&
    value.poll_interval_ms >= 5_000 &&
    value.poll_interval_ms <= 3_600_000 &&
    (lastCycle === null || isCount(lastCycle)) &&
    isCount(value.authorized_scope_count) &&
    isCount(value.active_event_count) &&
    isCount(value.degraded_scope_count) &&
    isCount(value.deferred_scope_count) &&
    (nextWake === null || isCount(nextWake)) &&
    isFixedErrorCode(value.last_error_code) &&
    hasCoherentState;
  if (!valid) throw new Error('Invalid watch runtime response');
  return value as unknown as WatchRuntimeStatus;
}

export async function loadRecentWatchEvents(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<WatchEventProgress[]> {
  return parseWatchEvents(await invokeCommand(RECENT_WATCH_EVENTS_COMMAND));
}

export async function loadWatchRuntimeStatus(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<WatchRuntimeStatus> {
  return parseWatchRuntimeStatus(await invokeCommand(WATCH_RUNTIME_STATUS_COMMAND));
}
