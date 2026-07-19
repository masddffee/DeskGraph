import { invoke } from '@tauri-apps/api/core';

export const DISCOVER_PROJECTS_COMMAND = 'discover_projects';
export const PROJECT_SCOPE_HAS_COMPLETED_SCAN_COMMAND = 'project_scope_has_completed_scan';
export const GET_PROJECT_CANDIDATE_DETAIL_COMMAND = 'get_project_candidate_detail';
export const DECIDE_PROJECT_CANDIDATE_COMMAND = 'decide_project_candidate';

export type ProjectCandidateState = 'suggested' | 'accepted' | 'rejected';
export type ProjectDecisionKind = 'accepted' | 'rejected';
export type ProjectSignalKind =
  | 'cargo_manifest'
  | 'java_script_package'
  | 'python_project'
  | 'go_module'
  | 'swift_package'
  | 'xcode_project'
  | 'visual_studio_solution'
  | 'readme';

const PROJECT_SIGNALS = {
  cargo_manifest: { markerName: 'Cargo.toml', weightBasisPoints: 8_500, strong: true },
  java_script_package: { markerName: 'package.json', weightBasisPoints: 7_500, strong: true },
  python_project: { markerName: 'pyproject.toml', weightBasisPoints: 8_000, strong: true },
  go_module: { markerName: 'go.mod', weightBasisPoints: 8_500, strong: true },
  swift_package: { markerName: 'Package.swift', weightBasisPoints: 8_500, strong: true },
  xcode_project: { markerName: '*.xcodeproj', weightBasisPoints: 9_000, strong: true },
  visual_studio_solution: { markerName: '*.sln', weightBasisPoints: 8_500, strong: true },
  readme: { markerName: 'README', weightBasisPoints: 1_500, strong: false },
} as const satisfies Record<
  ProjectSignalKind,
  { markerName: string; weightBasisPoints: number; strong: boolean }
>;

export interface ProjectCandidateSummary {
  api_version: 'deskgraph.project-candidate-summary.v1';
  project_id: number;
  scope_id: number;
  root_folder_node_id: number;
  state: ProjectCandidateState;
  confidence_basis_points: number;
  observed_at_unix_ms: number;
  latest_decision_at_unix_ms: number | null;
}
export interface ProjectCandidate {
  api_version: 'deskgraph.project-candidate.v1';
  project_id: number;
  scope_id: number;
  root_folder_node_id: number;
  root_folder_location_id: number;
  display_path: string;
  state: ProjectCandidateState;
  suggestion: {
    confidence_basis_points: number;
    provenance: { kind: ProjectSignalKind; marker_name: string; weight_basis_points: number }[];
    observed_at_unix_ms: number;
    created_by: 'system_rule';
    provider_id: 'deskgraph.folder-marker-rules';
    provider_version: '1';
    model_version: null;
  };
  latest_decision: {
    sequence: number;
    kind: ProjectDecisionKind;
    created_by: 'user';
    decided_at_unix_ms: number;
  } | null;
}
export interface ProjectCandidateDetail {
  api_version: 'deskgraph.project-candidate-detail.v1';
  candidate: ProjectCandidate;
  user_requested_path: true;
  current_evidence: true;
  automatic_membership_created: false;
  file_actions_available: false;
}
export interface ProjectDiscovery {
  api_version: 'deskgraph.project-discovery.v1';
  scope_id: number;
  candidates: ProjectCandidateSummary[];
  evaluated_root_count: number;
  bounded_root_limit: 100;
  evaluation_complete: boolean;
  automatic_membership_created: false;
  file_actions_available: false;
}
type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);
const isCount = (value: unknown): value is number =>
  typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
const isId = (value: unknown): value is number => isCount(value) && value > 0;
const hasOnlyKeys = (value: Record<string, unknown>, keys: readonly string[]) =>
  Object.keys(value).length === keys.length &&
  Object.keys(value).every((key) => keys.includes(key));
const isState = (value: unknown): value is ProjectCandidateState =>
  value === 'suggested' || value === 'accepted' || value === 'rejected';
const isDecision = (value: unknown): value is ProjectDecisionKind =>
  value === 'accepted' || value === 'rejected';
