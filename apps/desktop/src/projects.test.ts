import { describe, expect, it, vi } from 'vitest';
import {
  DECIDE_PROJECT_CANDIDATE_COMMAND,
  DISCOVER_PROJECTS_COMMAND,
  GET_PROJECT_CANDIDATE_DETAIL_COMMAND,
  PROJECT_SCOPE_HAS_COMPLETED_SCAN_COMMAND,
  decideProjectCandidate,
  discoverProjects,
  getProjectCandidateDetail,
  parseProjectCandidateDetail,
  parseProjectDiscovery,
  projectScopeHasCompletedScan,
} from './projects';

const summary = {
  api_version: 'deskgraph.project-candidate-summary.v1',
  project_id: 7,
  scope_id: 2,
  root_folder_node_id: 3,
  state: 'suggested',
  confidence_basis_points: 8500,
  observed_at_unix_ms: 1,
  latest_decision_at_unix_ms: null,
} as const;
const candidate = {
  api_version: 'deskgraph.project-candidate.v1',
  project_id: 7,
  scope_id: 2,
  root_folder_node_id: 3,
  root_folder_location_id: 4,
  display_path: '/private/authorized/project',
  state: 'suggested',
  suggestion: {
    confidence_basis_points: 8500,
    provenance: [{ kind: 'cargo_manifest', marker_name: 'Cargo.toml', weight_basis_points: 8500 }],
    observed_at_unix_ms: 1,
    created_by: 'system_rule',
    provider_id: 'deskgraph.folder-marker-rules',
    provider_version: '1',
    model_version: null,
  },
  latest_decision: null,
} as const;
const detail = {
  api_version: 'deskgraph.project-candidate-detail.v1',
  candidate,
  user_requested_path: true,
  current_evidence: true,
  automatic_membership_created: false,
  file_actions_available: false,
} as const;
const discovery = {
  api_version: 'deskgraph.project-discovery.v1',
  scope_id: 2,
  candidates: [summary],
  evaluated_root_count: 1,
  bounded_root_limit: 100,
  evaluation_complete: true,
  automatic_membership_created: false,
  file_actions_available: false,
} as const;

describe('Project Discovery IPC contract', () => {
  it('accepts a path-free discovery result and rejects leaked fields', () => {
    expect(parseProjectDiscovery(discovery)).toEqual(discovery);
    expect(() => parseProjectDiscovery({ ...discovery, display_path: '/private/path' })).toThrow();
    expect(() =>
      parseProjectDiscovery({
        ...discovery,
        candidates: [{ ...summary, root_path: '/private/path' }],
      }),
    ).toThrow();
    expect(() =>
      parseProjectDiscovery({ ...discovery, automatic_membership_created: true }),
    ).toThrow();
  });
  it('allows a path only in explicit current detail and rejects unsafe fields', () => {
    expect(parseProjectCandidateDetail(detail)).toEqual(detail);
    expect(() => parseProjectCandidateDetail({ ...detail, current_evidence: false })).toThrow();
    expect(() =>
      parseProjectCandidateDetail({ ...detail, candidate: { ...candidate, content: 'untrusted' } }),
    ).toThrow();
  });
  it('requires the fixed marker catalog, unique signals, and deterministic confidence formula', () => {
    const withReadme = {
      ...detail,
      candidate: {
        ...candidate,
        suggestion: {
          ...candidate.suggestion,
          confidence_basis_points: 8500,
          provenance: [
            ...candidate.suggestion.provenance,
            { kind: 'readme', marker_name: 'README', weight_basis_points: 1500 },
          ],
        },
      },
    };
    expect(parseProjectCandidateDetail(withReadme)).toEqual(withReadme);
    expect(
      parseProjectCandidateDetail({
        ...detail,
        candidate: {
          ...candidate,
          suggestion: {
            ...candidate.suggestion,
            confidence_basis_points: 9000,
            provenance: [
              ...candidate.suggestion.provenance,
              {
                kind: 'java_script_package',
                marker_name: 'package.json',
                weight_basis_points: 7500,
              },
            ],
          },
        },
      }),
    ).toMatchObject({ candidate: { suggestion: { confidence_basis_points: 9000 } } });
    expect(() =>
      parseProjectCandidateDetail({
        ...detail,
        candidate: {
          ...candidate,
          suggestion: { ...candidate.suggestion, confidence_basis_points: 8501 },
        },
      }),
    ).toThrow();
    expect(() =>
      parseProjectCandidateDetail({
        ...detail,
        candidate: {
          ...candidate,
          suggestion: {
            ...candidate.suggestion,
            provenance: [
              { kind: 'cargo_manifest', marker_name: 'Cargo.lock', weight_basis_points: 8500 },
            ],
          },
        },
      }),
    ).toThrow();
    expect(() =>
      parseProjectCandidateDetail({
        ...detail,
        candidate: {
          ...candidate,
          suggestion: {
            ...candidate.suggestion,
            provenance: [{ kind: 'readme', marker_name: 'README', weight_basis_points: 1500 }],
          },
        },
      }),
    ).toThrow();
    expect(() =>
      parseProjectCandidateDetail({
        ...detail,
        candidate: {
          ...candidate,
          suggestion: {
            ...candidate.suggestion,
            provenance: [...candidate.suggestion.provenance, ...candidate.suggestion.provenance],
          },
        },
      }),
    ).toThrow();
  });
  it('uses opaque IDs for discovery, detail, and append-only decisions', async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce(discovery)
      .mockResolvedValueOnce(detail)
      .mockResolvedValueOnce({
        ...detail,
        candidate: {
          ...candidate,
          state: 'accepted',
          latest_decision: {
            sequence: 1,
            kind: 'accepted',
            created_by: 'user',
            decided_at_unix_ms: 2,
          },
        },
      });
    await expect(discoverProjects(2, invoke)).resolves.toEqual(discovery);
    await expect(getProjectCandidateDetail(2, 7, invoke)).resolves.toEqual(detail);
    await expect(decideProjectCandidate(2, 7, 'accepted', invoke)).resolves.toMatchObject({
      candidate: { state: 'accepted' },
    });
    expect(invoke).toHaveBeenNthCalledWith(1, DISCOVER_PROJECTS_COMMAND, { scopeId: 2 });
    expect(invoke).toHaveBeenNthCalledWith(2, GET_PROJECT_CANDIDATE_DETAIL_COMMAND, {
      scopeId: 2,
      projectId: 7,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, DECIDE_PROJECT_CANDIDATE_COMMAND, {
      scopeId: 2,
      projectId: 7,
      decision: 'accepted',
    });
  });
  it('loads durable per-scope readiness instead of inferring from recent jobs', async () => {
    const invoke = vi.fn().mockResolvedValueOnce(true).mockResolvedValueOnce('true');
    await expect(projectScopeHasCompletedScan(2, invoke)).resolves.toBe(true);
    expect(invoke).toHaveBeenNthCalledWith(1, PROJECT_SCOPE_HAS_COMPLETED_SCAN_COMMAND, {
      scopeId: 2,
    });
    await expect(projectScopeHasCompletedScan(2, invoke)).rejects.toThrow(
      'Invalid project readiness response',
    );
  });
});
