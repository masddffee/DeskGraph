import { describe, expect, it, vi } from 'vitest';

import {
  HEALTH_COMMAND,
  lifecycleLabel,
  loadHealthReport,
  parseHealthReport,
  type HealthReport,
} from './health';

const report: HealthReport = {
  api_version: 'deskgraph.health.v1',
  product: 'DeskGraph',
  app_version: '0.1.0',
  status: 'ok',
  platform: { os: 'macos', architecture: 'aarch64' },
  database: { state: 'not_initialized', reason: 'manifest_database_pending_m1' },
  providers: {
    ocr: { state: 'disabled', reason: 'optional_provider_not_configured' },
    embeddings: { state: 'disabled', reason: 'optional_provider_not_configured' },
    local_llm: { state: 'disabled', reason: 'optional_provider_not_configured' },
  },
  privacy: {
    local_only_default: true,
    network_required: false,
    filesystem_locations_included: false,
    authorized_scope_count: 0,
  },
};

describe('health contract', () => {
  it('accepts the shared Rust schema', () => {
    expect(parseHealthReport(report)).toEqual(report);
  });

  it('rejects an unvalidated payload', () => {
    expect(() => parseHealthReport({ ...report, privacy: { network_required: false } })).toThrow(
      'Invalid health response',
    );
  });

  it('invokes the exact Tauri health command and validates the response', async () => {
    const invokeCommand = vi.fn().mockResolvedValue(report);

    await expect(loadHealthReport(invokeCommand)).resolves.toEqual(report);
    expect(invokeCommand).toHaveBeenCalledWith(HEALTH_COMMAND);
    expect(HEALTH_COMMAND).toBe('health');
  });

  it('uses human-readable lifecycle labels', () => {
    expect(lifecycleLabel('not_initialized')).toBe('Not initialized');
    expect(lifecycleLabel('disabled')).toBe('Disabled');
  });
});
