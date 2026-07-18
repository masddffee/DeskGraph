import { invoke } from '@tauri-apps/api/core';

export const EXTRACTION_STATS_COMMAND = 'content_extraction_stats';
export const RECENT_EXTRACTIONS_COMMAND = 'recent_content_extractions';
export const CREATE_SCREENSHOT_OCR_JOB_COMMAND = 'create_screenshot_ocr_job';
export const RUN_SCREENSHOT_OCR_JOB_COMMAND = 'run_screenshot_ocr_job';
export const SCREENSHOT_OCR_JOB_STATUS_COMMAND = 'screenshot_ocr_job_status';
export const CANCEL_SCREENSHOT_OCR_JOB_COMMAND = 'cancel_screenshot_ocr_job';
export const RESUME_SCREENSHOT_OCR_JOB_COMMAND = 'resume_screenshot_ocr_job';
export const SCREENSHOT_OCR_JOB_FOR_NODE_COMMAND = 'screenshot_ocr_job_for_node';
const SCREENSHOT_OCR_CAPACITY_BUSY = 'extraction_ocr_capacity_busy';

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

function isId(value: unknown): value is number {
  return isCount(value) && value > 0;
}

function isOptionalString(value: unknown): value is string | null {
  return value === null || (typeof value === 'string' && value.length <= 128);
}

function hasOnlyKeys(value: Record<string, unknown>, allowedKeys: readonly string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === allowedKeys.length && keys.every((key) => allowedKeys.includes(key));
}

/**
 * A display-only affordance. The backend must independently validate the
 * scoped node, current file identity, and image format before creating OCR.
 */
export function isScreenshotCandidateDisplayPath(displayPath: string): boolean {
  const basename = displayPath.split(/[\\/]/).at(-1);
  if (!basename) return false;

  const extensionStart = basename.lastIndexOf('.');
  if (extensionStart <= 0) return false;

  const extension = basename.slice(extensionStart).toLowerCase();
  return extension === '.png' || extension === '.jpg' || extension === '.jpeg';
}

export function activeScreenshotOcrJobIds(jobs: ExtractionJobProgress[]): number[] {
  return jobs
    .filter(
      (job) =>
        job.operation === 'screenshot_ocr' && (job.status === 'queued' || job.status === 'running'),
    )
    .map((job) => job.job_id);
}

export function mergePolledScreenshotOcrJob(
  jobs: ExtractionJobProgress[],
  incoming: ExtractionJobProgress,
): ExtractionJobProgress[] {
  const current = jobs.find((job) => job.job_id === incoming.job_id);
  const currentIsStable =
    current?.status === 'completed' ||
    current?.status === 'failed' ||
    current?.status === 'cancelled' ||
    current?.status === 'interrupted';
  const incomingIsActive = incoming.status === 'queued' || incoming.status === 'running';

  if (currentIsStable && incomingIsActive) return jobs;
  return [incoming, ...jobs.filter((job) => job.job_id !== incoming.job_id)];
}

export function isScreenshotOcrCapacityBusy(error: unknown): boolean {
  return (
    error === SCREENSHOT_OCR_CAPACITY_BUSY ||
    (error instanceof Error && error.message === SCREENSHOT_OCR_CAPACITY_BUSY)
  );
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
    hasOnlyKeys(value, [
      'api_version',
      'job_id',
      'scope_id',
      'node_id',
      'operation',
      'status',
      'provider_id',
      'provider_version',
      'error_code',
      'source_bytes',
      'output_bytes',
      'chunk_count',
      'elapsed_ms',
      'cancel_requested',
    ]) &&
    value.api_version === 'deskgraph.extraction-job.v2' &&
    (value.operation === 'content' || value.operation === 'screenshot_ocr') &&
    validStatus &&
    isId(value.job_id) &&
    isId(value.scope_id) &&
    isId(value.node_id) &&
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

function parseScreenshotOcrJob(value: unknown): ExtractionJobProgress {
  const job = parseExtractionJob(value);
  if (job.operation !== 'screenshot_ocr') {
    throw new Error('Invalid screenshot OCR job response');
  }
  return job;
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

export async function createScreenshotOcrJob(
  scopeId: number,
  nodeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ExtractionJobProgress> {
  return parseScreenshotOcrJob(
    await invokeCommand(CREATE_SCREENSHOT_OCR_JOB_COMMAND, { scopeId, nodeId }),
  );
}

export async function runScreenshotOcrJob(
  jobId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ExtractionJobProgress> {
  return parseScreenshotOcrJob(await invokeCommand(RUN_SCREENSHOT_OCR_JOB_COMMAND, { jobId }));
}

export async function loadScreenshotOcrJobStatus(
  jobId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ExtractionJobProgress> {
  return parseScreenshotOcrJob(await invokeCommand(SCREENSHOT_OCR_JOB_STATUS_COMMAND, { jobId }));
}

export async function cancelScreenshotOcrJob(
  jobId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ExtractionJobProgress> {
  return parseScreenshotOcrJob(await invokeCommand(CANCEL_SCREENSHOT_OCR_JOB_COMMAND, { jobId }));
}

export async function resumeScreenshotOcrJob(
  jobId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ExtractionJobProgress> {
  return parseScreenshotOcrJob(await invokeCommand(RESUME_SCREENSHOT_OCR_JOB_COMMAND, { jobId }));
}

export async function loadScreenshotOcrJobForNode(
  scopeId: number,
  nodeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ExtractionJobProgress | null> {
  const value = await invokeCommand(SCREENSHOT_OCR_JOB_FOR_NODE_COMMAND, { scopeId, nodeId });
  return value === null ? null : parseScreenshotOcrJob(value);
}
