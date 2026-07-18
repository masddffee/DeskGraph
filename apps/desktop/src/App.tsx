import { useEffect, useRef, useState } from 'react';

import { lifecycleLabel, loadHealthReport, type HealthReport } from './health';
import {
  createRenamePreview,
  loadRecentActionPlans,
  type ActionPlanPreview,
  type ActionPlanSummary,
  type ActionPolicyCheck,
} from './action';
import {
  activeScreenshotOcrJobIds,
  cancelScreenshotOcrJob,
  createScreenshotOcrJob,
  isScreenshotOcrCapacityBusy,
  isScreenshotCandidateDisplayPath,
  loadScreenshotOcrJobForNode,
  loadScreenshotOcrJobStatus,
  loadExtractionStats,
  loadRecentExtractions,
  mergePolledScreenshotOcrJob,
  resumeScreenshotOcrJob,
  runScreenshotOcrJob,
  type ExtractionJobProgress,
  type ExtractionStats,
} from './extraction';
import {
  addAuthorizedScope,
  createManifestScan,
  loadAuthorizedScopes,
  loadManifestStatus,
  loadRecentScanJobs,
  loadScanJobStatus,
  pauseManifestScan,
  resumeManifestScan,
  runManifestScan,
  type AuthorizedScope,
  type ManifestStats,
  type ScanJobProgress,
} from './manifest';
import {
  searchLocal,
  type SearchResponse,
  type SearchResult,
  type SearchSourceFilter,
} from './search';
import { loadRecentWatchEvents, type WatchEventProgress, type WatchEventReason } from './watch';

type ReadyState = {
  kind: 'ready';
  report: HealthReport;
  manifest: ManifestStats;
  scopes: AuthorizedScope[];
  jobs: ScanJobProgress[];
  extraction: ExtractionStats;
  extractionJobs: ExtractionJobProgress[];
  watchEvents: WatchEventProgress[];
  actionPlans: ActionPlanSummary[];
};
type ViewState = { kind: 'loading' } | ReadyState | { kind: 'error' };
type ActionState =
  | { kind: 'idle' }
  | { kind: 'working'; label: string }
  | { kind: 'success'; message: string }
  | { kind: 'error'; message: string };
type SearchState =
  | { kind: 'idle' }
  | { kind: 'working' }
  | { kind: 'ready'; response: SearchResponse }
  | { kind: 'error'; message: string };
type RenamePreviewState =
  | { kind: 'idle' }
  | { kind: 'working' }
  | { kind: 'ready'; preview: ActionPlanPreview }
  | { kind: 'error'; message: string };
type OcrActionState =
  | { kind: 'idle' }
  | { kind: 'working'; scopeId: number; nodeId: number }
  | { kind: 'success'; scopeId: number; nodeId: number; message: string }
  | { kind: 'error'; scopeId: number; nodeId: number; message: string };

interface StatusRowProps {
  label: string;
  value: string;
  tone?: 'ok' | 'quiet';
}

function StatusRow({ label, value, tone = 'quiet' }: StatusRowProps) {
  return (
    <div className="status-row">
      <span>{label}</span>
      <span className={`status-pill status-pill--${tone}`}>{value}</span>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div className="metric">
      <strong>{value.toLocaleString()}</strong>
      <span>{label}</span>
    </div>
  );
}

async function loadDashboard(): Promise<ReadyState> {
  const [report, manifest, scopes, jobs, extraction, extractionJobs, watchEvents, actionPlans] =
    await Promise.all([
      loadHealthReport(),
      loadManifestStatus(),
      loadAuthorizedScopes(),
      loadRecentScanJobs(),
      loadExtractionStats(),
      loadRecentExtractions(),
      loadRecentWatchEvents(),
      loadRecentActionPlans(),
    ]);
  return {
    kind: 'ready',
    report,
    manifest,
    scopes,
    jobs,
    extraction,
    extractionJobs,
    watchEvents,
    actionPlans,
  };
}

function replaceJob(jobs: ScanJobProgress[], job: ScanJobProgress): ScanJobProgress[] {
  return [job, ...jobs.filter((candidate) => candidate.job_id !== job.job_id)];
}

function replaceExtractionJob(
  jobs: ExtractionJobProgress[],
  job: ExtractionJobProgress,
): ExtractionJobProgress[] {
  return [job, ...jobs.filter((candidate) => candidate.job_id !== job.job_id)];
}

function screenshotOcrJobForResult(
  jobs: ExtractionJobProgress[],
  result: SearchResult,
): ExtractionJobProgress | undefined {
  return jobs.find(
    (job) =>
      job.operation === 'screenshot_ocr' &&
      job.scope_id === result.scope_id &&
      job.node_id === result.node_id,
  );
}

function scanStatusLabel(job: ScanJobProgress): string {
  if (job.status === 'running' && job.pause_requested) return 'Pausing safely…';
  if (job.status === 'running') return 'Scanning metadata…';
  if (job.status === 'paused') return 'Paused';
  if (job.status === 'interrupted') return 'Interrupted safely';
  if (job.status === 'completed') return 'Completed';
  return 'Stopped with an error';
}

function extractionStatusLabel(job: ExtractionJobProgress): string {
  if (job.status === 'queued') return 'Waiting to start';
  if (job.status === 'running' && job.cancel_requested) return 'Stopping safely…';
  if (job.status === 'running' && job.operation === 'screenshot_ocr') {
    return 'Reading screenshot text locally…';
  }
  if (job.status === 'running') return 'Extracting bounded text…';
  if (job.status === 'completed' && job.operation === 'screenshot_ocr') {
    return 'Screenshot text indexed locally';
  }
  if (job.status === 'completed') return 'Completed';
  if (job.status === 'cancelled') return 'Cancelled safely';
  if (job.status === 'interrupted') return 'Interrupted safely';
  if (job.operation === 'screenshot_ocr') return 'Screenshot OCR unavailable or skipped safely';
  return 'File skipped safely';
}

