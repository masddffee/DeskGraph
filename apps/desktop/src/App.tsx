import { useEffect, useState } from 'react';

import { lifecycleLabel, loadHealthReport, type HealthReport } from './health';
import {
  loadExtractionStats,
  loadRecentExtractions,
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

type ReadyState = {
  kind: 'ready';
  report: HealthReport;
  manifest: ManifestStats;
  scopes: AuthorizedScope[];
  jobs: ScanJobProgress[];
  extraction: ExtractionStats;
  extractionJobs: ExtractionJobProgress[];
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
  const [report, manifest, scopes, jobs, extraction, extractionJobs] = await Promise.all([
    loadHealthReport(),
    loadManifestStatus(),
    loadAuthorizedScopes(),
    loadRecentScanJobs(),
    loadExtractionStats(),
    loadRecentExtractions(),
  ]);
  return { kind: 'ready', report, manifest, scopes, jobs, extraction, extractionJobs };
}

function replaceJob(jobs: ScanJobProgress[], job: ScanJobProgress): ScanJobProgress[] {
  return [job, ...jobs.filter((candidate) => candidate.job_id !== job.job_id)];
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
  if (job.status === 'running') return 'Extracting bounded text…';
  if (job.status === 'completed') return 'Completed';
  if (job.status === 'cancelled') return 'Cancelled safely';
  if (job.status === 'interrupted') return 'Interrupted safely';
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
  const runningJobIds =
    state.kind === 'ready'
      ? state.jobs.filter((job) => job.status === 'running').map((job) => job.job_id)
      : [];
  const runningJobKey = runningJobIds.join(',');

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

  function updateJob(job: ScanJobProgress) {
    setState((current) =>
      current.kind === 'ready' ? { ...current, jobs: replaceJob(current.jobs, job) } : current,
    );
  }

  async function refreshManifest() {
    const [manifest, scopes, jobs, extraction, extractionJobs] = await Promise.all([
      loadManifestStatus(),
      loadAuthorizedScopes(),
      loadRecentScanJobs(),
      loadExtractionStats(),
      loadRecentExtractions(),
    ]);
    setState((current) =>
      current.kind === 'ready'
        ? { ...current, manifest, scopes, jobs, extraction, extractionJobs }
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
    } catch {
      setSearchState({
        kind: 'error',
        message: 'Search stopped safely. Try a shorter query or refresh the local manifest.',
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
                {searchState.response.results.map((result) => (
                  <li key={`${result.node_id}:${result.location_id}`}>
                    <div className="search-result-heading">
                      <span className="search-rank">#{result.lexical_rank}</span>
                      <strong>{searchExplanation(result)}</strong>
                    </div>
                    <code>{result.display_path}</code>
                    {result.snippet ? (
                      <p className="search-snippet">
                        <span>Untrusted local text</span>
                        {result.snippet}
                      </p>
                    ) : null}
                  </li>
                ))}
              </ol>
            ) : null}
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
                  Only already-scanned text, Markdown, and code files are eligible. Every source is
                  revalidated, output is size-limited, and a failed job cannot replace the last
                  complete text.
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
                <span>Latest job {state.extractionJobs[0].job_id}</span>
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
