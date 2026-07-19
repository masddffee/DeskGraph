import { useEffect, useRef, useState } from 'react';

import { loadHealthReport, type HealthReport } from './health';
import {
  createRenamePreview,
  loadRecentActionPlans,
  type ActionPlanPreview,
  type ActionPlanSummary,
  type ActionPlanState,
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
  createManifestScan,
  loadAuthorizedScopes,
  loadManifestStatus,
  loadRecentScanJobs,
  loadScanJobStatus,
  pauseManifestScan,
  resumeManifestScan,
  runManifestScan,
  selectAndAuthorizeScope,
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
import {
  loadRecentWatchEvents,
  loadWatchRuntimeStatus,
  type WatchEventProgress,
  type WatchEventReason,
  type WatchRuntimeStatus,
} from './watch';
import {
  createCleanupActionPreview,
  getCleanupSourceDetail,
  refreshSmartCleanupInbox,
  type CleanupActionPlanPreview,
  type CleanupSourceDetail,
  type CleanupSourceDetailMember,
  type CleanupSourceKind,
  type SmartCleanupInbox,
  type SmartCleanupInboxItem,
} from './cleanup';
import {
  decideProjectCandidate,
  discoverProjects,
  getProjectCandidateDetail,
  projectScopeHasCompletedScan,
  type ProjectCandidateDetail,
  type ProjectCandidateState,
  type ProjectDecisionKind,
  type ProjectDiscovery,
} from './projects';
import {
  collectLanguagePreferences,
  formatInteger,
  formatUtcDate,
  getCatalog,
  getLocalizedMetadata,
  isLocale,
  loadLocale,
  localeOptions,
  resolveLocale,
  storeLocale,
  type Catalog,
  type Locale,
} from './i18n';

type ReadyState = {
  kind: 'ready';
  report: HealthReport;
  manifest: ManifestStats;
  scopes: AuthorizedScope[];
  jobs: ScanJobProgress[];
  extraction: ExtractionStats;
  extractionJobs: ExtractionJobProgress[];
  watchEvents: WatchEventProgress[];
  watchRuntime: WatchRuntimeStatus;
  actionPlans: ActionPlanSummary[];
};
type ViewState = { kind: 'loading' } | ReadyState | { kind: 'error' };
type ActionMessage =
  | {
      key:
        | 'cancelled'
        | 'validating'
        | 'authorized'
        | 'denied'
        | 'reading'
        | 'interrupted'
        | 'stopped'
        | 'creating'
        | 'startDenied'
        | 'waiting'
        | 'pauseDenied'
        | 'revalidating'
        | 'resumeDenied';
    }
  | { key: 'complete'; files: number; folders: number }
  | { key: 'paused'; processed: number; queued: number };
type ActionState =
  | { kind: 'idle' }
  | { kind: 'working'; message: ActionMessage }
  | { kind: 'success'; message: ActionMessage }
  | { kind: 'cancelled'; message: ActionMessage }
  | { kind: 'error'; message: ActionMessage };
type SearchMessage = 'query' | 'extension' | 'dateRange' | 'request';
type SearchState =
  | { kind: 'idle' }
  | { kind: 'working' }
  | { kind: 'ready'; response: SearchResponse }
  | { kind: 'error'; message: SearchMessage };
type RenameMessage = 'chooseFolder' | 'required' | 'denied';
type RenamePreviewState =
  | { kind: 'idle' }
  | { kind: 'working' }
  | { kind: 'ready'; preview: ActionPlanPreview }
  | { kind: 'error'; message: RenameMessage };
type OcrMessage =
  | 'capacity'
  | 'providerUnavailable'
  | 'indexed'
  | 'cancelledFeedback'
  | 'interruptedFeedback'
  | 'failedFeedback'
  | 'denied'
  | 'resumeDenied'
  | 'cancelDenied';
type OcrActionState =
  | { kind: 'idle' }
  | { kind: 'working'; scopeId: number; nodeId: number }
  | { kind: 'success'; scopeId: number; nodeId: number; message: OcrMessage }
  | { kind: 'error'; scopeId: number; nodeId: number; message: OcrMessage };
type CleanupInboxState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'ready'; inbox: SmartCleanupInbox }
  | { kind: 'partial'; inbox: SmartCleanupInbox }
  | { kind: 'error' };
type CleanupReviewState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'detail'; detail: CleanupSourceDetail; targetNodeId: number | null }
  | { kind: 'creating'; detail: CleanupSourceDetail; targetNodeId: number }
  | { kind: 'ready'; preview: CleanupActionPlanPreview }
  | { kind: 'error'; message: 'detail' | 'selection' | 'preview' };
type ProjectDiscoveryState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'ready'; discovery: ProjectDiscovery }
  | { kind: 'partial'; discovery: ProjectDiscovery }
  | { kind: 'error' };
type ProjectReadinessState = 'idle' | 'loading' | 'ready' | 'scanRequired' | 'error';
type ProjectReviewState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'detail'; detail: ProjectCandidateDetail; decisionError: boolean }
  | { kind: 'saving'; detail: ProjectCandidateDetail; decision: ProjectDecisionKind }
  | { kind: 'error' };
type AppView = 'home' | 'search' | 'projects' | 'inbox' | 'history' | 'settings';

const APP_VIEWS: readonly AppView[] = [
  'home',
  'search',
  'projects',
  'inbox',
  'history',
  'settings',
];

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

function Metric({ label, value, locale }: { label: string; value: number; locale: Locale }) {
  return (
    <div className="metric">
      <strong>{formatInteger(value, locale)}</strong>
      <span>{label}</span>
    </div>
  );
}

