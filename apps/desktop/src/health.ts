import { invoke } from '@tauri-apps/api/core';

export const HEALTH_COMMAND = 'health';

export type LifecycleState = 'ready' | 'not_initialized' | 'disabled';

export interface ComponentHealth {
  state: LifecycleState;
  reason: string;
}

export interface HealthReport {
  api_version: 'deskgraph.health.v1';
  product: 'DeskGraph';
  app_version: string;
  status: 'ok';
  platform: {
    os: string;
    architecture: string;
  };
  database: ComponentHealth;
  providers: {
    ocr: ComponentHealth;
    embeddings: ComponentHealth;
    local_llm: ComponentHealth;
  };
  privacy: {
    local_only_default: boolean;
    network_required: boolean;
    filesystem_locations_included: boolean;
    authorized_scope_count: number;
  };
}

type InvokeCommand = (command: string) => Promise<unknown>;

const lifecycleStates = new Set<LifecycleState>(['ready', 'not_initialized', 'disabled']);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isComponentHealth(value: unknown): value is ComponentHealth {
  return (
    isRecord(value) &&
    typeof value.state === 'string' &&
    lifecycleStates.has(value.state as LifecycleState) &&
    typeof value.reason === 'string'
  );
}

export function parseHealthReport(value: unknown): HealthReport {
  if (!isRecord(value) || !isRecord(value.platform) || !isRecord(value.providers)) {
    throw new Error('Invalid health response');
  }

  if (!isRecord(value.privacy)) {
    throw new Error('Invalid health response');
  }

  const valid =
    value.api_version === 'deskgraph.health.v1' &&
    value.product === 'DeskGraph' &&
    typeof value.app_version === 'string' &&
    value.status === 'ok' &&
    typeof value.platform.os === 'string' &&
    typeof value.platform.architecture === 'string' &&
    isComponentHealth(value.database) &&
    isComponentHealth(value.providers.ocr) &&
    isComponentHealth(value.providers.embeddings) &&
    isComponentHealth(value.providers.local_llm) &&
    typeof value.privacy.local_only_default === 'boolean' &&
    typeof value.privacy.network_required === 'boolean' &&
    typeof value.privacy.filesystem_locations_included === 'boolean' &&
    typeof value.privacy.authorized_scope_count === 'number';

  if (!valid) {
    throw new Error('Invalid health response');
  }

  return value as unknown as HealthReport;
}

export async function loadHealthReport(
  invokeCommand: InvokeCommand = (command) => invoke(command),
): Promise<HealthReport> {
  const response = await invokeCommand(HEALTH_COMMAND);
  return parseHealthReport(response);
}

export function lifecycleLabel(state: LifecycleState): string {
  switch (state) {
    case 'ready':
      return 'Ready';
    case 'not_initialized':
      return 'Not initialized';
    case 'disabled':
      return 'Disabled';
  }
}