const isSignal = (value: unknown): value is ProjectSignalKind =>
  Object.hasOwn(PROJECT_SIGNALS, value as PropertyKey);

function isValidProvenance(value: unknown): value is ProjectCandidate['suggestion']['provenance'] {
  if (
    !Array.isArray(value) ||
    value.length === 0 ||
    value.length > Object.keys(PROJECT_SIGNALS).length
  ) {
    return false;
  }
  const kinds = new Set<ProjectSignalKind>();
  return value.every((signal) => {
    if (
      !isRecord(signal) ||
      !hasOnlyKeys(signal, ['kind', 'marker_name', 'weight_basis_points']) ||
      !isSignal(signal.kind) ||
      kinds.has(signal.kind) ||
      typeof signal.marker_name !== 'string' ||
      !isCount(signal.weight_basis_points)
    ) {
      return false;
    }
    kinds.add(signal.kind);
    const expected = PROJECT_SIGNALS[signal.kind];
    return (
      signal.marker_name === expected.markerName &&
      signal.weight_basis_points === expected.weightBasisPoints
    );
  });
}

function hasValidConfidenceFormula(
  confidenceBasisPoints: unknown,
  provenance: ProjectCandidate['suggestion']['provenance'],
): boolean {
  if (!isCount(confidenceBasisPoints) || confidenceBasisPoints > 9_500) return false;
  const strongSignals = provenance.filter(({ kind }) => PROJECT_SIGNALS[kind].strong);
  if (strongSignals.length === 0) return false;
  const strongest = Math.max(
    ...strongSignals.map(({ weight_basis_points }) => weight_basis_points),
  );
  return confidenceBasisPoints === Math.min(9_500, strongest + (strongSignals.length - 1) * 500);
}

