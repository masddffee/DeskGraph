import { describe, expect, it, vi } from 'vitest';

import {
  CANCEL_SCREENSHOT_OCR_JOB_COMMAND,
  CREATE_SCREENSHOT_OCR_JOB_COMMAND,
  EXTRACTION_STATS_COMMAND,
  RECENT_EXTRACTIONS_COMMAND,
  RESUME_SCREENSHOT_OCR_JOB_COMMAND,
  RUN_SCREENSHOT_OCR_JOB_COMMAND,
  SCREENSHOT_OCR_JOB_FOR_NODE_COMMAND,
  SCREENSHOT_OCR_JOB_STATUS_COMMAND,
  activeScreenshotOcrJobIds,
  cancelScreenshotOcrJob,
  createScreenshotOcrJob,
  loadScreenshotOcrJobStatus,
  loadExtractionStats,
  loadRecentExtractions,
  mergePolledScreenshotOcrJob,
  parseExtractionJob,
  parseExtractionJobs,
  parseExtractionStats,
  resumeScreenshotOcrJob,
  isScreenshotCandidateDisplayPath,
  isScreenshotOcrCapacityBusy,
  loadScreenshotOcrJobForNode,
  runScreenshotOcrJob,
  type ExtractionJobProgress,
  type ExtractionStats,
} from './extraction';

const stats: ExtractionStats = {
  api_version: 'deskgraph.extraction-stats.v1',
  active_chunk_count: 4,
  extracted_file_count: 2,
  completed_job_count: 2,
  failed_job_count: 1,
  cancelled_job_count: 1,
};

const job: ExtractionJobProgress = {
  api_version: 'deskgraph.extraction-job.v2',
  job_id: 7,
  scope_id: 2,
  node_id: 9,
  operation: 'content',
  status: 'completed',
  provider_id: 'deskgraph.utf8-text',
  provider_version: '1',
  error_code: null,
  source_bytes: 128,
  output_bytes: 128,
  chunk_count: 1,
  elapsed_ms: 4,
  cancel_requested: false,
};