async function loadDashboard(): Promise<ReadyState> {
  const [
    report,
    manifest,
    scopes,
    jobs,
    extraction,
    extractionJobs,
    watchEvents,
    watchRuntime,
    actionPlans,
  ] = await Promise.all([
    loadHealthReport(),
    loadManifestStatus(),
    loadAuthorizedScopes(),
    loadRecentScanJobs(),
    loadExtractionStats(),
    loadRecentExtractions(),
    loadRecentWatchEvents(),
    loadWatchRuntimeStatus(),
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
    watchRuntime,
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

function actionMessageLabel(catalog: Catalog, message: ActionMessage): string {
  if (message.key === 'complete') {
    return catalog.scope.validation.complete(message.files, message.folders);
  }
  if (message.key === 'paused') {
    return catalog.scope.validation.paused(message.processed, message.queued);
  }
  return catalog.scope.validation[message.key];
}

function scanStatusLabel(job: ScanJobProgress, catalog: Catalog): string {
  if (job.status === 'running' && job.pause_requested) return catalog.scope.status.pausing;
  if (job.status === 'running') return catalog.scope.status.scanning;
  if (job.status === 'paused') return catalog.scope.status.paused;
  if (job.status === 'interrupted') return catalog.scope.status.interrupted;
  if (job.status === 'completed') return catalog.scope.status.completed;
  return catalog.scope.status.stopped;
}

function extractionStatusLabel(job: ExtractionJobProgress, catalog: Catalog): string {
  if (job.status === 'queued') return catalog.search.ocr.queued;
  if (job.status === 'running' && job.cancel_requested) return catalog.search.ocr.stopping;
  if (job.status === 'running' && job.operation === 'screenshot_ocr') {
    return catalog.search.ocr.reading;
  }
  if (job.status === 'running') return catalog.search.ocr.running;
  if (job.status === 'completed' && job.operation === 'screenshot_ocr') {
    return catalog.search.ocr.completed;
  }
  if (job.status === 'completed') return catalog.scope.status.completed;
  if (job.status === 'cancelled') return catalog.search.ocr.cancelled;
  if (job.status === 'interrupted') return catalog.search.ocr.interrupted;
  if (job.operation === 'screenshot_ocr') return catalog.search.ocr.unavailable;
  return catalog.search.ocr.skipped;
}

function searchExplanation(result: SearchResult, catalog: Catalog): string {
  if (result.explanation === 'exact_filename_and_extracted_text') {
    return catalog.search.explanation.filenameAndText;
  }
  if (result.explanation === 'exact_filename') return catalog.search.explanation.filename;
  if (result.explanation === 'path_and_extracted_text_substring') {
    return catalog.search.explanation.pathAndText;
  }
  if (result.explanation === 'path_substring') return catalog.search.explanation.path;
  return catalog.search.explanation.text;
}

function utcDateToUnixSeconds(value: string): number | null {
  if (!value) return null;
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
  const milliseconds = Date.parse(`${value}T00:00:00Z`);
  if (!Number.isFinite(milliseconds)) return null;
  if (new Date(milliseconds).toISOString().slice(0, 10) !== value) return null;
  return Math.floor(milliseconds / 1000);
}

function activeSearchFilters(response: SearchResponse, catalog: Catalog, locale: Locale): string {
  const labels: string[] = [];
  if (response.filters.scope_id !== null) {
    labels.push(catalog.search.filters.scope(response.filters.scope_id));
  }
  if (response.filters.source === 'metadata_path') labels.push(catalog.search.filters.pathsOnly);
  if (response.filters.source === 'extracted_text') labels.push(catalog.search.filters.textOnly);
  if (response.filters.extension) labels.push(`.${response.filters.extension}`);
  if (response.filters.modified_since_unix_seconds !== null) {
    labels.push(
      catalog.search.filters.since(
        formatUtcDate(response.filters.modified_since_unix_seconds * 1000, locale),
      ),
    );
  }
  if (response.filters.modified_before_unix_seconds !== null) {
    labels.push(
      catalog.search.filters.before(
        formatUtcDate(response.filters.modified_before_unix_seconds * 1000, locale),
      ),
    );
  }
  return labels.length > 0 ? labels.join(' · ') : catalog.search.filters.allSources;
}

function watchReasonLabel(reason: WatchEventReason | null, catalog: Catalog): string {
  if (reason === 'temporary_download') return catalog.watch.reason.temporary;
  if (reason === 'hidden_entry') return catalog.watch.reason.hidden;
  if (reason === 'unsupported_entry') return catalog.watch.reason.unsupported;
  if (reason === 'source_unavailable') return catalog.watch.reason.unavailable;
  if (reason === 'reconcile_failed') return catalog.watch.reason.failed;
  return catalog.watch.status.noFailure;
}

function watchStatusLabel(event: WatchEventProgress, catalog: Catalog): string {
  if (event.status === 'stabilizing') return catalog.watch.status.stabilizing;
  if (event.status === 'reconciling') return catalog.watch.status.reconciling;
  if (event.status === 'completed') return catalog.watch.status.completed;
  return watchReasonLabel(event.reason, catalog);
}

function watchRuntimeLabel(runtime: WatchRuntimeStatus, catalog: Catalog): string {
  if (runtime.state === 'running') return catalog.watch.adapterActive;
  if (runtime.state === 'starting') return catalog.watch.adapterStarting;
  if (runtime.state === 'degraded') return catalog.watch.adapterDegraded;
  return catalog.watch.adapterStopped;
}

function actionPolicyCheckLabel(check: ActionPolicyCheck, catalog: Catalog): string {
  if (check === 'explicit_authorized_scope') return catalog.actions.policy.authorizedScope;
  if (check === 'present_manifest_file') return catalog.actions.policy.manifestFile;
  if (check === 'canonical_source_contained') return catalog.actions.policy.canonicalSource;
  if (check === 'source_identity_matches') return catalog.actions.policy.sourceIdentity;
  if (check === 'read_only_handle_identity_matches') return catalog.actions.policy.readOnlyHandle;
  if (check === 'portable_single_component_name') return catalog.actions.policy.portableName;
  if (check === 'same_canonical_parent') return catalog.actions.policy.sameParent;
  if (check === 'destination_contained') return catalog.actions.policy.destinationScope;
  return catalog.actions.policy.destinationAvailable;
}

function actionPlanStateLabel(state: ActionPlanState, catalog: Catalog): string {
  if (state === 'previewed') return catalog.actions.historyState.previewed;
  if (
    state === 'execute_requested' ||
    state === 'direct_rename_intent' ||
    state === 'undo_requested' ||
    state === 'undo_rename_intent'
  ) {
    return catalog.actions.historyState.pending;
  }
  if (state === 'executed') return catalog.actions.historyState.executed;
  if (state === 'undone') return catalog.actions.historyState.undone;
  return catalog.actions.historyState.needsAttention;
}

function browserLocaleStorage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function cleanupSourceKindLabel(sourceKind: CleanupSourceKind, catalog: Catalog): string {
  if (sourceKind === 'exact_duplicate') return catalog.cleanup.exactDuplicate;
  if (sourceKind === 'version') return catalog.cleanup.version;
  return catalog.cleanup.screenshotReviewGroup;
}

function cleanupSourceExplanation(sourceKind: CleanupSourceKind, catalog: Catalog): string {
  if (sourceKind === 'exact_duplicate') return catalog.cleanup.exactDuplicateExplanation;
  if (sourceKind === 'version') return catalog.cleanup.versionExplanation;
  return catalog.cleanup.screenshotReviewGroupExplanation;
}

function cleanupMemberRoleLabel(member: CleanupSourceDetailMember, catalog: Catalog): string {
  if (member.role === 'duplicate_candidate') {
    return catalog.cleanup.review.roles.duplicateCandidate;
  }
  if (member.role === 'older_version') return catalog.cleanup.review.roles.olderVersion;
  if (member.role === 'newer_version') return catalog.cleanup.review.roles.newerVersion;
  return catalog.cleanup.review.roles.screenshotCandidate;
}

function projectStateLabel(state: ProjectCandidateState, catalog: Catalog): string {
  if (state === 'accepted') return catalog.projects.state.accepted;
  if (state === 'rejected') return catalog.projects.state.rejected;
  return catalog.projects.state.suggested;
}

function browserLanguagePreferences(): readonly string[] {
  try {
    if (typeof navigator === 'undefined') return [];
    return collectLanguagePreferences(navigator.languages, navigator.language);
  } catch {
    return [];
  }
}

export default function App() {
  const [locale, setLocale] = useState<Locale>(() => {
    if (typeof window === 'undefined') return 'en';
    const storage = browserLocaleStorage();
    const preferences = browserLanguagePreferences();
    return storage ? loadLocale(storage, preferences) : resolveLocale(null, preferences);
  });
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<ViewState>({ kind: 'loading' });
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
  const [cleanupScopeId, setCleanupScopeId] = useState<number | null>(null);
  const [cleanupInboxState, setCleanupInboxState] = useState<CleanupInboxState>({ kind: 'idle' });
  const [cleanupReviewState, setCleanupReviewState] = useState<CleanupReviewState>({
    kind: 'idle',
  });
  const [projectScopeId, setProjectScopeId] = useState<number | null>(null);
  const [projectDiscoveryState, setProjectDiscoveryState] = useState<ProjectDiscoveryState>({
    kind: 'idle',
  });
  const [projectReadinessState, setProjectReadinessState] = useState<ProjectReadinessState>('idle');
  const [projectReviewState, setProjectReviewState] = useState<ProjectReviewState>({
    kind: 'idle',
  });
  const [activeView, setActiveView] = useState<AppView>('home');
  const ocrRequestInFlight = useRef(new Set<string>());
  const viewHeadingRef = useRef<HTMLHeadingElement>(null);
  const cleanupReviewHeadingRef = useRef<HTMLHeadingElement>(null);
  const cleanupReviewGenerationRef = useRef(0);
  const cleanupReviewTriggerRef = useRef<HTMLButtonElement | null>(null);
  const projectReviewGenerationRef = useRef(0);
  const projectDiscoveryGenerationRef = useRef(0);
  const projectReadinessGenerationRef = useRef(0);
  const projectScopeIdRef = useRef<number | null>(null);
  const projectReviewTriggerRef = useRef<HTMLButtonElement | null>(null);
  const projectReviewHeadingRef = useRef<HTMLHeadingElement>(null);
  const runningJobIds =
    state.kind === 'ready'
      ? state.jobs.filter((job) => job.status === 'running').map((job) => job.job_id)
      : [];
  const runningJobKey = runningJobIds.join(',');
  const activeExtractionJobIds =
    state.kind === 'ready' ? activeScreenshotOcrJobIds(state.extractionJobs) : [];
  const activeExtractionJobKey = activeExtractionJobIds.join(',');
  const dashboardReady = state.kind === 'ready';
  const catalog = getCatalog(locale);
  const activeViewCopy = catalog.navigation.views[activeView];

  useEffect(() => {
    const metadata = getLocalizedMetadata(locale);
    document.documentElement.lang = metadata.htmlLang;
    document.documentElement.dir = metadata.dir;
    document.title = metadata.title;
    document
      .querySelector('meta[name="description"]')
      ?.setAttribute('content', metadata.description);
  }, [locale]);

  useEffect(() => {
    if (
      cleanupReviewState.kind === 'detail' ||
      cleanupReviewState.kind === 'ready' ||
      cleanupReviewState.kind === 'error'
    ) {
      cleanupReviewHeadingRef.current?.focus();
    }
  }, [cleanupReviewState.kind]);

  useEffect(() => {
    if (projectReviewState.kind === 'detail' || projectReviewState.kind === 'error') {
      projectReviewHeadingRef.current?.focus();
    }
  }, [projectReviewState.kind]);

  function changeLocale(nextLocale: string) {
    if (!isLocale(nextLocale)) return;
    setLocale(nextLocale);
    if (typeof window !== 'undefined') {
      const storage = browserLocaleStorage();
      if (storage) storeLocale(storage, nextLocale);
    }
  }

  function showView(nextView: AppView) {
    if (nextView !== 'inbox') invalidateCleanupReview();
    if (nextView !== 'projects') invalidateProjectReview();
    setActiveView(nextView);
    window.requestAnimationFrame(() => viewHeadingRef.current?.focus());
  }

  function invalidateCleanupReview(returnFocus = false) {
    cleanupReviewGenerationRef.current += 1;
    setCleanupReviewState({ kind: 'idle' });
    if (returnFocus) {
      window.requestAnimationFrame(() => {
        if (cleanupReviewTriggerRef.current?.isConnected) {
          cleanupReviewTriggerRef.current.focus();
        }
      });
    }
  }

  function invalidateProjectReview(returnFocus = false) {
    projectReviewGenerationRef.current += 1;
    setProjectReviewState({ kind: 'idle' });
    if (returnFocus) {
      window.requestAnimationFrame(() => {
        if (projectReviewTriggerRef.current?.isConnected) projectReviewTriggerRef.current.focus();
      });
    }
  }

  function chooseProjectScope(scopeId: number | null) {
    projectScopeIdRef.current = scopeId;
    setProjectScopeId(scopeId);
    projectDiscoveryGenerationRef.current += 1;
    setProjectDiscoveryState({ kind: 'idle' });
    invalidateProjectReview();
    void refreshProjectReadiness(scopeId);
  }

  async function refreshProjectReadiness(scopeId: number | null) {
    if (projectScopeIdRef.current !== scopeId) return;
    const generation = projectReadinessGenerationRef.current + 1;
    projectReadinessGenerationRef.current = generation;
    if (scopeId === null) {
      setProjectReadinessState('idle');
      return;
    }
    setProjectReadinessState('loading');
    try {
      const completed = await projectScopeHasCompletedScan(scopeId);
      if (
        projectReadinessGenerationRef.current !== generation ||
        projectScopeIdRef.current !== scopeId
      )
        return;
      setProjectReadinessState(completed ? 'ready' : 'scanRequired');
    } catch {
      if (
        projectReadinessGenerationRef.current !== generation ||
        projectScopeIdRef.current !== scopeId
      )
        return;
      setProjectReadinessState('error');
    }
  }

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

  useEffect(() => {
    if (!dashboardReady) return;
    let active = true;

    const poll = async () => {
      try {
        const [manifest, watchEvents, watchRuntime] = await Promise.all([
          loadManifestStatus(),
          loadRecentWatchEvents(),
          loadWatchRuntimeStatus(),
        ]);
        if (!active) return;
        setState((current) =>
          current.kind === 'ready'
            ? {
                ...current,
                manifest,
                watchEvents,
                watchRuntime,
              }
            : current,
        );
      } catch {
        // Keep the last validated path-free status until the next poll succeeds.
      }
    };

    const timer = window.setInterval(() => void poll(), 5_000);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, [dashboardReady]);

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
    const [
      manifest,
      scopes,
      jobs,
      extraction,
      extractionJobs,
      watchEvents,
      watchRuntime,
      actionPlans,
    ] = await Promise.all([
      loadManifestStatus(),
      loadAuthorizedScopes(),
      loadRecentScanJobs(),
      loadExtractionStats(),
      loadRecentExtractions(),
      loadRecentWatchEvents(),
      loadWatchRuntimeStatus(),
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
            watchRuntime,
            actionPlans,
          }
        : current,
    );
  }

  async function authorizeRequestedScope() {
    setAction({ kind: 'working', message: { key: 'validating' } });
    try {
      const scope = await selectAndAuthorizeScope();
      if (scope === null) {
        setAction({ kind: 'cancelled', message: { key: 'cancelled' } });
        return;
      }
      await refreshManifest();
      setAction({
        kind: 'success',
        message: { key: 'authorized' },
      });
    } catch {
      setAction({
        kind: 'error',
        message: { key: 'denied' },
      });
    }
  }

  async function runJob(job: ScanJobProgress) {
    try {
      setAction({ kind: 'working', message: { key: 'reading' } });
      const progress = await runManifestScan(job.job_id);
      updateJob(progress);
      await refreshManifest();
      if (progress.status === 'completed') {
        setAction({
          kind: 'success',
          message: {
            key: 'complete',
            files: progress.discovered_files,
            folders: progress.discovered_folders,
          },
        });
      } else if (progress.status === 'paused') {
        setAction({
          kind: 'success',
          message: {
            key: 'paused',
            processed: progress.processed_entries,
            queued: progress.queued_entries,
          },
        });
      } else {
        setAction({
          kind: 'error',
          message: { key: 'interrupted' },
        });
      }
    } catch {
      await refreshManifest().catch(() => undefined);
      setAction({
        kind: 'error',
        message: { key: 'stopped' },
      });
    }
  }

  async function scan(scope: AuthorizedScope) {
    setAction({ kind: 'working', message: { key: 'creating' } });
    try {
      const job = await createManifestScan(scope.id);
      updateJob(job);
      await runJob(job);
      await refreshProjectReadiness(scope.id);
    } catch {
      await refreshManifest().catch(() => undefined);
      setAction({
        kind: 'error',
        message: { key: 'startDenied' },
      });
    }
  }

  async function pause(job: ScanJobProgress) {
    setAction({ kind: 'working', message: { key: 'waiting' } });
    try {
      const progress = await pauseManifestScan(job.job_id);
      updateJob(progress);
      if (progress.status === 'paused') {
        setAction({
          kind: 'success',
          message: {
            key: 'paused',
            processed: progress.processed_entries,
            queued: progress.queued_entries,
          },
        });
      }
    } catch {
      setAction({ kind: 'error', message: { key: 'pauseDenied' } });
    }
  }

  async function resume(job: ScanJobProgress) {
    setAction({ kind: 'working', message: { key: 'revalidating' } });
    try {
      const progress = await resumeManifestScan(job.job_id);
      updateJob(progress);
      await runJob(progress);
    } catch {
      await refreshManifest().catch(() => undefined);
      setAction({
        kind: 'error',
        message: { key: 'resumeDenied' },
      });
    }
  }

  async function submitSearch() {
    const query = searchQuery.trim();
    if ([...query].length < 3) {
      setSearchState({
        kind: 'error',
        message: 'query',
      });
      return;
    }
    const extension = searchExtension.trim();
    if (extension && !/^\.?[a-z0-9]{1,16}$/i.test(extension)) {
      setSearchState({
        kind: 'error',
        message: 'extension',
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
        message: 'dateRange',
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
        message: 'request',
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
          message: 'capacity',
        });
        return;
      }
      setOcrAction({
        kind: 'error',
        scopeId: job.scope_id,
        nodeId: job.node_id,
        message: 'providerUnavailable',
      });
    }
  }

  function setOcrFeedbackFromProgress(progress: ExtractionJobProgress) {
    if (progress.status === 'completed') {
      setOcrAction({
        kind: 'success',
        scopeId: progress.scope_id,
        nodeId: progress.node_id,
        message: 'indexed',
      });
      return;
    }
    if (progress.status === 'cancelled') {
      setOcrAction({
        kind: 'success',
        scopeId: progress.scope_id,
        nodeId: progress.node_id,
        message: 'cancelledFeedback',
      });
      return;
    }
    if (progress.status === 'interrupted') {
      setOcrAction({
        kind: 'error',
        scopeId: progress.scope_id,
        nodeId: progress.node_id,
        message: 'interruptedFeedback',
      });
      return;
    }
    if (progress.status === 'failed') {
      setOcrAction({
        kind: 'error',
        scopeId: progress.scope_id,
        nodeId: progress.node_id,
        message: 'failedFeedback',
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
        message: 'denied',
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
        message: 'resumeDenied',
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
        message: 'cancelDenied',
      });
    }
  }

  async function submitRenamePreview() {
    const sourcePath = renameSourcePath.trim();
    if (renameScopeId === null) {
      setRenameState({ kind: 'error', message: 'chooseFolder' });
      return;
    }
    if (!sourcePath || !renameNewName) {
      setRenameState({
        kind: 'error',
        message: 'required',
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
        message: 'denied',
      });
    }
  }

  async function refreshCleanupInbox() {
    if (cleanupScopeId === null) return;
    invalidateCleanupReview();
    setCleanupInboxState({ kind: 'loading' });
    try {
      const inbox = await refreshSmartCleanupInbox(cleanupScopeId);
      setCleanupInboxState({
        kind: inbox.evaluation_complete ? 'ready' : 'partial',
        inbox,
      });
    } catch {
      setCleanupInboxState({ kind: 'error' });
    }
  }

  async function discoverProjectCandidates() {
    const scopeId = projectScopeIdRef.current;
    if (scopeId === null || projectReadinessState !== 'ready') return;
    invalidateProjectReview();
    const generation = projectDiscoveryGenerationRef.current + 1;
    projectDiscoveryGenerationRef.current = generation;
    setProjectDiscoveryState({ kind: 'loading' });
    try {
      const discovery = await discoverProjects(scopeId);
      if (
        projectDiscoveryGenerationRef.current !== generation ||
        projectScopeIdRef.current !== scopeId
      )
        return;
      setProjectDiscoveryState({
        kind: discovery.evaluation_complete ? 'ready' : 'partial',
        discovery,
      });
    } catch {
      if (
        projectDiscoveryGenerationRef.current !== generation ||
        projectScopeIdRef.current !== scopeId
      )
        return;
      setProjectDiscoveryState({ kind: 'error' });
    }
  }

  async function openProjectReview(projectId: number, trigger: HTMLButtonElement) {
    if (projectScopeId === null) return;
    projectReviewTriggerRef.current = trigger;
    const generation = projectReviewGenerationRef.current + 1;
    projectReviewGenerationRef.current = generation;
    setProjectReviewState({ kind: 'loading' });
    try {
      const detail = await getProjectCandidateDetail(projectScopeId, projectId);
      if (projectReviewGenerationRef.current !== generation) return;
      setProjectReviewState({ kind: 'detail', detail, decisionError: false });
    } catch {
      if (projectReviewGenerationRef.current !== generation) return;
      setProjectReviewState({ kind: 'error' });
    }
  }

  async function submitProjectDecision(decision: ProjectDecisionKind) {
    if (projectReviewState.kind !== 'detail') return;
    const { detail } = projectReviewState;
    const generation = projectReviewGenerationRef.current + 1;
    projectReviewGenerationRef.current = generation;
    setProjectReviewState({ kind: 'saving', detail, decision });
    try {
      const updated = await decideProjectCandidate(
        detail.candidate.scope_id,
        detail.candidate.project_id,
        decision,
      );
      if (projectReviewGenerationRef.current !== generation) return;
      setProjectDiscoveryState((current) => {
        if (current.kind !== 'ready' && current.kind !== 'partial') return current;
        return {
          ...current,
          discovery: {
            ...current.discovery,
            candidates: current.discovery.candidates.map((candidate) =>
              candidate.project_id === updated.candidate.project_id
                ? {
                    ...candidate,
                    state: updated.candidate.state,
                    confidence_basis_points: updated.candidate.suggestion.confidence_basis_points,
                    observed_at_unix_ms: updated.candidate.suggestion.observed_at_unix_ms,
                    latest_decision_at_unix_ms:
                      updated.candidate.latest_decision?.decided_at_unix_ms ?? null,
                  }
                : candidate,
            ),
          },
        };
      });
      setProjectReviewState({ kind: 'detail', detail: updated, decisionError: false });
    } catch {
      if (projectReviewGenerationRef.current !== generation) return;
      setProjectReviewState({ kind: 'detail', detail, decisionError: true });
    }
  }

  async function openCleanupReview(item: SmartCleanupInboxItem, trigger: HTMLButtonElement) {
    cleanupReviewTriggerRef.current = trigger;
    const generation = cleanupReviewGenerationRef.current + 1;
    cleanupReviewGenerationRef.current = generation;
    setCleanupReviewState({ kind: 'loading' });
    try {
      const detail = await getCleanupSourceDetail(item);
      if (cleanupReviewGenerationRef.current !== generation) return;
      setCleanupReviewState({ kind: 'detail', detail, targetNodeId: null });
    } catch {
      if (cleanupReviewGenerationRef.current !== generation) return;
      setCleanupReviewState({ kind: 'error', message: 'detail' });
    }
  }

  async function submitCleanupPreview() {
    if (cleanupReviewState.kind !== 'detail') return;
    const { detail, targetNodeId } = cleanupReviewState;
    if (targetNodeId === null) {
      setCleanupReviewState({ kind: 'error', message: 'selection' });
      return;
    }
    const keeperNodeId =
      detail.selection_rule === 'either_member_is_target'
        ? (detail.members.find((member) => member.node_id !== targetNodeId)?.node_id ?? null)
        : detail.selection_rule === 'older_target_newer_keeper'
          ? (detail.members.find((member) => member.role === 'newer_version')?.node_id ?? null)
          : null;
    const generation = cleanupReviewGenerationRef.current + 1;
    cleanupReviewGenerationRef.current = generation;
    setCleanupReviewState({ kind: 'creating', detail, targetNodeId });
    try {
      const preview = await createCleanupActionPreview(detail, targetNodeId, keeperNodeId);
      if (cleanupReviewGenerationRef.current !== generation) return;
      setCleanupReviewState({ kind: 'ready', preview });
    } catch {
      if (cleanupReviewGenerationRef.current !== generation) return;
      setCleanupReviewState({ kind: 'error', message: 'preview' });
    }
  }

  return (
    <div className="app-frame">
      <a className="skip-link" href="#main-content">
        {catalog.navigation.skipToContent}
      </a>
      <aside className="app-sidebar">
        <div className="brand-lockup">
          <span className="brand-mark" aria-hidden="true">
            DG
          </span>
          <div>
            <strong>DeskGraph</strong>
            <span>{catalog.navigation.brandDescription}</span>
          </div>
        </div>

        <nav className="app-navigation" aria-label={catalog.navigation.ariaLabel}>
          {APP_VIEWS.map((view) => (
            <button
              key={view}
              type="button"
              className={
                activeView === view ? 'navigation-item navigation-item--active' : 'navigation-item'
              }
              aria-current={activeView === view ? 'page' : undefined}
              onClick={() => showView(view)}
            >
              <span>{catalog.navigation.views[view].label}</span>
            </button>
          ))}
        </nav>

        <div className="sidebar-status" aria-label={catalog.runtime.localOnly}>
          <span className="privacy-dot privacy-dot--on" aria-hidden="true" />
          <div>
            <strong>{catalog.navigation.localOnly}</strong>
            <span>{catalog.navigation.noNetwork}</span>
          </div>
        </div>

        <div className="sidebar-controls">
          <label className="language-selector" htmlFor="display-language">
            <span>{catalog.language.selectorLabel}</span>
            <select
              id="display-language"
              value={locale}
              onChange={(event) => changeLocale(event.target.value)}
            >
              {localeOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <span className="release-badge">{catalog.hero.release}</span>
        </div>
      </aside>

      <main id="main-content" className="app-shell" tabIndex={-1}>
        <header className="workspace-header">
          <div>
            <p className="eyebrow">{catalog.navigation.views[activeView].label}</p>
            <h1 ref={viewHeadingRef} tabIndex={-1}>
              {activeViewCopy.title}
            </h1>
            <p className="hero-copy">{activeViewCopy.description}</p>
          </div>
          <div className="workspace-badges" aria-label={catalog.runtime.localOnly}>
            <span className="connected-indicator">{catalog.navigation.localOnly}</span>
            <span className="connected-indicator connected-indicator--quiet">
              {catalog.navigation.noNetwork}
            </span>
          </div>
        </header>

        {state.kind === 'loading' ? (
          <section className="state-card" aria-live="polite" aria-busy="true">
            <span className="loader" aria-hidden="true" />
            <div>
              <h2>{catalog.loading.heading}</h2>
              <p>{catalog.loading.description}</p>
            </div>
          </section>
        ) : null}

        {state.kind === 'error' ? (
          <section className="state-card state-card--error" role="alert">
            <div>
              <h2>{catalog.backendError.heading}</h2>
              <p>{catalog.backendError.description}</p>
            </div>
            <button type="button" onClick={() => setAttempt((value) => value + 1)}>
              {catalog.backendError.retry}
            </button>
          </section>
        ) : null}

        {state.kind === 'ready' ? (
          <div className="dashboard">
            {activeView === 'home' ? (
              <>
                <section className="panel" aria-labelledby="runtime-title">
                  <div className="panel-heading">
                    <div>
                      <p className="panel-kicker">{catalog.runtime.kicker}</p>
                      <h2 id="runtime-title">{catalog.runtime.heading}</h2>
                    </div>
                    <span className="connected-indicator">{catalog.runtime.localOnly}</span>
                  </div>
                  <div className="status-list">
                    <StatusRow
                      label={catalog.runtime.platform}
                      value={`${state.report.platform.os} · ${state.report.platform.architecture}`}
                      tone="ok"
                    />
                    <StatusRow
                      label={catalog.runtime.sqliteManifest}
                      value={catalog.runtime.ready}
                      tone="ok"
                    />
                    <StatusRow
                      label={catalog.runtime.optionalLocalLlm}
                      value={
                        state.report.providers.local_llm.state === 'ready'
                          ? catalog.runtime.ready
                          : state.report.providers.local_llm.state === 'not_initialized'
                            ? catalog.runtime.lifecycle.notInitialized
                            : catalog.runtime.lifecycle.disabled
                      }
                    />
                    <StatusRow
                      label={catalog.runtime.networkRequired}
                      value={catalog.runtime.no}
                      tone="ok"
                    />
                  </div>
                </section>

                <section className="panel panel--privacy" aria-labelledby="manifest-title">
                  <p className="panel-kicker">{catalog.manifest.kicker}</p>
                  <h2 id="manifest-title">
                    {state.manifest.completed_scan_count === 0
                      ? catalog.manifest.emptyHeading
                      : catalog.manifest.readyHeading}
                  </h2>
                  <div className="metrics">
                    <Metric
                      label={catalog.manifest.files}
                      value={state.manifest.file_count}
                      locale={locale}
                    />
                    <Metric
                      label={catalog.manifest.folders}
                      value={state.manifest.folder_count}
                      locale={locale}
                    />
                    <Metric
                      label={catalog.manifest.locations}
                      value={state.manifest.active_location_count}
                      locale={locale}
                    />
                    <Metric
                      label={catalog.manifest.scanIssues}
                      value={state.manifest.issue_count}
                      locale={locale}
                    />
                  </div>
                </section>
              </>
            ) : null}

            {activeView === 'search' ? (
              <section className="panel panel--search" aria-labelledby="search-title">
                <div className="panel-heading panel-heading--wrap">
                  <div>
                    <p className="panel-kicker">{catalog.search.kicker}</p>
                    <h2 id="search-title">{catalog.search.heading}</h2>
                    <p>{catalog.search.description}</p>
                  </div>
                  <span className="connected-indicator">{catalog.search.mode}</span>
                </div>
                <form
                  className="search-form"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void submitSearch();
                  }}
                >
                  <label htmlFor="search-query">{catalog.search.queryLabel}</label>
                  <div className="search-form-row">
                    <input
                      id="search-query"
                      type="search"
                      value={searchQuery}
                      onChange={(event) => setSearchQuery(event.target.value)}
                      placeholder={catalog.search.queryPlaceholder}
                      autoComplete="off"
                      spellCheck="false"
                      maxLength={256}
                    />
                    <select
                      aria-label={catalog.search.scopeAria}
                      value={searchScopeId ?? ''}
                      onChange={(event) =>
                        setSearchScopeId(event.target.value ? Number(event.target.value) : null)
                      }
                    >
                      <option value="">{catalog.search.allFolders}</option>
                      {state.scopes.map((scope) => (
                        <option key={scope.id} value={scope.id}>
                          {catalog.search.authorizedScope(scope.id)}
                        </option>
                      ))}
                    </select>
                    <button type="submit" disabled={searchState.kind === 'working'}>
                      {searchState.kind === 'working'
                        ? catalog.search.searching
                        : catalog.search.search}
                    </button>
                  </div>
                  <div className="search-filter-grid" aria-label={catalog.search.filtersAria}>
                    <label>
                      {catalog.search.sourceLabel}
                      <select
                        value={searchSource}
                        onChange={(event) =>
                          setSearchSource(event.target.value as SearchSourceFilter)
                        }
                      >
                        <option value="all">{catalog.search.sources.all}</option>
                        <option value="metadata_path">{catalog.search.sources.paths}</option>
                        <option value="extracted_text">
                          {catalog.search.sources.extractedText}
                        </option>
                      </select>
                    </label>
                    <label>
                      {catalog.search.fileType}
                      <input
                        value={searchExtension}
                        onChange={(event) => setSearchExtension(event.target.value)}
                        placeholder={catalog.search.fileTypePlaceholder}
                        maxLength={17}
                        autoComplete="off"
                        spellCheck="false"
                      />
                    </label>
                    <label>
                      {catalog.search.modifiedSince}
                      <input
                        type="date"
                        value={searchModifiedSince}
                        onChange={(event) => setSearchModifiedSince(event.target.value)}
                      />
                    </label>
                    <label>
                      {catalog.search.modifiedBefore}
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
                    {catalog.search.validation[searchState.message]}
                  </p>
                ) : null}
                {searchState.kind === 'ready' && searchState.response.results.length === 0 ? (
                  <p className="search-message" role="status">
                    {catalog.search.empty(searchState.response.query)}
                  </p>
                ) : null}
                {searchState.kind === 'ready' && searchState.response.results.length > 0 ? (
                  <div className="search-summary" role="status">
                    <span>
                      {catalog.search.summary(
                        searchState.response.result_count,
                        searchState.response.elapsed_ms,
                      )}
                    </span>
                    <span>{activeSearchFilters(searchState.response, catalog, locale)}</span>
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
                            <span className="search-rank">
                              #{formatInteger(result.lexical_rank, locale)}
                            </span>
                            <strong>{searchExplanation(result, catalog)}</strong>
                          </div>
                          <code>{result.display_path}</code>
                          {isScreenshotCandidateDisplayPath(result.display_path) ? (
                            <div
                              className="search-result-action"
                              aria-label={catalog.search.ocr.controlsAria}
                            >
                              <div className="search-result-action-row">
                                <div>
                                  <strong>
                                    {ocrJob
                                      ? extractionStatusLabel(ocrJob, catalog)
                                      : catalog.search.ocr.notRead}
                                  </strong>
                                  <span>{catalog.search.ocr.description}</span>
                                </div>
                                {ocrIsRunning && ocrJob ? (
                                  <button
                                    type="button"
                                    disabled={ocrJob.cancel_requested}
                                    onClick={() => void cancelScreenshotOcr(ocrJob)}
                                  >
                                    {ocrJob.cancel_requested
                                      ? catalog.search.ocr.stopping
                                      : catalog.search.ocr.cancel}
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
                                      {catalog.search.ocr.retryQueued}
                                    </button>
                                    <button
                                      type="button"
                                      disabled={ocrJob.cancel_requested}
                                      onClick={() => void cancelScreenshotOcr(ocrJob)}
                                    >
                                      {catalog.search.ocr.cancel}
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
                                    {catalog.search.ocr.resume}
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
                                    {ocrJob
                                      ? catalog.search.ocr.readAgain
                                      : catalog.search.ocr.read}
                                  </button>
                                )}
                              </div>
                              {feedbackMatches && ocrAction.kind === 'error' ? (
                                <p className="ocr-feedback ocr-feedback--error" role="alert">
                                  {catalog.search.ocr[ocrAction.message]}
                                </p>
                              ) : null}
                              {feedbackMatches && ocrAction.kind === 'success' ? (
                                <p className="ocr-feedback" role="status">
                                  {catalog.search.ocr[ocrAction.message]}
                                </p>
                              ) : null}
                            </div>
                          ) : null}
                          {result.snippet ? (
                            <p className="search-snippet">
                              <span>{catalog.search.ocr.untrustedText}</span>
                              {result.snippet}
                            </p>
                          ) : null}
                        </li>
                      );
                    })}
                  </ol>
                ) : null}
              </section>
            ) : null}

            {activeView === 'history' ? (
              <section className="panel panel--actions" aria-labelledby="actions-title">
                <div className="panel-heading panel-heading--wrap">
                  <div>
                    <p className="panel-kicker">{catalog.actions.kicker}</p>
                    <h2 id="actions-title">{catalog.actions.heading}</h2>
                    <p>{catalog.actions.description}</p>
                  </div>
                  <span className="connected-indicator connected-indicator--pending">
                    {catalog.actions.previewOnly}
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
                    {catalog.actions.folderLabel}
                    <select
                      value={renameScopeId ?? ''}
                      onChange={(event) =>
                        setRenameScopeId(event.target.value ? Number(event.target.value) : null)
                      }
                    >
                      <option value="">{catalog.actions.chooseFolder}</option>
                      {state.scopes.map((scope) => (
                        <option key={scope.id} value={scope.id}>
                          {catalog.actions.scopeOption(scope.id, scope.display_path)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    {catalog.actions.sourceLabel}
                    <input
                      value={renameSourcePath}
                      onChange={(event) => setRenameSourcePath(event.target.value)}
                      placeholder={catalog.actions.sourcePlaceholder}
                      autoComplete="off"
                      spellCheck="false"
                    />
                  </label>
                  <label>
                    {catalog.actions.newNameLabel}
                    <input
                      value={renameNewName}
                      onChange={(event) => setRenameNewName(event.target.value)}
                      placeholder={catalog.actions.newNamePlaceholder}
                      autoComplete="off"
                      spellCheck="false"
                      maxLength={255}
                    />
                  </label>
                  <button type="submit" disabled={renameState.kind === 'working'}>
                    {renameState.kind === 'working'
                      ? catalog.actions.validating
                      : catalog.actions.createPreview}
                  </button>
                </form>

                {renameState.kind === 'error' ? (
                  <p className="action-message action-message--error" role="alert">
                    {catalog.actions.validation[renameState.message]}
                  </p>
                ) : null}

                {renameState.kind === 'ready' ? (
                  <div
                    className="rename-preview"
                    role="status"
                    aria-label={catalog.actions.previewAria}
                  >
                    <div className="rename-preview-heading">
                      <div>
                        <strong>{catalog.actions.plan(renameState.preview.plan_id)}</strong>
                        <span>
                          {renameState.preview.execution_strategy === 'case_only_staged'
                            ? catalog.actions.caseOnly
                            : catalog.actions.direct}
                        </span>
                      </div>
                      <span className="status-pill status-pill--ok">
                        {catalog.actions.unchanged}
                      </span>
                    </div>
                    <div className="rename-paths">
                      <div>
                        <span>{catalog.actions.before}</span>
                        <code>{renameState.preview.source_path}</code>
                      </div>
                      <div>
                        <span>{catalog.actions.after}</span>
                        <code>{renameState.preview.destination_path}</code>
                      </div>
                    </div>
                    <ul className="policy-checks" aria-label={catalog.actions.policyAria}>
                      {renameState.preview.policy.checks.map((check) => (
                        <li key={check}>{actionPolicyCheckLabel(check, catalog)}</li>
                      ))}
                    </ul>
                    <p className="content-empty">{catalog.actions.noExecute}</p>
                  </div>
                ) : null}

                <div className="action-history-heading">
                  <strong>{catalog.actions.historyHeading}</strong>
                  <span>{catalog.actions.plans(state.actionPlans.length)}</span>
                </div>
                {state.actionPlans.length === 0 ? (
                  <p className="content-empty">{catalog.actions.historyEmpty}</p>
                ) : (
                  <ol className="action-plan-list">
                    {state.actionPlans.slice(0, 5).map((plan) => (
                      <li key={plan.plan_id}>
                        <div>
                          <strong>{catalog.actions.historyPlan(plan.plan_id)}</strong>
                          <span>
                            {catalog.actions.historyScopeNode(plan.scope_id, plan.node_id)}
                          </span>
                        </div>
                        <span>
                          {plan.execution_strategy === 'case_only_staged'
                            ? catalog.actions.caseOnlyStaged
                            : catalog.actions.directStrategy}
                          {' · '}
                          {actionPlanStateLabel(plan.state, catalog)}
                        </span>
                      </li>
                    ))}
                  </ol>
                )}
              </section>
            ) : null}

            {activeView === 'home' ? (
              <section className="panel panel--watch" aria-labelledby="watch-title">
                <div className="panel-heading panel-heading--wrap">
                  <div>
                    <p className="panel-kicker">{catalog.watch.kicker}</p>
                    <h2 id="watch-title">{catalog.watch.heading}</h2>
                    <p>{catalog.watch.description}</p>
                  </div>
                  <span
                    className={`connected-indicator${
                      state.watchRuntime.state === 'running' ? '' : ' connected-indicator--pending'
                    }`}
                  >
                    {watchRuntimeLabel(state.watchRuntime, catalog)}
                  </span>
                </div>
                <div className="metrics metrics--content">
                  <Metric
                    label={catalog.watch.metrics.recent}
                    value={state.watchEvents.length}
                    locale={locale}
                  />
                  <Metric
                    label={catalog.watch.metrics.observed}
                    value={state.watchEvents.reduce(
                      (total, event) => total + event.observation_count,
                      0,
                    )}
                    locale={locale}
                  />
                  <Metric
                    label={catalog.watch.metrics.reconciled}
                    value={state.watchEvents.filter((event) => event.status === 'completed').length}
                    locale={locale}
                  />
                  <Metric
                    label={catalog.watch.metrics.deferred}
                    value={state.watchRuntime.deferred_scope_count}
                    locale={locale}
                  />
                  <Metric
                    label={catalog.watch.metrics.attention}
                    value={state.watchRuntime.degraded_scope_count}
                    locale={locale}
                  />
                </div>
                {state.watchEvents.length === 0 ? (
                  <p className="content-empty">{catalog.watch.empty}</p>
                ) : (
                  <ol className="watch-event-list">
                    {state.watchEvents.slice(0, 3).map((event) => (
                      <li key={event.event_id}>
                        <div>
                          <strong>{watchStatusLabel(event, catalog)}</strong>
                          <span>
                            {catalog.watch.event(
                              event.event_id,
                              event.scope_id,
                              event.observation_count,
                            )}
                          </span>
                        </div>
                        <span>
                          {event.scan_job_id
                            ? catalog.watch.scan(event.scan_job_id)
                            : catalog.watch.noScan}
                        </span>
                      </li>
                    ))}
                  </ol>
                )}
              </section>
            ) : null}

            {activeView === 'search' ? (
              <section className="panel panel--content" aria-labelledby="content-title">
                <div className="panel-heading panel-heading--wrap">
                  <div>
                    <p className="panel-kicker">{catalog.extraction.kicker}</p>
                    <h2 id="content-title">
                      {state.extraction.extracted_file_count === 0
                        ? catalog.extraction.emptyHeading
                        : catalog.extraction.readyHeading}
                    </h2>
                    <p>{catalog.extraction.description}</p>
                  </div>
                  <span className="connected-indicator">{catalog.extraction.neverUploaded}</span>
                </div>
                <div className="metrics metrics--content">
                  <Metric
                    label={catalog.extraction.metrics.files}
                    value={state.extraction.extracted_file_count}
                    locale={locale}
                  />
                  <Metric
                    label={catalog.extraction.metrics.chunks}
                    value={state.extraction.active_chunk_count}
                    locale={locale}
                  />
                  <Metric
                    label={catalog.extraction.metrics.completed}
                    value={state.extraction.completed_job_count}
                    locale={locale}
                  />
                  <Metric
                    label={catalog.extraction.metrics.skipped}
                    value={state.extraction.failed_job_count + state.extraction.cancelled_job_count}
                    locale={locale}
                  />
                </div>
                {state.extractionJobs[0] ? (
                  <div className="extraction-progress" role="status">
                    <span>
                      {catalog.extraction.latest(
                        state.extractionJobs[0].operation === 'screenshot_ocr'
                          ? catalog.extraction.operation.screenshotOcr
                          : catalog.extraction.operation.content,
                        state.extractionJobs[0].job_id,
                      )}
                    </span>
                    <strong>{extractionStatusLabel(state.extractionJobs[0], catalog)}</strong>
                    <span>
                      {catalog.extraction.progress(
                        state.extractionJobs[0].chunk_count,
                        state.extractionJobs[0].output_bytes,
                      )}
                    </span>
                  </div>
                ) : (
                  <p className="content-empty">{catalog.extraction.optInEmpty}</p>
                )}
              </section>
            ) : null}

            {activeView === 'inbox' ? (
              <section className="panel panel--cleanup" aria-labelledby="cleanup-title">
                <div className="panel-heading panel-heading--wrap">
                  <div>
                    <p className="panel-kicker">{catalog.cleanup.kicker}</p>
                    <h2 id="cleanup-title">{catalog.cleanup.heading}</h2>
                    <p>{catalog.cleanup.description}</p>
                  </div>
                  <span className="connected-indicator">{catalog.cleanup.suggestionOnly}</span>
                </div>
                <div className="cleanup-controls" aria-label={catalog.cleanup.controlsAria}>
                  <label htmlFor="cleanup-scope">{catalog.cleanup.scopeLabel}</label>
                  <div className="scope-form-row">
                    <select
                      id="cleanup-scope"
                      value={cleanupScopeId ?? ''}
                      disabled={
                        state.scopes.length === 0 ||
                        cleanupInboxState.kind === 'loading' ||
                        cleanupReviewState.kind === 'creating'
                      }
                      onChange={(event) => {
                        setCleanupScopeId(event.target.value ? Number(event.target.value) : null);
                        setCleanupInboxState({ kind: 'idle' });
                        invalidateCleanupReview();
                      }}
                    >
                      <option value="">{catalog.cleanup.chooseScope}</option>
                      {state.scopes.map((scope) => (
                        <option key={scope.id} value={scope.id}>
                          {catalog.search.authorizedScope(scope.id)}
                        </option>
                      ))}
                    </select>
                    <button
                      type="button"
                      disabled={
                        cleanupScopeId === null ||
                        cleanupInboxState.kind === 'loading' ||
                        cleanupReviewState.kind === 'creating'
                      }
                      onClick={() => void refreshCleanupInbox()}
                    >
                      {cleanupInboxState.kind === 'loading'
                        ? catalog.cleanup.refreshing
                        : catalog.cleanup.refresh}
                    </button>
                  </div>
                </div>
                {state.scopes.length === 0 ? (
                  <p className="content-empty" role="status">
                    {catalog.cleanup.authorizationRequired}
                  </p>
                ) : null}
                {state.scopes.length > 0 && cleanupScopeId === null ? (
                  <p className="content-empty" role="status">
                    {catalog.cleanup.chooseScope}
                  </p>
                ) : null}
                {cleanupInboxState.kind === 'loading' ? (
                  <p className="content-empty" role="status" aria-live="polite">
                    {catalog.cleanup.refreshing}
                  </p>
                ) : null}
                {cleanupInboxState.kind === 'error' ? (
                  <p className="content-empty cleanup-message--error" role="alert">
                    {catalog.cleanup.error}
                  </p>
                ) : null}
                {cleanupInboxState.kind === 'partial' ? (
                  <p className="content-empty" role="status">
                    {catalog.cleanup.partial(cleanupInboxState.inbox.not_current_source_count)}
                  </p>
                ) : null}
                {cleanupInboxState.kind === 'ready' || cleanupInboxState.kind === 'partial' ? (
                  cleanupInboxState.inbox.items.length === 0 ? (
                    <p className="content-empty" role="status">
                      {catalog.cleanup.empty}
                    </p>
                  ) : (
                    <ol className="cleanup-inbox-list" aria-label={catalog.cleanup.heading}>
                      {cleanupInboxState.inbox.items.map((item) => (
                        <li
                          key={`${item.source_kind}:${item.source_id}:${item.source_observation_id}`}
                        >
                          <strong>{cleanupSourceKindLabel(item.source_kind, catalog)}</strong>
                          <span>
                            {catalog.cleanup.itemMeta(
                              item.member_count,
                              item.confidence_basis_points,
                              formatUtcDate(item.observed_at_unix_ms, locale),
                            )}
                          </span>
                          <span>{cleanupSourceExplanation(item.source_kind, catalog)}</span>
                          <button
                            type="button"
                            className="cleanup-review-open"
                            disabled={
                              cleanupReviewState.kind === 'loading' ||
                              cleanupReviewState.kind === 'creating'
                            }
                            onClick={(event) => void openCleanupReview(item, event.currentTarget)}
                          >
                            {catalog.cleanup.review.open}
                          </button>
                        </li>
                      ))}
                    </ol>
                  )
                ) : null}
                {cleanupInboxState.kind === 'ready' || cleanupInboxState.kind === 'partial' ? (
                  <p className="content-empty cleanup-verification">
                    {catalog.cleanup.verification}
                  </p>
                ) : null}

                {cleanupReviewState.kind === 'loading' ? (
                  <p className="content-empty" role="status" aria-live="polite">
                    {catalog.cleanup.review.loading}
                  </p>
                ) : null}

                {cleanupReviewState.kind === 'detail' || cleanupReviewState.kind === 'creating' ? (
                  <section className="cleanup-review" aria-labelledby="cleanup-review-title">
                    <div className="cleanup-review-heading">
                      <div>
                        <h3 id="cleanup-review-title" ref={cleanupReviewHeadingRef} tabIndex={-1}>
                          {cleanupSourceKindLabel(cleanupReviewState.detail.source_kind, catalog)}
                        </h3>
                        <p>{catalog.cleanup.review.transientNotice}</p>
                      </div>
                      <button
                        type="button"
                        className="button-secondary"
                        disabled={cleanupReviewState.kind === 'creating'}
                        onClick={() => invalidateCleanupReview(true)}
                      >
                        {catalog.cleanup.review.close}
                      </button>
                    </div>

                    <fieldset className="cleanup-member-selection">
                      <legend>{catalog.cleanup.review.selectionLegend}</legend>
                      {cleanupReviewState.detail.selection_rule === 'single_target_no_keeper' ? (
                        <p className="cleanup-selection-hint">{catalog.cleanup.review.noKeeper}</p>
                      ) : null}
                      {cleanupReviewState.detail.members.map((member) => {
                        const selected = cleanupReviewState.targetNodeId === member.node_id;
                        const canBeTarget = member.role !== 'newer_version';
                        const isKeeper =
                          member.role === 'newer_version' ||
                          (cleanupReviewState.detail.selection_rule === 'either_member_is_target' &&
                            cleanupReviewState.targetNodeId !== null &&
                            !selected);
                        const memberCopy = (
                          <span className="cleanup-member-copy">
                            <span className="cleanup-member-role">
                              {isKeeper && canBeTarget
                                ? catalog.cleanup.review.keeperSwitch
                                : canBeTarget
                                  ? catalog.cleanup.review.selectTarget
                                  : catalog.cleanup.review.keeper}
                            </span>
                            <code>{member.display_path}</code>
                            <span>
                              {cleanupMemberRoleLabel(member, catalog)}
                              {' · '}
                              {catalog.cleanup.review.memberSize(member.size_bytes)}
                            </span>
                          </span>
                        );
                        if (!canBeTarget) {
                          return (
                            <div
                              key={member.node_id}
                              className="cleanup-member cleanup-member--keeper"
                            >
                              <span className="cleanup-member-marker" aria-hidden="true">
                                ✓
                              </span>
                              {memberCopy}
                            </div>
                          );
                        }
                        return (
                          <label
                            key={member.node_id}
                            className={
                              selected
                                ? 'cleanup-member cleanup-member--selected'
                                : 'cleanup-member'
                            }
                          >
                            <input
                              type="radio"
                              name="cleanup-target"
                              value={member.node_id}
                              checked={selected}
                              disabled={cleanupReviewState.kind === 'creating'}
                              onChange={() =>
                                setCleanupReviewState({
                                  kind: 'detail',
                                  detail: cleanupReviewState.detail,
                                  targetNodeId: member.node_id,
                                })
                              }
                            />
                            {memberCopy}
                          </label>
                        );
                      })}
                    </fieldset>

                    <button
                      type="button"
                      className="cleanup-preview-create"
                      disabled={
                        cleanupReviewState.kind === 'creating' ||
                        cleanupReviewState.targetNodeId === null
                      }
                      onClick={() => void submitCleanupPreview()}
                    >
                      {cleanupReviewState.kind === 'creating'
                        ? catalog.cleanup.review.creatingPreview
                        : catalog.cleanup.review.createPreview}
                    </button>
                    {cleanupReviewState.targetNodeId === null ? (
                      <p className="cleanup-selection-hint">
                        {catalog.cleanup.review.selectionRequired}
                      </p>
                    ) : null}
                    <p className="content-empty cleanup-verification">
                      {catalog.cleanup.review.noExecution}
                    </p>
                  </section>
                ) : null}

                {cleanupReviewState.kind === 'error' ? (
                  <div className="cleanup-review cleanup-review--error" role="alert">
                    <h3 ref={cleanupReviewHeadingRef} tabIndex={-1}>
                      {cleanupReviewState.message === 'detail'
                        ? catalog.cleanup.review.detailError
                        : cleanupReviewState.message === 'selection'
                          ? catalog.cleanup.review.selectionRequired
                          : catalog.cleanup.review.previewError}
                    </h3>
                    <button
                      type="button"
                      className="button-secondary"
                      onClick={() => invalidateCleanupReview(true)}
                    >
                      {catalog.cleanup.review.close}
                    </button>
                  </div>
                ) : null}

                {cleanupReviewState.kind === 'ready' ? (
                  <section
                    className="cleanup-review cleanup-preview-receipt"
                    aria-labelledby="cleanup-preview-title"
                    aria-live="polite"
                  >
                    <div className="cleanup-review-heading">
                      <div>
                        <h3 id="cleanup-preview-title" ref={cleanupReviewHeadingRef} tabIndex={-1}>
                          {catalog.cleanup.review.previewReady(cleanupReviewState.preview.plan_id)}
                        </h3>
                        <p>{catalog.cleanup.review.noExecution}</p>
                      </div>
                      <button
                        type="button"
                        className="button-secondary"
                        onClick={() => invalidateCleanupReview(true)}
                      >
                        {catalog.cleanup.review.close}
                      </button>
                    </div>
                    <dl className="cleanup-preview-facts">
                      <div>
                        <dt>{catalog.cleanup.review.expectedBytesLabel}</dt>
                        <dd>
                          {catalog.cleanup.review.expectedBytes(
                            cleanupReviewState.preview.expected_bytes,
                          )}
                        </dd>
                      </div>
                      <div>
                        <dt>{catalog.cleanup.review.journalLabel}</dt>
                        <dd>
                          {catalog.cleanup.review.journalSequence(
                            cleanupReviewState.preview.journal_sequence,
                          )}
                        </dd>
                      </div>
                      <div>
                        <dt>{catalog.cleanup.review.checksLabel}</dt>
                        <dd>
                          {catalog.cleanup.review.checksPassed(
                            cleanupReviewState.preview.policy.checks.length,
                          )}
                        </dd>
                      </div>
                    </dl>
                  </section>
                ) : null}
              </section>
            ) : null}

            {activeView === 'projects' ? (
              <>
                <section className="panel panel--projects" aria-labelledby="projects-title">
                  <div className="panel-heading panel-heading--wrap">
                    <div>
                      <p className="panel-kicker">{catalog.projects.kicker}</p>
                      <h2 id="projects-title">{catalog.projects.heading}</h2>
                      <p>{catalog.projects.description}</p>
                    </div>
                    <span className="connected-indicator">{catalog.projects.suggestionOnly}</span>
                  </div>
                  <div className="project-controls" aria-label={catalog.projects.controlsAria}>
                    <label htmlFor="project-scope">{catalog.projects.scopeLabel}</label>
                    <div className="scope-form-row">
                      <select
                        id="project-scope"
                        value={projectScopeId ?? ''}
                        disabled={
                          state.scopes.length === 0 ||
                          projectDiscoveryState.kind === 'loading' ||
                          projectReviewState.kind === 'saving'
                        }
                        onChange={(event) => {
                          chooseProjectScope(
                            event.target.value ? Number(event.target.value) : null,
                          );
                        }}
                      >
                        <option value="">{catalog.projects.chooseScope}</option>
                        {state.scopes.map((scope) => (
                          <option key={scope.id} value={scope.id}>
                            {catalog.search.authorizedScope(scope.id)}
                          </option>
                        ))}
                      </select>
                      <button
                        type="button"
                        disabled={
                          projectScopeId === null ||
                          projectReadinessState !== 'ready' ||
                          projectDiscoveryState.kind === 'loading' ||
                          projectReviewState.kind === 'saving'
                        }
                        onClick={() => void discoverProjectCandidates()}
                      >
                        {projectDiscoveryState.kind === 'loading'
                          ? catalog.projects.discovering
                          : catalog.projects.discover}
                      </button>
                    </div>
                  </div>
                  {state.scopes.length === 0 ? (
                    <p className="content-empty" role="status">
                      {catalog.projects.authorizationRequired}
                    </p>
                  ) : null}
                  {state.scopes.length > 0 && projectScopeId === null ? (
                    <p className="content-empty" role="status">
                      {catalog.projects.chooseScope}
                    </p>
                  ) : null}
                  {projectReadinessState === 'loading' ? (
                    <p className="content-empty" role="status" aria-live="polite">
                      {catalog.projects.checkingReadiness}
                    </p>
                  ) : null}
                  {projectReadinessState === 'scanRequired' ? (
                    <p className="content-empty" role="status">
                      {catalog.projects.scanRequired}
                    </p>
                  ) : null}
                  {projectReadinessState === 'error' ? (
                    <p className="content-empty project-message--error" role="alert">
                      {catalog.projects.readinessError}
                    </p>
                  ) : null}
                  {projectDiscoveryState.kind === 'loading' ? (
                    <p className="content-empty" role="status" aria-live="polite">
                      {catalog.projects.discovering}
                    </p>
                  ) : null}
                  {projectDiscoveryState.kind === 'error' ? (
                    <p className="content-empty project-message--error" role="alert">
                      {catalog.projects.error}
                    </p>
                  ) : null}
                  {projectDiscoveryState.kind === 'partial' ? (
                    <p className="content-empty" role="status">
                      {catalog.projects.partial}
                    </p>
                  ) : null}
                  {projectDiscoveryState.kind === 'ready' ||
                  projectDiscoveryState.kind === 'partial' ? (
                    projectDiscoveryState.discovery.candidates.length === 0 ? (
                      <p className="content-empty" role="status">
                        {catalog.projects.empty}
                      </p>
                    ) : (
                      <ol className="project-candidate-list" aria-label={catalog.projects.heading}>
                        {projectDiscoveryState.discovery.candidates.map((candidate) => (
                          <li key={candidate.project_id}>
                            <div>
                              <strong>{projectStateLabel(candidate.state, catalog)}</strong>
                              <span>
                                {catalog.projects.candidateMeta(
                                  candidate.confidence_basis_points,
                                  formatUtcDate(candidate.observed_at_unix_ms, locale),
                                )}
                              </span>
                            </div>
                            <button
                              type="button"
                              className="project-detail-open"
                              disabled={
                                projectReviewState.kind === 'loading' ||
                                projectReviewState.kind === 'saving'
                              }
                              onClick={(event) =>
                                void openProjectReview(candidate.project_id, event.currentTarget)
                              }
                            >
                              {catalog.projects.viewEvidence}
                            </button>
                          </li>
                        ))}
                      </ol>
                    )
                  ) : null}
                  {projectDiscoveryState.kind === 'ready' ||
                  projectDiscoveryState.kind === 'partial' ? (
                    <p className="content-empty project-verification">
                      {catalog.projects.noAutomaticMembership} {catalog.projects.noFileActions}
                    </p>
                  ) : null}
                  {projectReviewState.kind === 'loading' ? (
                    <p className="content-empty" role="status" aria-live="polite">
                      {catalog.projects.detail.loading}
                    </p>
                  ) : null}
                  {projectReviewState.kind === 'detail' || projectReviewState.kind === 'saving' ? (
                    <section className="project-review" aria-labelledby="project-review-title">
                      <div className="project-review-heading">
                        <div>
                          <h3 id="project-review-title" ref={projectReviewHeadingRef} tabIndex={-1}>
                            {projectStateLabel(projectReviewState.detail.candidate.state, catalog)}
                          </h3>
                          <p>{catalog.projects.detail.transientNotice}</p>
                        </div>
                        <button
                          type="button"
                          className="button-secondary"
                          disabled={projectReviewState.kind === 'saving'}
                          onClick={() => invalidateProjectReview(true)}
                        >
                          {catalog.projects.detail.close}
                        </button>
                      </div>
                      <dl className="project-evidence">
                        <div>
                          <dt>{catalog.projects.detail.rootLabel}</dt>
                          <dd>
                            <code>{projectReviewState.detail.candidate.display_path}</code>
                          </dd>
                        </div>
                        <div>
                          <dt>{catalog.projects.detail.signalsLabel}</dt>
                          <dd>
                            {projectReviewState.detail.candidate.suggestion.provenance
                              .map((signal) => signal.marker_name)
                              .join(' · ')}
                          </dd>
                        </div>
                      </dl>
                      {projectReviewState.kind === 'detail' && projectReviewState.decisionError ? (
                        <p className="project-message--error" role="alert">
                          {catalog.projects.detail.decisionError}
                        </p>
                      ) : null}
                      <div className="project-decision-actions">
                        <button
                          type="button"
                          disabled={projectReviewState.kind === 'saving'}
                          onClick={() => void submitProjectDecision('accepted')}
                        >
                          {projectReviewState.kind === 'saving' &&
                          projectReviewState.decision === 'accepted'
                            ? catalog.projects.detail.saving
                            : catalog.projects.detail.accept}
                        </button>
                        <button
                          type="button"
                          className="button-secondary"
                          disabled={projectReviewState.kind === 'saving'}
                          onClick={() => void submitProjectDecision('rejected')}
                        >
                          {projectReviewState.kind === 'saving' &&
                          projectReviewState.decision === 'rejected'
                            ? catalog.projects.detail.saving
                            : catalog.projects.detail.reject}
                        </button>
                      </div>
                    </section>
                  ) : null}
                  {projectReviewState.kind === 'error' ? (
                    <div className="project-review project-review--error" role="alert">
                      <h3 ref={projectReviewHeadingRef} tabIndex={-1}>
                        {catalog.projects.detail.detailError}
                      </h3>
                      <button
                        type="button"
                        className="button-secondary"
                        onClick={() => invalidateProjectReview(true)}
                      >
                        {catalog.projects.detail.close}
                      </button>
                    </div>
                  ) : null}
                </section>
                <section className="panel panel--scopes" aria-labelledby="scopes-title">
                  <div className="panel-heading panel-heading--wrap">
                    <div>
                      <p className="panel-kicker">{catalog.scope.kicker}</p>
                      <h2 id="scopes-title">{catalog.scope.heading}</h2>
                      <p>{catalog.scope.description}</p>
                    </div>
                    <span className="scope-count">{catalog.scope.count(state.scopes.length)}</span>
                  </div>

                  <div className="scope-form">
                    <div className="scope-form-row">
                      <button
                        type="button"
                        disabled={action.kind === 'working'}
                        aria-label={catalog.scope.inputLabel}
                        onClick={() => void authorizeRequestedScope()}
                      >
                        {catalog.scope.authorize}
                      </button>
                    </div>
                  </div>

                  {action.kind !== 'idle' ? (
                    <p
                      className={`action-message action-message--${action.kind}`}
                      role={action.kind === 'error' ? 'alert' : 'status'}
                    >
                      {actionMessageLabel(catalog, action.message)}
                    </p>
                  ) : null}

                  {state.scopes.length === 0 ? (
                    <div className="empty-scope">
                      <strong>{catalog.scope.emptyHeading}</strong>
                      <span>{catalog.scope.emptyDescription}</span>
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
                              <span className="scope-label">{catalog.scope.label(scope.id)}</span>
                              <code>{scope.display_path}</code>
                              {latestJob ? (
                                <div className="scan-progress" role="status">
                                  <span>{scanStatusLabel(latestJob, catalog)}</span>
                                  <span>
                                    {catalog.scope.progress(
                                      latestJob.processed_entries,
                                      latestJob.queued_entries,
                                      latestJob.issue_count,
                                    )}
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
                                {resumableJob.pause_requested
                                  ? catalog.scope.pausing
                                  : catalog.scope.pause}
                              </button>
                            ) : null}
                            {resumableJob?.status === 'paused' ||
                            resumableJob?.status === 'interrupted' ? (
                              <button type="button" onClick={() => void resume(resumableJob)}>
                                {catalog.scope.resume}
                              </button>
                            ) : null}
                            {!resumableJob ? (
                              <button
                                type="button"
                                disabled={action.kind === 'working'}
                                onClick={() => void scan(scope)}
                              >
                                {catalog.scope.scan}
                              </button>
                            ) : null}
                          </li>
                        );
                      })}
                    </ul>
                  )}
                </section>
              </>
            ) : null}

            {activeView === 'settings' ? (
              <section className="panel panel--settings" aria-labelledby="settings-title">
                <div className="panel-heading panel-heading--wrap">
                  <div>
                    <p className="panel-kicker">{catalog.runtime.kicker}</p>
                    <h2 id="settings-title">{catalog.navigation.views.settings.title}</h2>
                    <p>{catalog.navigation.views.settings.description}</p>
                  </div>
                  <span className="connected-indicator">{catalog.runtime.localOnly}</span>
                </div>
                <div className="settings-grid">
                  <div className="settings-block">
                    <span className="settings-label">{catalog.language.selectorLabel}</span>
                    <strong>
                      {localeOptions.find((option) => option.value === locale)?.label}
                    </strong>
                  </div>
                  <div className="settings-block">
                    <span className="settings-label">{catalog.runtime.platform}</span>
                    <strong>
                      {state.report.platform.os} · {state.report.platform.architecture}
                    </strong>
                  </div>
                  <div className="settings-block">
                    <span className="settings-label">{catalog.runtime.networkRequired}</span>
                    <strong>{catalog.runtime.no}</strong>
                  </div>
                  <div className="settings-block">
                    <span className="settings-label">{catalog.runtime.optionalLocalLlm}</span>
                    <strong>
                      {state.report.providers.local_llm.state === 'ready'
                        ? catalog.runtime.ready
                        : state.report.providers.local_llm.state === 'not_initialized'
                          ? catalog.runtime.lifecycle.notInitialized
                          : catalog.runtime.lifecycle.disabled}
                    </strong>
                  </div>
                </div>
                <ul className="privacy-list privacy-list--settings">
                  <li>
                    <span className="privacy-dot privacy-dot--on" aria-hidden="true" />
                    {catalog.navigation.localOnly}
                  </li>
                  <li>
                    <span className="privacy-dot privacy-dot--on" aria-hidden="true" />
                    {catalog.navigation.noNetwork}
                  </li>
                </ul>
              </section>
            ) : null}
          </div>
        ) : null}

        <footer>
          <span>
            {catalog.footer.version(state.kind === 'ready' ? state.report.app_version : '0.1.0')}
          </span>
          <span>{catalog.footer.description}</span>
        </footer>
      </main>
    </div>
  );
}