export function parseProjectCandidateSummary(value: unknown): ProjectCandidateSummary {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'api_version',
      'project_id',
      'scope_id',
      'root_folder_node_id',
      'state',
      'confidence_basis_points',
      'observed_at_unix_ms',
      'latest_decision_at_unix_ms',
    ]) ||
    value.api_version !== 'deskgraph.project-candidate-summary.v1' ||
    !isId(value.project_id) ||
    !isId(value.scope_id) ||
    !isId(value.root_folder_node_id) ||
    !isState(value.state) ||
    !isCount(value.confidence_basis_points) ||
    value.confidence_basis_points > 10_000 ||
    !isCount(value.observed_at_unix_ms) ||
    !(value.latest_decision_at_unix_ms === null || isCount(value.latest_decision_at_unix_ms))
  )
    throw new Error('Invalid project candidate summary response');
  return value as unknown as ProjectCandidateSummary;
}
export function parseProjectCandidate(value: unknown): ProjectCandidate {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'api_version',
      'project_id',
      'scope_id',
      'root_folder_node_id',
      'root_folder_location_id',
      'display_path',
      'state',
      'suggestion',
      'latest_decision',
    ]) ||
    value.api_version !== 'deskgraph.project-candidate.v1' ||
    !isId(value.project_id) ||
    !isId(value.scope_id) ||
    !isId(value.root_folder_node_id) ||
    !isId(value.root_folder_location_id) ||
    typeof value.display_path !== 'string' ||
    value.display_path.length < 1 ||
    value.display_path.length > 65_536 ||
    !isState(value.state) ||
    !isRecord(value.suggestion) ||
    (!isRecord(value.latest_decision) && value.latest_decision !== null)
  )
    throw new Error('Invalid project candidate response');
  const s = value.suggestion;
  const decision = value.latest_decision;
  const validSuggestion =
    hasOnlyKeys(s, [
      'confidence_basis_points',
      'provenance',
      'observed_at_unix_ms',
      'created_by',
      'provider_id',
      'provider_version',
      'model_version',
    ]) &&
    isValidProvenance(s.provenance) &&
    hasValidConfidenceFormula(s.confidence_basis_points, s.provenance) &&
    isCount(s.observed_at_unix_ms) &&
    s.created_by === 'system_rule' &&
    s.provider_id === 'deskgraph.folder-marker-rules' &&
    s.provider_version === '1' &&
    s.model_version === null;
  const validDecision =
    decision === null ||
    (hasOnlyKeys(decision, ['sequence', 'kind', 'created_by', 'decided_at_unix_ms']) &&
      isId(decision.sequence) &&
      isDecision(decision.kind) &&
      decision.created_by === 'user' &&
      isCount(decision.decided_at_unix_ms));
  if (!validSuggestion || !validDecision) throw new Error('Invalid project candidate response');
  return value as unknown as ProjectCandidate;
}
export function parseProjectCandidateDetail(value: unknown): ProjectCandidateDetail {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'api_version',
      'candidate',
      'user_requested_path',
      'current_evidence',
      'automatic_membership_created',
      'file_actions_available',
    ]) ||
    value.api_version !== 'deskgraph.project-candidate-detail.v1' ||
    value.user_requested_path !== true ||
    value.current_evidence !== true ||
    value.automatic_membership_created !== false ||
    value.file_actions_available !== false
  )
    throw new Error('Invalid project candidate detail response');
  parseProjectCandidate(value.candidate);
  return value as unknown as ProjectCandidateDetail;
}
export function parseProjectDiscovery(value: unknown): ProjectDiscovery {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, [
      'api_version',
      'scope_id',
      'candidates',
      'evaluated_root_count',
      'bounded_root_limit',
      'evaluation_complete',
      'automatic_membership_created',
      'file_actions_available',
    ]) ||
    value.api_version !== 'deskgraph.project-discovery.v1' ||
    !isId(value.scope_id) ||
    !Array.isArray(value.candidates) ||
    !isCount(value.evaluated_root_count) ||
    value.bounded_root_limit !== 100 ||
    value.evaluated_root_count > value.bounded_root_limit ||
    value.candidates.length > value.evaluated_root_count ||
    typeof value.evaluation_complete !== 'boolean' ||
    value.automatic_membership_created !== false ||
    value.file_actions_available !== false
  )
    throw new Error('Invalid project discovery response');
  const candidates = value.candidates.map(parseProjectCandidateSummary);
  if (
    candidates.some((candidate) => candidate.scope_id !== value.scope_id) ||
    new Set(candidates.map((candidate) => candidate.project_id)).size !== candidates.length
  )
    throw new Error('Invalid project discovery response');
  return value as unknown as ProjectDiscovery;
}
export async function discoverProjects(
  scopeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ProjectDiscovery> {
  if (!isId(scopeId)) throw new Error('Invalid project discovery scope');
  return parseProjectDiscovery(await invokeCommand(DISCOVER_PROJECTS_COMMAND, { scopeId }));
}
export async function projectScopeHasCompletedScan(
  scopeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<boolean> {
  if (!isId(scopeId)) throw new Error('Invalid project readiness scope');
  const ready = await invokeCommand(PROJECT_SCOPE_HAS_COMPLETED_SCAN_COMMAND, { scopeId });
  if (typeof ready !== 'boolean') throw new Error('Invalid project readiness response');
  return ready;
}
export async function getProjectCandidateDetail(
  scopeId: number,
  projectId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ProjectCandidateDetail> {
  if (!isId(scopeId) || !isId(projectId)) throw new Error('Invalid project candidate request');
  const detail = parseProjectCandidateDetail(
    await invokeCommand(GET_PROJECT_CANDIDATE_DETAIL_COMMAND, { scopeId, projectId }),
  );
  if (detail.candidate.scope_id !== scopeId || detail.candidate.project_id !== projectId)
    throw new Error('Project candidate detail did not match the request');
  return detail;
}
export async function decideProjectCandidate(
  scopeId: number,
  projectId: number,
  decision: ProjectDecisionKind,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<ProjectCandidateDetail> {
  if (!isId(scopeId) || !isId(projectId) || !isDecision(decision))
    throw new Error('Invalid project decision request');
  const detail = parseProjectCandidateDetail(
    await invokeCommand(DECIDE_PROJECT_CANDIDATE_COMMAND, { scopeId, projectId, decision }),
  );
  if (
    detail.candidate.scope_id !== scopeId ||
    detail.candidate.project_id !== projectId ||
    detail.candidate.latest_decision?.kind !== decision
  )
    throw new Error('Project decision did not match the request');
  return detail;
}