describe('extraction contract', () => {
  it('offers screenshot OCR only for supported image display-path suffixes', () => {
    for (const displayPath of [
      'Screenshot.png',
      'Screenshot.JPG',
      'Screenshot.JpEg',
      '/Users/example/Desktop/Screenshot.png',
      'C:\\Users\\example\\Pictures\\Screenshot.jpeg',
    ]) {
      expect(isScreenshotCandidateDisplayPath(displayPath)).toBe(true);
    }

    for (const displayPath of [
      '',
      '/Users/example/Desktop/',
      'C:\\Users\\example\\Pictures\\',
      '.png',
      'Screenshot.gif',
      'Screenshot.png.bak',
      'Screenshot.png.',
      'Screenshot',
    ]) {
      expect(isScreenshotCandidateDisplayPath(displayPath)).toBe(false);
    }
  });

  it('polls only active screenshot OCR jobs', () => {
    expect(
      activeScreenshotOcrJobIds([
        { ...job, job_id: 1, operation: 'content', status: 'running' },
        { ...job, job_id: 2, operation: 'screenshot_ocr', status: 'queued' },
        { ...job, job_id: 3, operation: 'screenshot_ocr', status: 'running' },
        { ...job, job_id: 4, operation: 'screenshot_ocr', status: 'completed' },
      ]),
    ).toEqual([2, 3]);
  });

  it('does not let a stale poll regress a stable OCR state', () => {
    for (const status of ['completed', 'failed', 'cancelled', 'interrupted'] as const) {
      const stable = { ...job, operation: 'screenshot_ocr' as const, status };
      expect(
        mergePolledScreenshotOcrJob([stable], {
          ...stable,
          status: 'running',
          cancel_requested: false,
        }),
      ).toEqual([stable]);
    }

    const running = {
      ...job,
      operation: 'screenshot_ocr' as const,
      status: 'running' as const,
    };
    const completed = { ...running, status: 'completed' as const };
    expect(mergePolledScreenshotOcrJob([running], completed)).toEqual([completed]);
  });

  it('recognizes only the fixed native OCR capacity error', () => {
    expect(isScreenshotOcrCapacityBusy('extraction_ocr_capacity_busy')).toBe(true);
    expect(isScreenshotOcrCapacityBusy(new Error('extraction_ocr_capacity_busy'))).toBe(true);
    expect(isScreenshotOcrCapacityBusy('extraction_ocr_provider_failed')).toBe(false);
    expect(isScreenshotOcrCapacityBusy({ message: 'extraction_ocr_capacity_busy' })).toBe(false);
  });

  it('validates aggregate counts and rejects invalid values', () => {
    expect(parseExtractionStats(stats)).toEqual(stats);
    expect(() => parseExtractionStats({ ...stats, active_chunk_count: -1 })).toThrow(
      'Invalid extraction statistics response',
    );
  });

  it('validates every durable state without accepting path or text fields as contracts', () => {
    for (const status of [
      'queued',
      'running',
      'completed',
      'failed',
      'cancelled',
      'interrupted',
    ] as const) {
      expect(parseExtractionJob({ ...job, status }).status).toBe(status);
    }
    expect(parseExtractionJobs([job])).toEqual([job]);
    expect(parseExtractionJob({ ...job, operation: 'screenshot_ocr' }).operation).toBe(
      'screenshot_ocr',
    );
    expect(() => parseExtractionJob({ ...job, operation: 'arbitrary' })).toThrow(
      'Invalid extraction job response',
    );
    expect(() => parseExtractionJob({ ...job, output_bytes: -1 })).toThrow(
      'Invalid extraction job response',
    );
    expect(() => parseExtractionJob({ ...job, job_id: 0 })).toThrow(
      'Invalid extraction job response',
    );
    expect(() => parseExtractionJob({ ...job, error_code: 'x'.repeat(129) })).toThrow(
      'Invalid extraction job response',
    );
    expect(() => parseExtractionJob({ ...job, display_path: '/private/secret.png' })).toThrow(
      'Invalid extraction job response',
    );
    expect(() => parseExtractionJob({ ...job, extracted_text: 'untrusted content' })).toThrow(
      'Invalid extraction job response',
    );
    expect(() => parseExtractionJobs({})).toThrow('Invalid extraction job list response');
  });

  it('uses narrow read-only dashboard commands', async () => {
    const invokeCommand = vi.fn().mockImplementation((command: string) => {
      if (command === EXTRACTION_STATS_COMMAND) return Promise.resolve(stats);
      if (command === RECENT_EXTRACTIONS_COMMAND) return Promise.resolve([job]);
      return Promise.reject(new Error('unexpected command'));
    });

    await expect(loadExtractionStats(invokeCommand)).resolves.toEqual(stats);
    await expect(loadRecentExtractions(invokeCommand)).resolves.toEqual([job]);
    expect(invokeCommand).toHaveBeenNthCalledWith(1, EXTRACTION_STATS_COMMAND);
    expect(invokeCommand).toHaveBeenNthCalledWith(2, RECENT_EXTRACTIONS_COMMAND);
  });

  it('uses opaque identifiers for screenshot OCR lifecycle commands', async () => {
    const screenshotOcrJob = {
      ...job,
      operation: 'screenshot_ocr' as const,
      status: 'queued' as const,
    };
    const invokeCommand = vi.fn().mockImplementation((command: string) => {
      if (command === CREATE_SCREENSHOT_OCR_JOB_COMMAND) return Promise.resolve(screenshotOcrJob);
      if (command === RUN_SCREENSHOT_OCR_JOB_COMMAND) {
        return Promise.resolve({ ...screenshotOcrJob, status: 'completed' });
      }
      if (command === SCREENSHOT_OCR_JOB_STATUS_COMMAND) return Promise.resolve(screenshotOcrJob);
      if (command === CANCEL_SCREENSHOT_OCR_JOB_COMMAND) {
        return Promise.resolve({
          ...screenshotOcrJob,
          status: 'cancelled',
          cancel_requested: true,
        });
      }
      if (command === RESUME_SCREENSHOT_OCR_JOB_COMMAND) return Promise.resolve(screenshotOcrJob);
      if (command === SCREENSHOT_OCR_JOB_FOR_NODE_COMMAND) {
        return Promise.resolve(screenshotOcrJob);
      }
      return Promise.reject(new Error('unexpected command'));
    });

    await expect(createScreenshotOcrJob(2, 9, invokeCommand)).resolves.toEqual(screenshotOcrJob);
    await expect(runScreenshotOcrJob(7, invokeCommand)).resolves.toMatchObject({
      status: 'completed',
    });
    await expect(loadScreenshotOcrJobStatus(7, invokeCommand)).resolves.toEqual(screenshotOcrJob);
    await expect(cancelScreenshotOcrJob(7, invokeCommand)).resolves.toMatchObject({
      status: 'cancelled',
      cancel_requested: true,
    });
    await expect(resumeScreenshotOcrJob(7, invokeCommand)).resolves.toEqual(screenshotOcrJob);
    await expect(loadScreenshotOcrJobForNode(2, 9, invokeCommand)).resolves.toEqual(
      screenshotOcrJob,
    );

    expect(invokeCommand).toHaveBeenNthCalledWith(1, CREATE_SCREENSHOT_OCR_JOB_COMMAND, {
      scopeId: 2,
      nodeId: 9,
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(2, RUN_SCREENSHOT_OCR_JOB_COMMAND, { jobId: 7 });
    expect(invokeCommand).toHaveBeenNthCalledWith(3, SCREENSHOT_OCR_JOB_STATUS_COMMAND, {
      jobId: 7,
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(4, CANCEL_SCREENSHOT_OCR_JOB_COMMAND, {
      jobId: 7,
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(5, RESUME_SCREENSHOT_OCR_JOB_COMMAND, {
      jobId: 7,
    });
    expect(invokeCommand).toHaveBeenNthCalledWith(6, SCREENSHOT_OCR_JOB_FOR_NODE_COMMAND, {
      scopeId: 2,
      nodeId: 9,
    });
  });

  it('rejects a non-OCR job returned by a screenshot OCR command', async () => {
    await expect(createScreenshotOcrJob(2, 9, vi.fn().mockResolvedValue(job))).rejects.toThrow(
      'Invalid screenshot OCR job response',
    );
    await expect(resumeScreenshotOcrJob(7, vi.fn().mockResolvedValue(job))).rejects.toThrow(
      'Invalid screenshot OCR job response',
    );
    await expect(loadScreenshotOcrJobForNode(2, 9, vi.fn().mockResolvedValue(job))).rejects.toThrow(
      'Invalid screenshot OCR job response',
    );
  });

  it('accepts an absent node-specific screenshot OCR job', async () => {
    await expect(
      loadScreenshotOcrJobForNode(2, 9, vi.fn().mockResolvedValue(null)),
    ).resolves.toBeNull();
  });
});