function searchExplanation(result: SearchResult): string {
  if (result.explanation === 'exact_filename_and_extracted_text') {
    return 'Exact filename + extracted text';
  }
  if (result.explanation === 'exact_filename') return 'Exact filename';
  if (result.explanation === 'path_and_extracted_text_substring') {
    return 'Path + extracted text';
  }
  if (result.explanation === 'path_substring') return 'Filename or path';
  return 'Extracted text';
}

function utcDateToUnixSeconds(value: string): number | null {
  if (!value) return null;
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
  const milliseconds = Date.parse(`${value}T00:00:00Z`);
  if (!Number.isFinite(milliseconds)) return null;
  if (new Date(milliseconds).toISOString().slice(0, 10) !== value) return null;
  return Math.floor(milliseconds / 1000);
}

function activeSearchFilters(response: SearchResponse): string {
  const labels: string[] = [];
  if (response.filters.scope_id !== null) {
    labels.push(`scope ${response.filters.scope_id}`);
  }
  if (response.filters.source === 'metadata_path') labels.push('paths only');
  if (response.filters.source === 'extracted_text') labels.push('extracted text only');
  if (response.filters.extension) labels.push(`.${response.filters.extension}`);
  if (response.filters.modified_since_unix_seconds !== null) {
    labels.push(
      `since ${new Date(response.filters.modified_since_unix_seconds * 1000)
        .toISOString()
        .slice(0, 10)} UTC`,
    );
  }
  if (response.filters.modified_before_unix_seconds !== null) {
    labels.push(
      `before ${new Date(response.filters.modified_before_unix_seconds * 1000)
        .toISOString()
        .slice(0, 10)} UTC`,
    );
  }
  return labels.length > 0 ? labels.join(' · ') : 'all authorized local sources';
}

function watchReasonLabel(reason: WatchEventReason | null): string {
  if (reason === 'temporary_download') return 'Temporary download ignored';
  if (reason === 'hidden_entry') return 'Hidden entry ignored';
  if (reason === 'unsupported_entry') return 'Unsupported entry ignored';
  if (reason === 'source_unavailable') return 'Source unavailable';
  if (reason === 'reconcile_failed') return 'Reconciliation failed safely';
  return 'No failure';
}

function watchStatusLabel(event: WatchEventProgress): string {
  if (event.status === 'stabilizing') return 'Waiting for a stable snapshot';
  if (event.status === 'reconciling') return 'Atomic manifest reconciliation';
  if (event.status === 'completed') return 'Reconciled';
  return watchReasonLabel(event.reason);
}

function actionPolicyCheckLabel(check: ActionPolicyCheck): string {
  if (check === 'explicit_authorized_scope') return 'Inside the selected authorized folder';
  if (check === 'present_manifest_file') return 'Current scanned file';
  if (check === 'canonical_source_contained') return 'Canonical source stays in scope';
  if (check === 'source_identity_matches') return 'Platform identity matches the manifest';
  if (check === 'read_only_handle_identity_matches') return 'Read-only open handle matches';
  if (check === 'portable_single_component_name') return 'Portable one-part filename';
  if (check === 'same_canonical_parent') return 'Same canonical parent folder';
  if (check === 'destination_contained') return 'Destination stays in scope';
  return 'Destination is available';
}

