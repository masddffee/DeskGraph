export type Catalog = {
  metadata: { title: string; description: string };
  language: { selectorLabel: string };
  navigation: {
    ariaLabel: string;
    skipToContent: string;
    brandDescription: string;
    localOnly: string;
    noNetwork: string;
    views: {
      home: { label: string; title: string; description: string };
      search: { label: string; title: string; description: string };
      projects: { label: string; title: string; description: string };
      inbox: { label: string; title: string; description: string };
      history: { label: string; title: string; description: string };
      settings: { label: string; title: string; description: string };
    };
  };
  hero: {
    eyebrow: string;
    heading: string;
    description: string;
    release: string;
  };
  loading: { heading: string; description: string };
  backendError: { heading: string; description: string; retry: string };
  runtime: {
    kicker: string;
    heading: string;
    localOnly: string;
    platform: string;
    sqliteManifest: string;
    optionalLocalLlm: string;
    networkRequired: string;
    ready: string;
    no: string;
    lifecycle: { notInitialized: string; disabled: string };
  };
  manifest: {
    kicker: string;
    emptyHeading: string;
    readyHeading: string;
    files: string;
    folders: string;
    locations: string;
    scanIssues: string;
  };
  search: {
    kicker: string;
    heading: string;
    description: string;
    mode: string;
    queryLabel: string;
    queryPlaceholder: string;
    scopeAria: string;
    allFolders: string;
    authorizedScope: (scopeId: number) => string;
    search: string;
    searching: string;
    filtersAria: string;
    sourceLabel: string;
    sources: { all: string; paths: string; extractedText: string };
    fileType: string;
    fileTypePlaceholder: string;
    modifiedSince: string;
    modifiedBefore: string;
    validation: { query: string; extension: string; dateRange: string; request: string };
    empty: (query: string) => string;
    summary: (count: number, elapsedMs: number) => string;
    filters: {
      scope: (scopeId: number) => string;
      pathsOnly: string;
      textOnly: string;
      since: (date: string) => string;
      before: (date: string) => string;
      allSources: string;
    };
    explanation: {
      filenameAndText: string;
      filename: string;
      pathAndText: string;
      path: string;
      text: string;
    };
    ocr: {
      controlsAria: string;
      notRead: string;
      description: string;
      cancel: string;
      stopping: string;
      retryQueued: string;
      resume: string;
      readAgain: string;
      read: string;
      untrustedText: string;
      queued: string;
      running: string;
      reading: string;
      completed: string;
      cancelled: string;
      interrupted: string;
      unavailable: string;
      skipped: string;
      capacity: string;
      providerUnavailable: string;
      indexed: string;
      cancelledFeedback: string;
      interruptedFeedback: string;
      failedFeedback: string;
      denied: string;
      resumeDenied: string;
      cancelDenied: string;
    };
  };
  actions: {
    kicker: string;
    heading: string;
    description: string;
    previewOnly: string;
    folderLabel: string;
    chooseFolder: string;
    scopeOption: (scopeId: number, path: string) => string;
    sourceLabel: string;
    sourcePlaceholder: string;
    newNameLabel: string;
    newNamePlaceholder: string;
    validating: string;
    createPreview: string;
    validation: { chooseFolder: string; required: string; denied: string };
    previewAria: string;
    plan: (planId: number) => string;
    caseOnly: string;
    direct: string;
    unchanged: string;
    before: string;
    after: string;
    policyAria: string;
    policy: {
      authorizedScope: string;
      manifestFile: string;
      canonicalSource: string;
      sourceIdentity: string;
      readOnlyHandle: string;
      portableName: string;
      sameParent: string;
      destinationScope: string;
      destinationAvailable: string;
    };
    noExecute: string;
    historyHeading: string;
    plans: (count: number) => string;
    historyEmpty: string;
    historyPlan: (planId: number) => string;
    historyScopeNode: (scopeId: number, nodeId: number) => string;
    caseOnlyStaged: string;
    directStrategy: string;
    historyState: {
      previewed: string;
      pending: string;
      executed: string;
      undone: string;
      needsAttention: string;
    };
  };
  watch: {
    kicker: string;
    heading: string;
    description: string;
    adapterActive: string;
    adapterStarting: string;
    adapterDegraded: string;
    adapterStopped: string;
    metrics: {
      recent: string;
      observed: string;
      reconciled: string;
      deferred: string;
      attention: string;
    };
    empty: string;
    status: { stabilizing: string; reconciling: string; completed: string; noFailure: string };
    reason: {
      temporary: string;
      hidden: string;
      unsupported: string;
      unavailable: string;
      failed: string;
    };
    event: (eventId: number, scopeId: number, hints: number) => string;
    scan: (jobId: number) => string;
    noScan: string;
  };
  cleanup: {
    kicker: string;
    heading: string;
    description: string;
    suggestionOnly: string;
    scopeLabel: string;
    chooseScope: string;
    refresh: string;
    refreshing: string;
    controlsAria: string;
    authorizationRequired: string;
    empty: string;
    partial: (notCurrent: number) => string;
    error: string;
    exactDuplicate: string;
    version: string;
    screenshotReviewGroup: string;
    exactDuplicateExplanation: string;
    versionExplanation: string;
    screenshotReviewGroupExplanation: string;
    itemMeta: (members: number, evidenceScoreBasisPoints: number, observedAt: string) => string;
    verification: string;
    review: {
      open: string;
      loading: string;
      close: string;
      transientNotice: string;
      selectionLegend: string;
      selectTarget: string;
      keeper: string;
      keeperSwitch: string;
      noKeeper: string;
      roles: {
        duplicateCandidate: string;
        olderVersion: string;
        newerVersion: string;
        screenshotCandidate: string;
      };
      memberSize: (bytes: number) => string;
      createPreview: string;
      creatingPreview: string;
      selectionRequired: string;
      detailError: string;
      previewError: string;
      previewReady: (planId: number) => string;
      expectedBytesLabel: string;
      expectedBytes: (bytes: number) => string;
      journalLabel: string;
      journalSequence: (sequence: number) => string;
      checksLabel: string;
      checksPassed: (count: number) => string;
      noExecution: string;
    };
  };
  projects: {
    kicker: string;
    heading: string;
    description: string;
    scopeLabel: string;
    chooseScope: string;
    discover: string;
    discovering: string;
    checkingReadiness: string;
    readinessError: string;
    controlsAria: string;
    authorizationRequired: string;
    scanRequired: string;
    empty: string;
    partial: string;
    error: string;
    candidateMeta: (confidence: number, observedAt: string) => string;
    viewEvidence: string;
    suggestionOnly: string;
    noAutomaticMembership: string;
    noFileActions: string;
    state: { suggested: string; accepted: string; rejected: string };
    detail: {
      loading: string;
      transientNotice: string;
      rootLabel: string;
      signalsLabel: string;
      close: string;
      accept: string;
      reject: string;
      saving: string;
      decisionError: string;
      detailError: string;
    };
  };
  extraction: {
    kicker: string;
    emptyHeading: string;
    readyHeading: string;
    description: string;
    neverUploaded: string;
    metrics: { files: string; chunks: string; completed: string; skipped: string };
    latest: (operation: string, jobId: number) => string;
    operation: { screenshotOcr: string; content: string };
    progress: (chunks: number, bytes: number) => string;
    optInEmpty: string;
  };
  scope: {
    kicker: string;
    heading: string;
    description: string;
    count: (count: number) => string;
    pickerAriaLabel: string;
    authorize: string;
    emptyHeading: string;
    emptyDescription: string;
    label: (scopeId: number) => string;
    progress: (processed: number, queued: number, issues: number) => string;
    pause: string;
    pausing: string;
    resume: string;
    scan: string;
    validation: {
      cancelled: string;
      validating: string;
      authorized: (count: number) => string;
      refreshFailed: (count: number) => string;
      denied: string;
      reading: string;
      complete: (files: number, folders: number) => string;
      paused: (processed: number, queued: number) => string;
      interrupted: string;
      stopped: string;
      creating: string;
      startDenied: string;
      waiting: string;
      pauseDenied: string;
      revalidating: string;
      resumeDenied: string;
    };
    status: {
      pausing: string;
      scanning: string;
      paused: string;
      interrupted: string;
      completed: string;
      stopped: string;
    };
  };
  hardExclusion: {
    kicker: string;
    heading: string;
    description: string;
    scopeLabel: string;
    chooseScope: string;
    loading: string;
    policyRevision: (revision: number) => string;
    addFolders: string;
    addFiles: string;
    noExclusions: string;
    currentExclusions: string;
    removalUnavailable: string;
    previewHeading: string;
    previewNotice: string;
    previewEmpty: string;
    notConfirmable: string;
    item: {
      willAdd: string;
      alreadyExcluded: string;
      coveredByParent: string;
      file: string;
      folder: string;
    };
    impact: (
      locations: number,
      content: number,
      graph: number,
      candidates: number,
      jobs: number,
      actions: number,
    ) => string;
    confirm: string;
    confirming: string;
    cancel: string;
    cancelled: string;
    committed: (count: number) => string;
    refreshFailed: (count: number) => string;
    error: string;
    sourceSafe: string;
  };
  footer: { version: (version: string) => string; description: string };
};

export function formatIntegerForLocale(value: number, locale: string): string {
  return new Intl.NumberFormat(locale).format(value);
}

export function englishCount(value: number, singular: string, plural = `${singular}s`): string {
  return `${formatIntegerForLocale(value, 'en')} ${value === 1 ? singular : plural}`;
}
