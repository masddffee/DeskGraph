import { englishCount, formatIntegerForLocale, type Catalog } from './catalog';

export const en = {
  metadata: {
    title: 'DeskGraph — Pre-release',
    description: 'DeskGraph pre-release local-first computer context graph',
  },
  language: {
    selectorLabel: 'Display language',
  },
  navigation: {
    ariaLabel: 'Primary navigation',
    skipToContent: 'Skip to main content',
    brandDescription: 'Local-first computer context',
    localOnly: 'Local only',
    noNetwork: 'No network required',
    views: {
      home: {
        label: 'Home',
        title: 'Your local context, at a glance',
        description:
          'Review authorization, metadata scan, and local manifest status on this computer.',
      },
      search: {
        label: 'Search',
        title: 'Search local context',
        description:
          'Find current filenames and opt-in extracted text without sending a query away.',
      },
      projects: {
        label: 'Projects',
        title: 'Authorized folders and project suggestions',
        description:
          'Authorize folders, scan metadata, and review explainable local project suggestions.',
      },
      inbox: {
        label: 'Inbox',
        title: 'Review local suggestions',
        description:
          'Review current local cleanup suggestions. This inbox cannot change, trash, delete, or undo files.',
      },
      history: {
        label: 'History',
        title: 'Preview history',
        description:
          'Review path-free rename previews. This view does not execute or undo file operations.',
      },
      settings: {
        label: 'Settings',
        title: 'Local runtime and privacy',
        description:
          'Choose the display language and review local-only runtime and privacy boundaries.',
      },
    },
  },
  hero: {
    eyebrow: 'DeskGraph · M2 Local Context',
    heading: 'Graphify your computer.',
    description:
      'Confirm one or more local folders in one step, build their metadata manifests, and keep bounded text extraction on this computer—without uploading paths or content.',
    release: 'PRE-RELEASE',
  },
  journey: {
    kicker: 'A safe first pass',
    heading: 'See useful local context in three explicit steps.',
    description:
      'Choose the coverage, scan metadata, then review only the local suggestions you ask for. DeskGraph never starts with your whole computer.',
    privacy: 'No uploads · no automatic scan · no file changes',
    scope: {
      title: '1 · Choose coverage',
      description:
        'Select one or more folders in the native picker. Authorization alone does not read or scan them.',
      action: 'Choose local folders',
    },
    search: {
      title: '2 · Scan, then search',
      description:
        'Run an initial metadata scan for a chosen folder, then search filenames and opt-in extracted text in SQLite.',
      action: 'Open search',
    },
    review: {
      title: '3 · Review explained suggestions',
      description:
        'Inspect local project candidates and cleanup previews. Neither screen can change, trash, delete, or undo files.',
      projectsAction: 'Review projects',
      cleanupAction: 'Review cleanup',
    },
    mcp: {
      title: 'Read-only MCP is separate',
      description:
        'After a completed scan, the independently launched macOS/Linux local MCP slice can search only its launch-granted scopes. This Desktop screen does not configure it or expose write tools.',
    },
    status: {
      noScope: 'Choose coverage to begin',
      scopesReady: (count) =>
        `${count} authorized folder${count === 1 ? '' : 's'} · scanning stays explicit`,
      scanNeeded: 'Run an initial scan before search, Projects, or Cleanup can evaluate a folder',
      scanReady: 'A completed local scan is ready for search and review',
    },
  },
  loading: {
    heading: 'Opening the local manifest',
    description: 'No authorized folder is scanned automatically.',
  },
  backendError: {
    heading: 'Local manifest unavailable',
    description: 'The backend returned no validated status. Raw local errors and paths are hidden.',
    retry: 'Retry',
  },
  runtime: {
    kicker: 'Local runtime',
    heading: 'Manifest is ready',
    localOnly: 'Local only',
    platform: 'Platform',
    sqliteManifest: 'SQLite manifest',
    optionalLocalLlm: 'Optional local LLM',
    networkRequired: 'Network required',
    ready: 'Ready',
    no: 'No',
    lifecycle: { notInitialized: 'Not initialized', disabled: 'Disabled' },
  },
  manifest: {
    kicker: 'Current graph',
    emptyHeading: 'Nothing indexed yet',
    readyHeading: 'Metadata indexed',
    files: 'Files',
    folders: 'Folders',
    locations: 'Locations',
    scanIssues: 'Scan issues',
  },
  search: {
    kicker: 'Deterministic local search',
    heading: 'Find filenames and extracted text',
    description:
      'Traditional Chinese and English queries stay inside SQLite. Embeddings are off; every result says which local field matched.',
    mode: 'Lexical · offline',
    queryLabel: 'Search local context',
    queryPlaceholder: '專案脈絡 or project context',
    scopeAria: 'Search folder scope',
    allFolders: 'All authorized folders',
    authorizedScope: (scopeId) => `Authorized scope ${scopeId}`,
    search: 'Search',
    searching: 'Searching…',
    filtersAria: 'Bounded local search filters',
    sourceLabel: 'Match source',
    sources: {
      all: 'Paths + extracted text',
      paths: 'Filenames and paths only',
      extractedText: 'Extracted text only',
    },
    fileType: 'File type',
    fileTypePlaceholder: 'md or pdf',
    modifiedSince: 'Modified since (UTC)',
    modifiedBefore: 'Before (UTC, exclusive)',
    validation: {
      query: 'Enter at least 3 characters to keep local search bounded.',
      extension: 'File type must be a 1–16 character extension such as md, pdf, or docx.',
      dateRange: 'Choose a valid UTC date range where “Modified since” is before “Before”.',
      request: 'Search stopped safely. Try a shorter query or refresh the local manifest.',
    },
    empty: (query) => `No current path or active extracted text matched “${query}”.`,
    summary: (count, elapsedMs) =>
      `${englishCount(count, 'result')} · ${formatIntegerForLocale(elapsedMs, 'en')} ms`,
    filters: {
      scope: (scopeId) => `scope ${scopeId}`,
      pathsOnly: 'paths only',
      textOnly: 'extracted text only',
      since: (date) => `since ${date} UTC`,
      before: (date) => `before ${date} UTC`,
      allSources: 'all authorized local sources',
    },
    explanation: {
      filenameAndText: 'Exact filename + extracted text',
      filename: 'Exact filename',
      pathAndText: 'Path + extracted text',
      path: 'Filename or path',
      text: 'Extracted text',
    },
    ocr: {
      controlsAria: 'Local screenshot OCR controls',
      notRead: 'Screenshot text has not been read',
      description: 'Only this already-scanned screenshot is revalidated and read on this computer.',
      cancel: 'Cancel OCR',
      stopping: 'Stopping safely…',
      retryQueued: 'Retry queued OCR',
      resume: 'Resume screenshot OCR',
      readAgain: 'Read screenshot again',
      read: 'Read screenshot text locally',
      untrustedText: 'Untrusted local text',
      queued: 'Waiting to start',
      running: 'Extracting bounded text…',
      reading: 'Reading screenshot text locally…',
      completed: 'Screenshot text indexed locally',
      cancelled: 'Cancelled safely',
      interrupted: 'Interrupted safely',
      unavailable: 'Screenshot OCR unavailable or skipped safely',
      skipped: 'File skipped safely',
      capacity: 'Another local OCR is still finishing. This job remains queued; retry it safely.',
      providerUnavailable:
        'Screenshot OCR stopped safely. The local provider may be unavailable on this computer.',
      indexed: 'Screenshot text was indexed locally. Search again to find its contents.',
      cancelledFeedback: 'Screenshot OCR was cancelled safely. No partial text was published.',
      interruptedFeedback: 'Screenshot OCR was interrupted safely. Resume it to continue locally.',
      failedFeedback:
        'Screenshot OCR stopped safely. The previous complete local index was preserved.',
      denied:
        'OCR was denied safely. Rescan the file if it changed and confirm it is a supported screenshot.',
      resumeDenied: 'Resume was denied safely. Refresh the local manifest before trying again.',
      cancelDenied: 'The cancellation request could not be recorded safely.',
    },
  },
  actions: {
    kicker: 'Safe organization preview',
    heading: 'Review a rename without changing the file',
    description:
      'DeskGraph revalidates the selected scope, current manifest snapshot, file identity, read-only open handle, proposed name, and destination before it journals a preview.',
    previewOnly: 'Preview only · no execute',
    folderLabel: 'Authorized folder',
    chooseFolder: 'Choose a scanned folder',
    scopeOption: (scopeId, path) => `Scope ${scopeId} · ${path}`,
    sourceLabel: 'Current absolute file path',
    sourcePlaceholder: '/authorized/folder/draft.md',
    newNameLabel: 'Proposed filename only',
    newNamePlaceholder: 'final.md',
    validating: 'Validating safely…',
    createPreview: 'Create durable preview',
    validation: {
      chooseFolder: 'Choose the authorized folder first.',
      required: 'Enter the current absolute file path and one proposed filename.',
      denied:
        'Preview denied safely. Rescan a changed file, verify the authorized folder, and choose an unused portable filename.',
    },
    previewAria: 'Validated rename preview',
    plan: (planId) => `Plan ${planId} · validated preview`,
    caseOnly: 'Case-only rename requires a staged future executor.',
    direct: 'Direct rename strategy recorded in this durable preview.',
    unchanged: 'No file changed',
    before: 'Before',
    after: 'After',
    policyAria: 'Passed policy checks',
    policy: {
      authorizedScope: 'Inside the selected authorized folder',
      manifestFile: 'Current scanned file',
      canonicalSource: 'Canonical source stays in scope',
      sourceIdentity: 'Platform identity matches the manifest',
      readOnlyHandle: 'Read-only open handle matches',
      portableName: 'Portable one-part filename',
      sameParent: 'Same canonical parent folder',
      destinationScope: 'Destination stays in scope',
      destinationAvailable: 'Destination is available',
    },
    noExecute:
      'This Desktop screen creates and shows previews only. It exposes no Execute or Undo control.',
    historyHeading: 'Recent path-free action history',
    plans: (count) => englishCount(count, 'plan'),
    historyEmpty:
      'No organization plan has been journaled. This Desktop screen cannot execute or undo a plan.',
    historyPlan: (planId) => `Rename plan ${planId}`,
    historyScopeNode: (scopeId, nodeId) => `Scope ${scopeId} · node ${nodeId}`,
    caseOnlyStaged: 'Case-only staged',
    directStrategy: 'Direct',
    historyState: {
      previewed: 'Previewed',
      pending: 'Pending',
      executed: 'Executed',
      undone: 'Undone',
      needsAttention: 'Needs attention',
    },
  },
  watch: {
    kicker: 'Native event hints with safety reconciliation',
    heading: 'Native event hints + 5-minute safety reconciliation',
    description:
      'Native filesystem events are hints only. A five-minute periodic safety reconciliation also checks eligible, previously scanned folders in bounded batches. It does not promise incremental content refresh or a completion deadline.',
    adapterActive: 'Native hints + 5-minute safety reconciliation · active',
    adapterStarting: 'Native hints + 5-minute safety reconciliation · starting',
    adapterDegraded: 'Native hints or periodic-only fallback · needs attention',
    adapterStopped: 'Native hints + 5-minute safety reconciliation · stopped',
    metrics: {
      recent: 'Recent events',
      observed: 'Observed hints',
      reconciled: 'Reconciled',
      deferred: 'Deferred folders',
      attention: 'Needs attention',
    },
    empty:
      'No changes observed yet. Due, previously scanned folders are scheduled locally in bounded batches; newly authorized folders still require an explicit initial scan.',
    status: {
      stabilizing: 'Waiting for a stable snapshot',
      reconciling: 'Atomic manifest reconciliation',
      completed: 'Reconciled',
      noFailure: 'No failure',
    },
    reason: {
      temporary: 'Temporary download ignored',
      hidden: 'Hidden entry ignored',
      unsupported: 'Unsupported entry ignored',
      unavailable: 'Source unavailable',
      failed: 'Reconciliation failed safely',
    },
    event: (eventId, scopeId, hints) =>
      `Event ${eventId} · scope ${scopeId} · ${formatIntegerForLocale(hints, 'en')} coalesced hint${hints === 1 ? '' : 's'}`,
    scan: (jobId) => `Scan ${jobId}`,
    noScan: 'No scan yet',
  },
  cleanup: {
    kicker: 'Smart Cleanup Inbox',
    heading: 'Review local cleanup suggestions',
    description:
      'Refresh one authorized folder explicitly to review current duplicate, older-version, and screenshot-group evidence. This only reads local evidence.',
    suggestionOnly: 'Suggestions only · no file changes',
    scopeLabel: 'Authorized folder',
    chooseScope: 'Choose an authorized folder',
    refresh: 'Refresh suggestions',
    refreshing: 'Refreshing local evidence…',
    controlsAria: 'Smart Cleanup Inbox controls',
    authorizationRequired: 'Authorize a folder before requesting local cleanup suggestions.',
    empty: 'No current cleanup suggestions were found for this authorized folder.',
    partial: (notCurrent) =>
      notCurrent === 0
        ? 'This bounded refresh is partial. Some sources could not be evaluated safely.'
        : `This bounded refresh is partial. ${formatIntegerForLocale(notCurrent, 'en')} older suggestion${notCurrent === 1 ? '' : 's'} were excluded because their evidence is no longer current.`,
    error: 'Suggestions could not be refreshed safely. No file was changed.',
    exactDuplicate: 'Exact duplicate',
    version: 'Older version candidate',
    screenshotReviewGroup: 'Screenshot review group',
    exactDuplicateExplanation:
      'A fresh bounded full-byte comparison matched this pair. It does not choose a keeper or authorize cleanup.',
    versionExplanation:
      'An explicit numeric vN filename rule points from the lower to the higher number. It does not prove that the older file is disposable.',
    screenshotReviewGroupExplanation:
      'Matching dimensions and a ten-minute window with current OCR provenance only group these items for review. They do not prove screenshot origin, similar content, duplication, or disposability.',
    itemMeta: (members, evidenceScoreBasisPoints, observedAt) =>
      `${formatIntegerForLocale(members, 'en')} members · evidence score ${formatIntegerForLocale(evidenceScoreBasisPoints, 'en')} / 10,000 · observed ${observedAt}`,
    verification:
      'Opening a review revalidates current evidence before a durable Preview is created. This Inbox cannot confirm, move, trash, delete, restore, or undo files.',
    review: {
      open: 'Review files and create preview',
      loading: 'Revalidating this suggestion locally…',
      close: 'Close file review',
      transientNotice:
        'Paths are shown only for this explicit local review. They are not saved in the cleanup plan, history, logs, or language preference.',
      selectionLegend: 'Choose one cleanup target',
      selectTarget: 'Select as preview target',
      keeper: 'Kept as the comparison file',
      keeperSwitch: 'Will be kept · select to make this the target instead',
      noKeeper: 'No keeper is assigned for screenshot review. Each Preview binds one target only.',
      roles: {
        duplicateCandidate: 'Exact duplicate candidate',
        olderVersion: 'Older version · eligible target',
        newerVersion: 'Newer version · required keeper',
        screenshotCandidate: 'Screenshot-group member',
      },
      memberSize: (bytes) => `${formatIntegerForLocale(bytes, 'en')} bytes`,
      createPreview: 'Create durable preview',
      creatingPreview: 'Binding current file evidence…',
      selectionRequired: 'Choose one file to preview before continuing.',
      detailError: 'This suggestion is no longer safe to review. Refresh the Inbox and try again.',
      previewError:
        'The file or its evidence changed before the preview was sealed. No action was authorized.',
      previewReady: (planId) =>
        `Preview ${formatIntegerForLocale(planId, 'en')} was sealed locally`,
      expectedBytesLabel: 'Bound file size',
      expectedBytes: (bytes) =>
        `${formatIntegerForLocale(bytes, 'en')} bytes bound to this preview`,
      journalLabel: 'Journal',
      journalSequence: (sequence) =>
        `Immutable journal sequence ${formatIntegerForLocale(sequence, 'en')}`,
      checksLabel: 'Policy validation',
      checksPassed: (count) => `${formatIntegerForLocale(count, 'en')} policy checks recorded`,
      noExecution:
        'Preview only. This does not confirm, move, trash, delete, restore, or undo any file.',
    },
  },
  projects: {
    kicker: 'Project Discovery',
    heading: 'Review local project suggestions',
    description:
      'Explicitly evaluate one scanned authorized folder using deterministic local folder markers.',
    scopeLabel: 'Authorized folder',
    chooseScope: 'Choose an authorized folder',
    discover: 'Discover projects',
    discovering: 'Evaluating local folder evidence…',
    checkingReadiness: 'Checking durable scan readiness…',
    readinessError: 'This folder’s scan readiness could not be verified safely.',
    controlsAria: 'Project Discovery controls',
    authorizationRequired: 'Authorize a folder before requesting local project suggestions.',
    scanRequired: 'Complete a metadata scan for this folder before discovering projects.',
    empty: 'No current folder-marker project suggestions were found.',
    partial: 'This bounded evaluation is partial; some roots could not be evaluated safely.',
    error: 'Project suggestions could not be evaluated safely.',
    candidateMeta: (confidence, observedAt) =>
      `Evidence score ${formatIntegerForLocale(confidence, 'en')} / 10,000 · observed ${observedAt}`,
    viewEvidence: 'View evidence',
    suggestionOnly: 'Suggestions only',
    noAutomaticMembership: 'Accepting a root does not create file membership.',
    noFileActions: 'No file is moved, renamed, deleted, trashed, or restored.',
    state: {
      suggested: 'Suggested project root',
      accepted: 'Accepted project root',
      rejected: 'Rejected project root',
    },
    detail: {
      loading: 'Revalidating this project suggestion locally…',
      transientNotice:
        'This explicit local review shows the current path only. It is not included in ordinary lists or logs.',
      rootLabel: 'Current project root',
      signalsLabel: 'Observed folder markers',
      close: 'Close project review',
      accept: 'Accept project root',
      reject: 'Reject project root',
      saving: 'Saving your append-only decision…',
      decisionError: 'The decision could not be saved safely.',
      detailError: 'This project suggestion is no longer current. Refresh and try again.',
    },
  },
  extraction: {
    kicker: 'Bounded local content',
    emptyHeading: 'No file content extracted yet',
    readyHeading: 'Local text is ready',
    description:
      'Only already-scanned supported documents and explicitly selected screenshots are eligible. Every source is revalidated, output is size-limited, and a failed job cannot replace the last complete text.',
    neverUploaded: 'Never uploaded',
    metrics: {
      files: 'Files with text',
      chunks: 'Active chunks',
      completed: 'Completed jobs',
      skipped: 'Skipped or cancelled',
    },
    latest: (operation, jobId) => `Latest ${operation} job ${jobId}`,
    operation: { screenshotOcr: 'Screenshot OCR', content: 'content' },
    progress: (chunks, bytes) =>
      `${englishCount(chunks, 'chunk')} · ${englishCount(bytes, 'byte')}`,
    optInEmpty: 'Extraction is opt-in. Authorizing or scanning a folder never reads file contents.',
  },
  scope: {
    kicker: 'Explicit authorization',
    heading: 'Folders DeskGraph may inspect',
    description:
      'Choose one or more folders together in the system picker. DeskGraph accesses only the folders you confirm. Authorization and scanning are separate actions; selection never starts a scan, and symlinks or hidden entries are not followed.',
    count: (count) => `${formatIntegerForLocale(count, 'en')} authorized`,
    pickerAriaLabel: 'Choose one or more folders with the system picker',
    authorize: 'Choose folders to authorize',
    emptyHeading: 'No folder access',
    emptyDescription: 'DeskGraph cannot inspect Desktop, Downloads, or Documents until added here.',
    label: (scopeId) => `Authorized scope ${scopeId}`,
    progress: (processed, queued, issues) =>
      `${formatIntegerForLocale(processed, 'en')} / ${englishCount(queued, 'entry')} · ${englishCount(issues, 'issue')}`,
    pause: 'Pause scan',
    pausing: 'Pausing…',
    resume: 'Resume scan',
    scan: 'Scan metadata',
    validation: {
      cancelled: 'Folder selection cancelled. No authorization change was committed.',
      validating: 'Waiting for your folder choices, then validating every folder boundary…',
      authorized: (count) =>
        `${englishCount(count, 'folder')} authorized together. Nothing was scanned; choose Scan metadata for each folder when ready.`,
      refreshFailed: (count) =>
        `${englishCount(count, 'folder')} authorized and not scanned, but the latest local status could not be refreshed. Reload to reconcile the view.`,
      denied:
        'The selected folders could not be authorized as one set. No selected change was committed; remove unavailable, protected, duplicate, or overlapping folders and try again.',
      reading: 'Reading metadata inside the authorized folder…',
      complete: (files, folders) =>
        `Scan complete: ${englishCount(files, 'file')} and ${englishCount(folders, 'folder')}.`,
      paused: (processed, queued) =>
        `Scan paused after ${formatIntegerForLocale(processed, 'en')} of ${englishCount(queued, 'discovered entry')}.`,
      interrupted:
        'The scan was interrupted safely. Resume it after checking the authorized folder.',
      stopped:
        'The metadata scan stopped safely. Existing manifest data was not partially replaced.',
      creating: 'Creating a durable local scan job…',
      startDenied: 'A new scan could not start. Resume the existing job if this folder has one.',
      waiting: 'Waiting for the current metadata entry to finish…',
      pauseDenied: 'The pause request could not be recorded safely.',
      revalidating: 'Revalidating the authorized folder boundary…',
      resumeDenied: 'Resume was denied because the job or authorized folder is no longer valid.',
    },
    status: {
      pausing: 'Pausing safely…',
      scanning: 'Scanning metadata…',
      paused: 'Paused',
      interrupted: 'Interrupted safely',
      completed: 'Completed',
      stopped: 'Stopped with an error',
    },
  },
  hardExclusion: {
    kicker: 'Coverage privacy',
    heading: 'Hard exclusions',
    description:
      'Hard exclusions deny local access and indexing; they are not just hidden search results.',
    scopeLabel: 'Authorized root',
    chooseScope: 'Choose an authorized root',
    loading: 'Loading current policy…',
    policyRevision: (revision) => `Policy revision ${revision}`,
    addFolders: 'Add folders…',
    addFiles: 'Add files…',
    noExclusions: 'No hard exclusions in this root.',
    currentExclusions: 'Current hard exclusions',
    removalUnavailable: 'Removing a hard exclusion is not available in this release slice.',
    previewHeading: 'Review hard exclusion',
    previewNotice:
      'DeskGraph will clear affected local index data. It will not move or delete source files.',
    previewEmpty: 'The picker was cancelled; no policy change was made.',
    notConfirmable:
      'This exclusion cannot be confirmed until current file operations or recoverable journal work finish. Source files have not changed.',
    item: {
      willAdd: 'Will exclude',
      alreadyExcluded: 'Already excluded',
      coveredByParent: 'Covered by selected folder',
      file: 'File',
      folder: 'Folder',
    },
    impact: (locations, content, graph, candidates, jobs, actions) =>
      `${locations} locations, ${content} content/OCR chunks, ${graph} graph facts, ${candidates} derived candidates, ${jobs} pending jobs, and ${actions} action safety records that block this change are affected locally.`,
    confirm: 'Confirm exclusion and clear local index',
    confirming: 'Confirming privacy purge…',
    cancel: 'Cancel preview',
    cancelled: 'Preview cancelled; no policy change was made.',
    committed: (count) =>
      `${count} hard exclusion${count === 1 ? '' : 's'} saved and affected local index data cleared.`,
    refreshFailed: (count) =>
      `${count} hard exclusion${count === 1 ? '' : 's'} saved and local data cleared, but refresh failed.`,
    error: 'Hard exclusion was not confirmed. No local privacy purge is reported.',
    sourceSafe: 'Source files will not be moved, changed, or deleted.',
  },
  footer: {
    version: (version) => `DeskGraph ${version}`,
    description: 'Metadata + bounded local text · No uploads · No file operations',
  },
} satisfies Catalog;