export default function App() {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<ViewState>({ kind: 'loading' });
  const [scopePath, setScopePath] = useState('');
  const [action, setAction] = useState<ActionState>({ kind: 'idle' });
  const [searchQuery, setSearchQuery] = useState('');
  const [searchScopeId, setSearchScopeId] = useState<number | null>(null);
  const [searchSource, setSearchSource] = useState<SearchSourceFilter>('all');
  const [searchExtension, setSearchExtension] = useState('');
  const [searchModifiedSince, setSearchModifiedSince] = useState('');
  const [searchModifiedBefore, setSearchModifiedBefore] = useState('');
  const [searchState, setSearchState] = useState<SearchState>({ kind: 'idle' });
  const [renameScopeId, setRenameScopeId] = useState<number | null>(null);
  const [renameSourcePath, setRenameSourcePath] = useState('');
  const [renameNewName, setRenameNewName] = useState('');
  const [renameState, setRenameState] = useState<RenamePreviewState>({ kind: 'idle' });
  const [ocrAction, setOcrAction] = useState<OcrActionState>({ kind: 'idle' });
  const ocrRequestInFlight = useRef(new Set<string>());
  const runningJobIds =
    state.kind === 'ready'
      ? state.jobs.filter((job) => job.status === 'running').map((job) => job.job_id)
      : [];
  const runningJobKey = runningJobIds.join(',');
  const activeExtractionJobIds =
    state.kind === 'ready' ? activeScreenshotOcrJobIds(state.extractionJobs) : [];
  const activeExtractionJobKey = activeExtractionJobIds.join(',');

  useEffect(() => {
    let active = true;
    setState({ kind: 'loading' });

    void loadDashboard()
      .then((dashboard) => {
        if (active) setState(dashboard);
      })
      .catch(() => {
        if (active) setState({ kind: 'error' });
      });

    return () => {
      active = false;
    };
  }, [attempt]);

  useEffect(() => {
    if (!runningJobKey) return;
    let active = true;

    const poll = async () => {
      try {
        const jobs = await Promise.all(runningJobIds.map((jobId) => loadScanJobStatus(jobId)));
        if (!active) return;
        setState((current) => {
          if (current.kind !== 'ready') return current;
          return {
            ...current,
            jobs: jobs.reduce(replaceJob, current.jobs),
          };
        });
      } catch {
        // The foreground runner reports a validated error state if polling cannot continue.
      }
    };

    const timer = window.setInterval(() => void poll(), 300);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [runningJobKey]);

  useEffect(() => {
    if (!activeExtractionJobKey) return;
    let active = true;

    const poll = async () => {
      try {
        const jobs = await Promise.all(
          activeExtractionJobIds.map((jobId) => loadScreenshotOcrJobStatus(jobId)),
        );
        if (!active) return;
        setState((current) => {
          if (current.kind !== 'ready') return current;
          return {
            ...current,
            extractionJobs: jobs.reduce(mergePolledScreenshotOcrJob, current.extractionJobs),
          };
        });
      } catch {
        // The foreground runner publishes a validated terminal state or a generic UI error.
      }
    };

    const timer = window.setInterval(() => void poll(), 300);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [activeExtractionJobKey]);

  function updateJob(job: ScanJobProgress) {
    setState((current) =>
      current.kind === 'ready' ? { ...current, jobs: replaceJob(current.jobs, job) } : current,
    );
  }

  function updateExtractionJob(job: ExtractionJobProgress) {
    setState((current) =>
      current.kind === 'ready'
        ? {
            ...current,
            extractionJobs: replaceExtractionJob(current.extractionJobs, job),
          }
        : current,
    );
  }

  async function refreshManifest() {
    const [manifest, scopes, jobs, extraction, extractionJobs, watchEvents, actionPlans] =
      await Promise.all([
        loadManifestStatus(),
        loadAuthorizedScopes(),
        loadRecentScanJobs(),
        loadExtractionStats(),
        loadRecentExtractions(),
        loadRecentWatchEvents(),
        loadRecentActionPlans(),
      ]);
    setState((current) =>
      current.kind === 'ready'
        ? {
            ...current,
            manifest,
            scopes,
            jobs,
            extraction,
            extractionJobs,
            watchEvents,
            actionPlans,
          }
        : current,
    );
  }

  async function authorizeRequestedScope() {
    const requestedPath = scopePath.trim();
    if (!requestedPath) {
      setAction({ kind: 'error', message: 'Enter an existing folder path first.' });
      return;
    }
    setAction({ kind: 'working', label: 'Validating the folder boundary…' });
    try {
      await addAuthorizedScope(requestedPath);
      await refreshManifest();
      setScopePath('');
      setAction({
        kind: 'success',
        message: 'Folder authorized. Nothing was scanned until you choose Scan metadata.',
      });
    } catch {
      setAction({
        kind: 'error',
        message: 'The folder could not be authorized. Check that it exists and is not protected.',
      });
    }
  }

  async function runJob(job: ScanJobProgress) {
    try {
      setAction({ kind: 'working', label: 'Reading metadata inside the authorized folder…' });
      const progress = await runManifestScan(job.job_id);
      updateJob(progress);
      await refreshManifest();
      if (progress.status === 'completed') {
        setAction({
          kind: 'success',
          message: `Scan complete: ${progress.discovered_files} files and ${progress.discovered_folders} folders.`,
        });
      } else if (progress.status === 'paused') {
        setAction({
          kind: 'success',
          message: `Scan paused after ${progress.processed_entries} of ${progress.queued_entries} discovered entries.`,
        });
      } else {
        setAction({
          kind: 'error',
          message:
            'The scan was interrupted safely. Resume it after checking the authorized folder.',
        });
      }
    } catch {
      await refreshManifest().catch(() => undefined);
      setAction({
        kind: 'error',
        message:
          'The metadata scan stopped safely. Existing manifest data was not partially replaced.',
      });
    }
  }

  async function scan(scope: AuthorizedScope) {
    setAction({ kind: 'working', label: 'Creating a durable local scan job…' });
    try {
      const job = await createManifestScan(scope.id);
      updateJob(job);
      await runJob(job);
    } catch {
      await refreshManifest().catch(() => undefined);
      setAction({
        kind: 'error',
        message: 'A new scan could not start. Resume the existing job if this folder has one.',
      });
    }
  }

  async function pause(job: ScanJobProgress) {
    setAction({ kind: 'working', label: 'Waiting for the current metadata entry to finish…' });
    try {
      const progress = await pauseManifestScan(job.job_id);
      updateJob(progress);
      if (progress.status === 'paused') {
        setAction({ kind: 'success', message: 'Scan paused. Durable progress is safe to resume.' });
      }
    } catch {
      setAction({ kind: 'error', message: 'The pause request could not be recorded safely.' });
    }
  }

  async function resume(job: ScanJobProgress) {
    setAction({ kind: 'working', label: 'Revalidating the authorized folder boundary…' });
    try {
      const progress = await resumeManifestScan(job.job_id);
      updateJob(progress);
      await runJob(progress);
    } catch {
      await refreshManifest().catch(() => undefined);
      setAction({
        kind: 'error',
        message: 'Resume was denied because the job or authorized folder is no longer valid.',
      });
    }
  }

  async function submitSearch() {
    const query = searchQuery.trim();
    if ([...query].length < 3) {
      setSearchState({
        kind: 'error',
        message: 'Enter at least 3 characters to keep local search bounded.',
      });
      return;
    }
    const extension = searchExtension.trim();
    if (extension && !/^\.?[a-z0-9]{1,16}$/i.test(extension)) {
      setSearchState({
        kind: 'error',
        message: 'File type must be a 1–16 character extension such as md, pdf, or docx.',
      });
      return;
    }
    const modifiedSinceUnixSeconds = utcDateToUnixSeconds(searchModifiedSince);
    const modifiedBeforeUnixSeconds = utcDateToUnixSeconds(searchModifiedBefore);
    if (
      (searchModifiedSince && modifiedSinceUnixSeconds === null) ||
      (searchModifiedBefore && modifiedBeforeUnixSeconds === null) ||
      (modifiedSinceUnixSeconds !== null &&
        modifiedBeforeUnixSeconds !== null &&
        modifiedSinceUnixSeconds >= modifiedBeforeUnixSeconds)
    ) {
      setSearchState({
        kind: 'error',
        message: 'Choose a valid UTC date range where “Modified since” is before “Before”.',
      });
      return;
    }
    setSearchState({ kind: 'working' });
    try {
      const response = await searchLocal(query, {
        scopeId: searchScopeId,
        source: searchSource,
        extension: extension || null,
        modifiedSinceUnixSeconds,
        modifiedBeforeUnixSeconds,
      });
      setSearchState({ kind: 'ready', response });
      const ocrJobs = await Promise.all(
        response.results
          .filter((result) => isScreenshotCandidateDisplayPath(result.display_path))
          .map(async (result) => {
            try {
              return await loadScreenshotOcrJobForNode(result.scope_id, result.node_id);
            } catch {
              return null;
            }
          }),
      );
      setState((current) => {
        if (current.kind !== 'ready') return current;
        return {
          ...current,
          extractionJobs: ocrJobs
            .filter((job): job is ExtractionJobProgress => job !== null)
            .reduce(replaceExtractionJob, current.extractionJobs),
        };
      });
    } catch {
      setSearchState({
        kind: 'error',
        message: 'Search stopped safely. Try a shorter query or refresh the local manifest.',
      });
    }
  }

  async function runScreenshotOcr(job: ExtractionJobProgress) {
    setOcrAction({ kind: 'working', scopeId: job.scope_id, nodeId: job.node_id });
    try {
      const progress = await runScreenshotOcrJob(job.job_id);
      updateExtractionJob(progress);
      setOcrFeedbackFromProgress(progress);
      await refreshManifest().catch(() => undefined);
    } catch (error) {
      await refreshManifest().catch(() => undefined);
      if (isScreenshotOcrCapacityBusy(error)) {
        setOcrAction({
          kind: 'error',
          scopeId: job.scope_id,
          nodeId: job.node_id,
          message:
            'Another local OCR is still finishing. This job remains queued; retry it safely.',
        });
        return;
      }
      setOcrAction({
        kind: 'error',
        scopeId: job.scope_id,
        nodeId: job.node_id,
        message:
          'Screenshot OCR stopped safely. The local provider may be unavailable on this computer.',
      });
    }
  }

  function setOcrFeedbackFromProgress(progress: ExtractionJobProgress) {
    if (progress.status === 'completed') {
      setOcrAction({
        kind: 'success',
        scopeId: progress.scope_id,
        nodeId: progress.node_id,
        message: 'Screenshot text was indexed locally. Search again to find its contents.',
      });
      return;
    }
    if (progress.status === 'cancelled') {
      setOcrAction({
        kind: 'success',
        scopeId: progress.scope_id,
        nodeId: progress.node_id,
        message: 'Screenshot OCR was cancelled safely. No partial text was published.',
      });
      return;
    }
    if (progress.status === 'interrupted') {
      setOcrAction({
        kind: 'error',
        scopeId: progress.scope_id,
        nodeId: progress.node_id,
        message: 'Screenshot OCR was interrupted safely. Resume it to continue locally.',
      });
      return;
    }
    if (progress.status === 'failed') {
      setOcrAction({
        kind: 'error',
        scopeId: progress.scope_id,
        nodeId: progress.node_id,
        message: 'Screenshot OCR stopped safely. The previous complete local index was preserved.',
      });
    }
  }

  async function startScreenshotOcr(result: SearchResult) {
    const requestKey = `${result.scope_id}:${result.node_id}`;
    if (activeExtractionJobIds.length > 0 || ocrRequestInFlight.current.size > 0) return;
    ocrRequestInFlight.current.add(requestKey);
    setOcrAction({ kind: 'working', scopeId: result.scope_id, nodeId: result.node_id });
    try {
      const job = await createScreenshotOcrJob(result.scope_id, result.node_id);
      updateExtractionJob(job);
      await runScreenshotOcr(job);
    } catch {
      await refreshManifest().catch(() => undefined);
      setOcrAction({
        kind: 'error',
        scopeId: result.scope_id,
        nodeId: result.node_id,
        message:
          'OCR was denied safely. Rescan the file if it changed and confirm it is a supported screenshot.',
      });
    } finally {
      ocrRequestInFlight.current.delete(requestKey);
    }
  }

  async function resumeScreenshotOcr(job: ExtractionJobProgress) {
    const requestKey = `${job.scope_id}:${job.node_id}`;
    if (activeExtractionJobIds.length > 0 || ocrRequestInFlight.current.size > 0) return;
    ocrRequestInFlight.current.add(requestKey);
    setOcrAction({ kind: 'working', scopeId: job.scope_id, nodeId: job.node_id });
    try {
      const queued = await resumeScreenshotOcrJob(job.job_id);
      updateExtractionJob(queued);
      await runScreenshotOcr(queued);
    } catch {
      await refreshManifest().catch(() => undefined);
      setOcrAction({
        kind: 'error',
        scopeId: job.scope_id,
        nodeId: job.node_id,
        message: 'Resume was denied safely. Refresh the local manifest before trying again.',
      });
    } finally {
      ocrRequestInFlight.current.delete(requestKey);
    }
  }

  async function retryQueuedScreenshotOcr(job: ExtractionJobProgress) {
    const requestKey = `${job.scope_id}:${job.node_id}`;
    if (ocrRequestInFlight.current.size > 0) return;
    ocrRequestInFlight.current.add(requestKey);
    try {
      await runScreenshotOcr(job);
    } finally {
      ocrRequestInFlight.current.delete(requestKey);
    }
  }

  async function cancelScreenshotOcr(job: ExtractionJobProgress) {
    try {
      const progress = await cancelScreenshotOcrJob(job.job_id);
      updateExtractionJob(progress);
      if (progress.status === 'cancelled') {
        setOcrFeedbackFromProgress(progress);
        await refreshManifest().catch(() => undefined);
      }
    } catch {
      try {
        const progress = await loadScreenshotOcrJobStatus(job.job_id);
        updateExtractionJob(progress);
        if (progress.status !== 'queued' && progress.status !== 'running') {
          setOcrFeedbackFromProgress(progress);
          await refreshManifest().catch(() => undefined);
          return;
        }
      } catch {
        // Fall through to a path-free generic error.
      }
      setOcrAction({
        kind: 'error',
        scopeId: job.scope_id,
        nodeId: job.node_id,
        message: 'The cancellation request could not be recorded safely.',
      });
    }
  }

  async function submitRenamePreview() {
    const sourcePath = renameSourcePath.trim();
    if (renameScopeId === null) {
      setRenameState({ kind: 'error', message: 'Choose the authorized folder first.' });
      return;
    }
    if (!sourcePath || !renameNewName) {
      setRenameState({
        kind: 'error',
        message: 'Enter the current absolute file path and one proposed filename.',
      });
      return;
    }
    setRenameState({ kind: 'working' });
    try {
      const preview = await createRenamePreview(renameScopeId, sourcePath, renameNewName);
      const actionPlans = await loadRecentActionPlans();
      setState((current) => (current.kind === 'ready' ? { ...current, actionPlans } : current));
      setRenameState({ kind: 'ready', preview });
    } catch {
      setRenameState({
        kind: 'error',
        message:
          'Preview denied safely. Rescan a changed file, verify the authorized folder, and choose an unused portable filename.',
      });
    }
  }

  return (
    <main className="app-shell">
      <header className="hero">
        <div>
          <p className="eyebrow">DeskGraph · M2 Local Context</p>
          <h1>Graphify your computer.</h1>
          <p className="hero-copy">
            Authorize one local folder at a time, build its metadata manifest, and keep bounded text
            extraction on this computer.
          </p>
          <p className="hero-copy hero-copy--zh">
            一次明確授權一個本機資料夾；metadata 與受限文字抽取都留在本機，不上傳路徑或內容。
          </p>
        </div>
        <span className="release-badge">PRE-RELEASE</span>
      </header>

      {state.kind === 'loading' ? (
        <section className="state-card" aria-live="polite" aria-busy="true">
          <span className="loader" aria-hidden="true" />
          <div>
            <h2>Opening the local manifest</h2>
            <p>No authorized folder is scanned automatically.</p>
          </div>
        </section>
      ) : null}

      {state.kind === 'error' ? (
        <section className="state-card state-card--error" role="alert">
          <div>
            <h2>Local manifest unavailable</h2>
            <p>The backend returned no validated status. Raw local errors and paths are hidden.</p>
          </div>
          <button type="button" onClick={() => setAttempt((value) => value + 1)}>
            Retry
          </button>
        </section>
      ) : null}

      {state.kind === 'ready' ? (
        <div className="dashboard" aria-live="polite">
          <section className="panel" aria-labelledby="runtime-title">
            <div className="panel-heading">
              <div>
                <p className="panel-kicker">Local runtime</p>
                <h2 id="runtime-title">Manifest is ready</h2>
              </div>
              <span className="connected-indicator">Local only</span>
            </div>
            <div className="status-list">
              <StatusRow
                label="Platform"
                value={`${state.report.platform.os} · ${state.report.platform.architecture}`}
                tone="ok"
              />
              <StatusRow label="SQLite manifest" value="Ready" tone="ok" />
              <StatusRow
                label="Optional local LLM"
                value={lifecycleLabel(state.report.providers.local_llm.state)}
              />
              <StatusRow label="Network required" value="No" tone="ok" />
            </div>
          </section>

          <section className="panel panel--privacy" aria-labelledby="manifest-title">
            <p className="panel-kicker">Current graph</p>
            <h2 id="manifest-title">
              {state.manifest.completed_scan_count === 0
                ? 'Nothing indexed yet'
                : 'Metadata indexed'}
            </h2>
            <div className="metrics">
              <Metric label="Files" value={state.manifest.file_count} />
              <Metric label="Folders" value={state.manifest.folder_count} />
              <Metric label="Locations" value={state.manifest.active_location_count} />
              <Metric label="Scan issues" value={state.manifest.issue_count} />
            </div>
          </section>

          <section className="panel panel--search" aria-labelledby="search-title">
            <div className="panel-heading panel-heading--wrap">
              <div>
                <p className="panel-kicker">Deterministic local search</p>
                <h2 id="search-title">Find filenames and extracted text</h2>
                <p>
                  Traditional Chinese and English queries stay inside SQLite. Embeddings are off;
                  every result says which local field matched.
                </p>
              </div>
              <span className="connected-indicator">Lexical · offline</span>
            </div>
            <form
              className="search-form"
              onSubmit={(event) => {
                event.preventDefault();
                void submitSearch();
              }}
            >
              <label htmlFor="search-query">Search local context</label>
              <div className="search-form-row">
                <input
                  id="search-query"
                  type="search"
                  value={searchQuery}
                  onChange={(event) => setSearchQuery(event.target.value)}
                  placeholder="專案脈絡 or project context"
                  autoComplete="off"
                  spellCheck="false"
                  maxLength={256}
                />
                <select
                  aria-label="Search folder scope"
                  value={searchScopeId ?? ''}
                  onChange={(event) =>
                    setSearchScopeId(event.target.value ? Number(event.target.value) : null)
                  }
                >
                  <option value="">All authorized folders</option>
                  {state.scopes.map((scope) => (
                    <option key={scope.id} value={scope.id}>
                      Authorized scope {scope.id}
                    </option>
                  ))}
                </select>
                <button type="submit" disabled={searchState.kind === 'working'}>
                  {searchState.kind === 'working' ? 'Searching…' : 'Search'}
                </button>
              </div>
              <div className="search-filter-grid" aria-label="Bounded local search filters">
                <label>
                  Match source
                  <select
                    value={searchSource}
                    onChange={(event) => setSearchSource(event.target.value as SearchSourceFilter)}
                  >
                    <option value="all">Paths + extracted text</option>
                    <option value="metadata_path">Filenames and paths only</option>
                    <option value="extracted_text">Extracted text only</option>
                  </select>
                </label>
                <label>
                  File type
                  <input
                    value={searchExtension}
                    onChange={(event) => setSearchExtension(event.target.value)}
                    placeholder="md or pdf"
                    maxLength={17}
                    autoComplete="off"
                    spellCheck="false"
                  />
                </label>
                <label>
                  Modified since (UTC)
                  <input
                    type="date"
                    value={searchModifiedSince}
                    onChange={(event) => setSearchModifiedSince(event.target.value)}
                  />
                </label>
                <label>
                  Before (UTC, exclusive)
                  <input
                    type="date"
                    value={searchModifiedBefore}
                    onChange={(event) => setSearchModifiedBefore(event.target.value)}
                  />
                </label>
              </div>
            </form>

            {searchState.kind === 'error' ? (
              <p className="search-message search-message--error" role="alert">
                {searchState.message}
              </p>
            ) : null}
            {searchState.kind === 'ready' && searchState.response.results.length === 0 ? (
              <p className="search-message" role="status">
                No current path or active extracted text matched “{searchState.response.query}”.
              </p>
            ) : null}
            {searchState.kind === 'ready' && searchState.response.results.length > 0 ? (
              <div className="search-summary" role="status">
                <span>
                  {searchState.response.result_count.toLocaleString()} results ·{' '}
                  {searchState.response.elapsed_ms.toLocaleString()} ms
                </span>
                <span>{activeSearchFilters(searchState.response)}</span>
              </div>
            ) : null}
            {searchState.kind === 'ready' && searchState.response.results.length > 0 ? (
              <ol className="search-results">
                {searchState.response.results.map((result) => {
                  const ocrJob = screenshotOcrJobForResult(state.extractionJobs, result);
                  const ocrIsRunning = ocrJob?.status === 'running';
                  const ocrIsQueued = ocrJob?.status === 'queued';
                  const anotherOcrIsRunning = state.extractionJobs.some(
                    (job) =>
                      job.operation === 'screenshot_ocr' &&
                      job.status === 'running' &&
                      job.job_id !== ocrJob?.job_id,
                  );
                  const feedbackMatches =
                    ocrAction.kind !== 'idle' &&
                    ocrAction.scopeId === result.scope_id &&
                    ocrAction.nodeId === result.node_id;
                  return (
                    <li key={`${result.node_id}:${result.location_id}`}>
                      <div className="search-result-heading">
                        <span className="search-rank">#{result.lexical_rank}</span>
                        <strong>{searchExplanation(result)}</strong>
                      </div>
                      <code>{result.display_path}</code>
                      {isScreenshotCandidateDisplayPath(result.display_path) ? (
                        <div
                          className="search-result-action"
                          aria-label="Local screenshot OCR controls"
                        >
                          <div className="search-result-action-row">
                            <div>
                              <strong>
                                {ocrJob
                                  ? extractionStatusLabel(ocrJob)
                                  : 'Screenshot text has not been read'}
                              </strong>
                              <span>
                                Only this already-scanned screenshot is revalidated and read on this
                                computer.
                              </span>
                            </div>
                            {ocrIsRunning && ocrJob ? (
                              <button
                                type="button"
                                disabled={ocrJob.cancel_requested}
                                onClick={() => void cancelScreenshotOcr(ocrJob)}
                              >
                                {ocrJob.cancel_requested ? 'Stopping safely…' : 'Cancel OCR'}
                              </button>
                            ) : ocrIsQueued && ocrJob ? (
                              <div className="search-result-action-buttons">
                                <button
                                  type="button"
                                  disabled={
                                    anotherOcrIsRunning ||
                                    (feedbackMatches && ocrAction.kind === 'working')
                                  }
                                  onClick={() => void retryQueuedScreenshotOcr(ocrJob)}
                                >
                                  Retry queued OCR
                                </button>
                                <button
                                  type="button"
                                  disabled={ocrJob.cancel_requested}
                                  onClick={() => void cancelScreenshotOcr(ocrJob)}
                                >
                                  Cancel OCR
                                </button>
                              </div>
                            ) : ocrJob?.status === 'interrupted' ? (
                              <button
                                type="button"
                                disabled={
                                  activeExtractionJobIds.length > 0 ||
                                  (feedbackMatches && ocrAction.kind === 'working')
                                }
                                onClick={() => void resumeScreenshotOcr(ocrJob)}
                              >
                                Resume screenshot OCR
                              </button>
                            ) : (
                              <button
                                type="button"
                                disabled={
                                  activeExtractionJobIds.length > 0 ||
                                  (feedbackMatches && ocrAction.kind === 'working')
                                }
                                onClick={() => void startScreenshotOcr(result)}
                              >
                                {ocrJob ? 'Read screenshot again' : 'Read screenshot text locally'}
                              </button>
                            )}
                          </div>
                          {feedbackMatches && ocrAction.kind === 'error' ? (
                            <p className="ocr-feedback ocr-feedback--error" role="alert">
                              {ocrAction.message}
                            </p>
                          ) : null}
                          {feedbackMatches && ocrAction.kind === 'success' ? (
                            <p className="ocr-feedback" role="status">
                              {ocrAction.message}
                            </p>
                          ) : null}
                        </div>
                      ) : null}
                      {result.snippet ? (
                        <p className="search-snippet">
                          <span>Untrusted local text</span>
                          {result.snippet}
                        </p>
                      ) : null}
                    </li>
                  );
                })}
              </ol>
            ) : null}
          </section>

          <section className="panel panel--actions" aria-labelledby="actions-title">
            <div className="panel-heading panel-heading--wrap">
              <div>
                <p className="panel-kicker">Safe organization preview</p>
                <h2 id="actions-title">Review a rename without changing the file</h2>
                <p>
                  DeskGraph revalidates the selected scope, current manifest snapshot, file
                  identity, read-only open handle, proposed name, and destination before it journals
                  a preview.
                </p>
              </div>
              <span className="connected-indicator connected-indicator--pending">
                Preview only · no execute
              </span>
            </div>

            <form
              className="rename-form"
              onSubmit={(event) => {
                event.preventDefault();
                void submitRenamePreview();
              }}
            >
              <label>
                Authorized folder
                <select
                  value={renameScopeId ?? ''}
                  onChange={(event) =>
                    setRenameScopeId(event.target.value ? Number(event.target.value) : null)
                  }
                >
                  <option value="">Choose a scanned folder</option>
                  {state.scopes.map((scope) => (
                    <option key={scope.id} value={scope.id}>
                      Scope {scope.id} · {scope.display_path}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Current absolute file path
                <input
                  value={renameSourcePath}
                  onChange={(event) => setRenameSourcePath(event.target.value)}
                  placeholder="/authorized/folder/draft.md"
                  autoComplete="off"
                  spellCheck="false"
                />
              </label>
              <label>
                Proposed filename only
                <input
                  value={renameNewName}
                  onChange={(event) => setRenameNewName(event.target.value)}
                  placeholder="final.md"
                  autoComplete="off"
                  spellCheck="false"
                  maxLength={255}
                />
              </label>
              <button type="submit" disabled={renameState.kind === 'working'}>
                {renameState.kind === 'working' ? 'Validating safely…' : 'Create durable preview'}
              </button>
            </form>

            {renameState.kind === 'error' ? (
              <p className="action-message action-message--error" role="alert">
                {renameState.message}
              </p>
            ) : null}

            {renameState.kind === 'ready' ? (
              <div className="rename-preview" role="status" aria-label="Validated rename preview">
                <div className="rename-preview-heading">
                  <div>
                    <strong>Plan {renameState.preview.plan_id} · validated preview</strong>
                    <span>
                      {renameState.preview.execution_strategy === 'case_only_staged'
                        ? 'Case-only rename requires a staged future executor.'
                        : 'Direct rename strategy recorded for a future executor.'}
                    </span>
                  </div>
                  <span className="status-pill status-pill--ok">No file changed</span>
                </div>
                <div className="rename-paths">
                  <div>
                    <span>Before</span>
                    <code>{renameState.preview.source_path}</code>
                  </div>
                  <div>
                    <span>After</span>
                    <code>{renameState.preview.destination_path}</code>
                  </div>
                </div>
                <ul className="policy-checks" aria-label="Passed policy checks">
                  {renameState.preview.policy.checks.map((check) => (
                    <li key={check}>{actionPolicyCheckLabel(check)}</li>
                  ))}
                </ul>
                <p className="content-empty">
                  This plan is journaled but cannot execute. Recovery and Undo do not exist yet, so
                  DeskGraph exposes no action button.
                </p>
              </div>
            ) : null}

            <div className="action-history-heading">
              <strong>Recent path-free preview history</strong>
              <span>{state.actionPlans.length} plans</span>
            </div>
            {state.actionPlans.length === 0 ? (
              <p className="content-empty">
                No organization preview has been journaled. Files cannot be changed from this app.
              </p>
            ) : (
              <ol className="action-plan-list">
                {state.actionPlans.slice(0, 5).map((plan) => (
                  <li key={plan.plan_id}>
                    <div>
                      <strong>Rename preview {plan.plan_id}</strong>
                      <span>
                        Scope {plan.scope_id} · node {plan.node_id}
                      </span>
                    </div>
                    <span>
                      {plan.execution_strategy === 'case_only_staged'
                        ? 'Case-only staged'
                        : 'Direct · previewed'}
                    </span>
                  </li>
                ))}
              </ol>
            )}
          </section>

          <section className="panel panel--watch" aria-labelledby="watch-title">
            <div className="panel-heading panel-heading--wrap">
              <div>
                <p className="panel-kicker">Durable watch reconciliation</p>
                <h2 id="watch-title">Stable hints, atomic manifest updates</h2>
                <p>
                  The local core can debounce path-free event states, reject temporary downloads,
                  and resume reconciliation after restart. The native OS event adapter and automatic
                  content re-indexing are not connected yet.
                </p>
              </div>
              <span className="connected-indicator connected-indicator--pending">
                Core ready · adapter pending
              </span>
            </div>
            <div className="metrics metrics--content">
              <Metric label="Recent events" value={state.watchEvents.length} />
              <Metric
                label="Observed hints"
                value={state.watchEvents.reduce(
                  (total, event) => total + event.observation_count,
                  0,
                )}
              />
              <Metric
                label="Reconciled"
                value={state.watchEvents.filter((event) => event.status === 'completed').length}
              />
              <Metric
                label="Needs attention"
                value={
                  state.watchEvents.filter(
                    (event) => event.status === 'failed' || event.status === 'reconciling',
                  ).length
                }
              />
            </div>
            {state.watchEvents.length === 0 ? (
              <p className="content-empty">
                No event source is enabled. Files are still updated only by an explicit scan.
              </p>
            ) : (
              <ol className="watch-event-list">
                {state.watchEvents.slice(0, 3).map((event) => (
                  <li key={event.event_id}>
                    <div>
                      <strong>{watchStatusLabel(event)}</strong>
                      <span>
                        Event {event.event_id} · scope {event.scope_id} ·{' '}
                        {event.observation_count.toLocaleString()} coalesced hint
                        {event.observation_count === 1 ? '' : 's'}
                      </span>
                    </div>
                    <span>{event.scan_job_id ? `Scan ${event.scan_job_id}` : 'No scan yet'}</span>
                  </li>
                ))}
              </ol>
            )}
          </section>

          <section className="panel panel--content" aria-labelledby="content-title">
            <div className="panel-heading panel-heading--wrap">
              <div>
                <p className="panel-kicker">Bounded local content</p>
                <h2 id="content-title">
                  {state.extraction.extracted_file_count === 0
                    ? 'No file content extracted yet'
                    : 'Local text is ready'}
                </h2>
                <p>
                  Only already-scanned supported documents and explicitly selected screenshots are
                  eligible. Every source is revalidated, output is size-limited, and a failed job
                  cannot replace the last complete text.
                </p>
              </div>
              <span className="connected-indicator">Never uploaded</span>
            </div>
            <div className="metrics metrics--content">
              <Metric label="Files with text" value={state.extraction.extracted_file_count} />
              <Metric label="Active chunks" value={state.extraction.active_chunk_count} />
              <Metric label="Completed jobs" value={state.extraction.completed_job_count} />
              <Metric
                label="Skipped or cancelled"
                value={state.extraction.failed_job_count + state.extraction.cancelled_job_count}
              />
            </div>
            {state.extractionJobs[0] ? (
              <div className="extraction-progress" role="status">
                <span>
                  Latest{' '}
                  {state.extractionJobs[0].operation === 'screenshot_ocr'
                    ? 'Screenshot OCR'
                    : 'content'}{' '}
                  job {state.extractionJobs[0].job_id}
                </span>
                <strong>{extractionStatusLabel(state.extractionJobs[0])}</strong>
                <span>
                  {state.extractionJobs[0].chunk_count.toLocaleString()} chunks ·{' '}
                  {state.extractionJobs[0].output_bytes.toLocaleString()} bytes
                </span>
              </div>
            ) : (
              <p className="content-empty">
                Extraction is opt-in. Authorizing or scanning a folder never reads file contents.
              </p>
            )}
          </section>

          <section className="panel panel--scopes" aria-labelledby="scopes-title">
            <div className="panel-heading panel-heading--wrap">
              <div>
                <p className="panel-kicker">Explicit authorization</p>
                <h2 id="scopes-title">Folders DeskGraph may inspect</h2>
                <p>
                  Enter an existing folder path. Authorization and scanning are separate actions;
                  symlinks and hidden entries are not followed.
                </p>
              </div>
              <span className="scope-count">{state.scopes.length} authorized</span>
            </div>

            <div className="scope-form">
              <label htmlFor="scope-path">Folder path</label>
              <div className="scope-form-row">
                <input
                  id="scope-path"
                  type="text"
                  value={scopePath}
                  onChange={(event) => setScopePath(event.target.value)}
                  placeholder="/Users/you/Documents or C:\Users\you\Documents"
                  autoComplete="off"
                  spellCheck="false"
                />
                <button
                  type="button"
                  disabled={action.kind === 'working'}
                  onClick={() => void authorizeRequestedScope()}
                >
                  Authorize folder
                </button>
              </div>
            </div>

            {action.kind !== 'idle' ? (
              <p className={`action-message action-message--${action.kind}`} role="status">
                {action.kind === 'working' ? action.label : action.message}
              </p>
            ) : null}

            {state.scopes.length === 0 ? (
              <div className="empty-scope">
                <strong>No folder access</strong>
                <span>
                  DeskGraph cannot inspect Desktop, Downloads, or Documents until added here.
                </span>
              </div>
            ) : (
              <ul className="scope-list">
                {state.scopes.map((scope) => {
                  const latestJob = state.jobs.find((job) => job.scope_id === scope.id);
                  const resumableJob =
                    latestJob &&
                    (latestJob.status === 'running' ||
                      latestJob.status === 'paused' ||
                      latestJob.status === 'interrupted')
                      ? latestJob
                      : undefined;
                  return (
                    <li key={scope.id}>
                      <div className="scope-details">
                        <span className="scope-label">Authorized scope {scope.id}</span>
                        <code>{scope.display_path}</code>
                        {latestJob ? (
                          <div className="scan-progress" role="status">
                            <span>{scanStatusLabel(latestJob)}</span>
                            <span>
                              {latestJob.processed_entries.toLocaleString()} /{' '}
                              {latestJob.queued_entries.toLocaleString()} entries ·{' '}
                              {latestJob.issue_count.toLocaleString()} issues
                            </span>
                          </div>
                        ) : null}
                      </div>
                      {resumableJob?.status === 'running' ? (
                        <button
                          type="button"
                          disabled={resumableJob.pause_requested}
                          onClick={() => void pause(resumableJob)}
                        >
                          {resumableJob.pause_requested ? 'Pausing…' : 'Pause scan'}
                        </button>
                      ) : null}
                      {resumableJob?.status === 'paused' ||
                      resumableJob?.status === 'interrupted' ? (
                        <button type="button" onClick={() => void resume(resumableJob)}>
                          Resume scan
                        </button>
                      ) : null}
                      {!resumableJob ? (
                        <button
                          type="button"
                          disabled={action.kind === 'working'}
                          onClick={() => void scan(scope)}
                        >
                          Scan metadata
                        </button>
                      ) : null}
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
        </div>
      ) : null}

      <footer>
        <span>DeskGraph {state.kind === 'ready' ? state.report.app_version : '0.1.0'}</span>
        <span>Metadata + bounded local text · No uploads · No file operations</span>
      </footer>
    </main>
  );
}
