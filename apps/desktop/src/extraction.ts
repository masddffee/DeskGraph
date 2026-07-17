import { invoke } from '@tauri-apps/api/core';

export const EXTRACTION_STATS_COMMAND = 'content_extraction_stats';
export const RECENT_EXTRACTIONS_COMMAND = 'recent_content_extractions';

export type ExtractionStatus =
  'queued' | 'running' | 'completed' | 'failed' | 'cancelled' | 'interrupted';

export type ExtractionOperation = 'content' | 'screenshot_ocr';

export interface ExtractionStats {
  api_version: 'deskgraph.extraction-stats.v1';
  active_chunk_count: number;
  extracted_file_count: number;
  completed_job_count: number;
  failed_job_count: number;
  cancelled_job_count: number;
}

export interface ExtractionJobProgress {
  api_version: 'deskgraph.extraction-job.v2';
  job_id: number;
  scope_id: number;
  node_id: number;
  operation: ExtractionOperation;
  status: ExtractionStatus;
  provider_id: string | null;
  provider_version: string | null;
  error_code: string | null;
  source_bytes: number;
  output_bytes: number;
  chunk_count: number;
  elapsed_ms: number;
  cancel_requested: boolean;
}

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isOptionalString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}

export function parseExtractionStats(value: unknown): ExtractionStats {
  if (!isRecord(value)) throw new Error('Invalid extraction statistics response');
  const valid =
    value.api_version === 'deskgraph.extraction-stats.v1' &&
    isCount(value.active_chunk_count) &&
    isCount(value.extracted_file_count) &&
    isCount(value.completed_job_count) &&
    isCount(value.failed_job_count) &&
    isCount(value.cancelled_job_count);
  if (!valid) throw new Error('Invalid extraction statistics response');
  return value as unknown as ExtractionStats;
}

export function parseExtractionJob(value: unknown): ExtractionJobProgress {
  if (!isRecord(value)) throw new Error('Invalid extraction job response');
  const validStatus =
    value.status === 'queued' ||
    value.status === 'running' ||
    value.status === 'completed' ||
    value.status === 'failed' ||
    value.status === 'cancelled' ||
    value.status === 'interrupted';
  const valid =
    value.api_version === 'deskgraph.extraction-job.v2' &&
    (value.operation === 'content' || value.operation === 'screenshot_ocr') &&
    validStatus &&
    isCount(value.job_id) &&
    isCount(value.scope_id) &&
    isCount(value.node_id) &&
    isOptionalString(value.provider_id) &&
    isOptionalString(value.provider_version) &&
    isOptionalString(value.error_code) &&
    isCount(value.source_bytes) &&
    isCount(value.output_bytes) &&
    isCount(value.chunk_count) &&
    isCount(value.elapsed_ms) &&
    typeof value.cancel_requested === 'boolean';
  if (!valid) throw new Error('Invalid extraction job response');
  return value as unknown as ExtractionJobProgress;
}

export function parseExtractionJobs(value: unknown): ExtractionJobProgress[] {
  if (!Array.isArray(value)) throw new Error('Invalid extraction job list response');
  return value.map(parseExtractionJob);
}

export async function loadExtractionStats(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ExtractionStats> {
  return parseExtractionStats(await invokeCommand(EXTRACTION_STATS_COMMAND));
}

export async function loadRecentExtractions(
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ExtractionJobProgress[]> {
  return parseExtractionJobs(await invokeCommand(RECENT_EXTRACTIONS_COMMAND));
}
