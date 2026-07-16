import { invoke } from '@tauri-apps/api/core';

export const RECENT_WATCH_EVENTS_COMMAND = 'recent_watch_events';

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

export async function loadRecentWatchEvents(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<WatchEventProgress[]> {
  return parseWatchEvents(await invokeCommand(RECENT_WATCH_EVENTS_COMMAND));
}
