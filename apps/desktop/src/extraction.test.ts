import { describe, expect, it, vi } from 'vitest';

import {
  EXTRACTION_STATS_COMMAND,
  RECENT_EXTRACTIONS_COMMAND,
  loadExtractionStats,
  loadRecentExtractions,
  parseExtractionJob,
  parseExtractionJobs,
  parseExtractionStats,
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
});
