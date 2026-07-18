export const LOCALES = ['en', 'zh-TW'] as const;

export type Locale = (typeof LOCALES)[number];

export interface LocaleStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export const LOCALE_STORAGE_KEY = 'deskgraph.locale';

export function englishCount(value: number, singular: string, plural = `${singular}s`): string {
  return `${formatInteger(value, 'en')} ${value === 1 ? singular : plural}`;
}

export type Catalog = {
  metadata: { htmlLang: string; title: string; description: string };
  language: { selectorLabel: string; english: string; traditionalChinese: string };
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
    directPreviewed: string;
  };
  watch: {
    kicker: string;
    heading: string;
    description: string;
    adapterPending: string;
    metrics: { recent: string; observed: string; reconciled: string; attention: string };
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
    inputLabel: string;
    placeholder: string;
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
      required: string;
      validating: string;
      authorized: string;
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
  footer: { version: (version: string) => string; description: string };
};

const en = {
  metadata: {
    htmlLang: 'en',
    title: 'DeskGraph — Pre-release',
    description: 'DeskGraph pre-release local-first computer context graph',
  },
  language: {
    selectorLabel: 'Display language',
    english: 'English',
    traditionalChinese: '繁體中文',
  },
  hero: {
    eyebrow: 'DeskGraph · M2 Local Context',
    heading: 'Graphify your computer.',
    description:
      'Authorize one local folder at a time, build its metadata manifest, and keep bounded text extraction on this computer—without uploading paths or content.',
    release: 'PRE-RELEASE',
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
      `${englishCount(count, 'result')} · ${formatInteger(elapsedMs, 'en')} ms`,
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
    direct: 'Direct rename strategy recorded for a future executor.',
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
      'This plan is journaled but cannot execute. Recovery and Undo do not exist yet, so DeskGraph exposes no action button.',
    historyHeading: 'Recent path-free preview history',
    plans: (count) => englishCount(count, 'plan'),
    historyEmpty:
      'No organization preview has been journaled. Files cannot be changed from this app.',
    historyPlan: (planId) => `Rename preview ${planId}`,
    historyScopeNode: (scopeId, nodeId) => `Scope ${scopeId} · node ${nodeId}`,
    caseOnlyStaged: 'Case-only staged',
    directPreviewed: 'Direct · previewed',
  },
  watch: {
    kicker: 'Durable watch reconciliation',
    heading: 'Stable hints, atomic manifest updates',
    description:
      'The local core can debounce path-free event states, reject temporary downloads, and resume reconciliation after restart. The native OS event adapter and automatic content re-indexing are not connected yet.',
    adapterPending: 'Core ready · adapter pending',
    metrics: {
      recent: 'Recent events',
      observed: 'Observed hints',
      reconciled: 'Reconciled',
      attention: 'Needs attention',
    },
    empty: 'No event source is enabled. Files are still updated only by an explicit scan.',
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
      `Event ${eventId} · scope ${scopeId} · ${formatInteger(hints, 'en')} coalesced hint${hints === 1 ? '' : 's'}`,
    scan: (jobId) => `Scan ${jobId}`,
    noScan: 'No scan yet',
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
      'Enter an existing folder path. Authorization and scanning are separate actions; symlinks and hidden entries are not followed.',
    count: (count) => `${formatInteger(count, 'en')} authorized`,
    inputLabel: 'Folder path',
    placeholder: '/Users/you/Documents or C:\\Users\\you\\Documents',
    authorize: 'Authorize folder',
    emptyHeading: 'No folder access',
    emptyDescription: 'DeskGraph cannot inspect Desktop, Downloads, or Documents until added here.',
    label: (scopeId) => `Authorized scope ${scopeId}`,
    progress: (processed, queued, issues) =>
      `${formatInteger(processed, 'en')} / ${englishCount(queued, 'entry')} · ${englishCount(issues, 'issue')}`,
    pause: 'Pause scan',
    pausing: 'Pausing…',
    resume: 'Resume scan',
    scan: 'Scan metadata',
    validation: {
      required: 'Enter an existing folder path first.',
      validating: 'Validating the folder boundary…',
      authorized: 'Folder authorized. Nothing was scanned until you choose Scan metadata.',
      denied: 'The folder could not be authorized. Check that it exists and is not protected.',
      reading: 'Reading metadata inside the authorized folder…',
      complete: (files, folders) =>
        `Scan complete: ${englishCount(files, 'file')} and ${englishCount(folders, 'folder')}.`,
      paused: (processed, queued) =>
        `Scan paused after ${formatInteger(processed, 'en')} of ${englishCount(queued, 'discovered entry')}.`,
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
  footer: {
    version: (version) => `DeskGraph ${version}`,
    description: 'Metadata + bounded local text · No uploads · No file operations',
  },
} satisfies Catalog;

const zhTW = {
  metadata: {
    htmlLang: 'zh-TW',
    title: 'DeskGraph — 預先發行版',
    description: 'DeskGraph 預先發行版：本機優先的電腦脈絡圖譜',
  },
  language: {
    selectorLabel: '顯示語言',
    english: 'English',
    traditionalChinese: '繁體中文',
  },
  hero: {
    eyebrow: 'DeskGraph · M2 本機脈絡',
    heading: '讓你的電腦脈絡化。',
    description:
      '一次明確授權一個本機資料夾，建立其中繼資料清單，並將受限文字抽取留在這台電腦上，不上傳路徑或內容。',
    release: '預先發行版',
  },
  loading: { heading: '正在開啟本機清單', description: '不會自動掃描任何已授權資料夾。' },
  backendError: {
    heading: '本機清單無法使用',
    description: '後端未回傳已驗證的狀態。原始本機錯誤與路徑已隱藏。',
    retry: '重試',
  },
  runtime: {
    kicker: '本機執行環境',
    heading: '清單已就緒',
    localOnly: '僅限本機',
    platform: '平台',
    sqliteManifest: 'SQLite 清單',
    optionalLocalLlm: '選用的本機 LLM',
    networkRequired: '需要網路',
    ready: '就緒',
    no: '否',
    lifecycle: { notInitialized: '尚未初始化', disabled: '已停用' },
  },
  manifest: {
    kicker: '目前圖譜',
    emptyHeading: '尚未建立索引',
    readyHeading: '中繼資料已建立索引',
    files: '檔案',
    folders: '資料夾',
    locations: '位置',
    scanIssues: '掃描問題',
  },
  search: {
    kicker: '可預測的本機搜尋',
    heading: '尋找檔名與已抽取文字',
    description:
      '繁體中文與英文查詢都留在 SQLite 內。嵌入功能已關閉；每筆結果都會說明命中的本機欄位。',
    mode: '字面比對 · 離線',
    queryLabel: '搜尋本機脈絡',
    queryPlaceholder: '專案脈絡或 project context',
    scopeAria: '搜尋資料夾範圍',
    allFolders: '所有已授權資料夾',
    authorizedScope: (scopeId) => `已授權範圍 ${scopeId}`,
    search: '搜尋',
    searching: '正在搜尋…',
    filtersAria: '受限的本機搜尋篩選條件',
    sourceLabel: '比對來源',
    sources: { all: '路徑 + 已抽取文字', paths: '僅檔名與路徑', extractedText: '僅已抽取文字' },
    fileType: '檔案類型',
    fileTypePlaceholder: 'md 或 pdf',
    modifiedSince: '修改時間起點（UTC）',
    modifiedBefore: '修改時間終點（UTC，不含）',
    validation: {
      query: '請至少輸入 3 個字元，讓本機搜尋維持受限。',
      extension: '檔案類型必須是 1–16 個字元的副檔名，例如 md、pdf 或 docx。',
      dateRange: '請選擇有效的 UTC 日期範圍，且「修改時間起點」必須早於「修改時間終點」。',
      request: '搜尋已安全停止。請嘗試較短的查詢，或重新整理本機清單。',
    },
    empty: (query) => `目前路徑或有效的已抽取文字沒有符合「${query}」的結果。`,
    summary: (count, elapsedMs) =>
      `${formatInteger(count, 'zh-TW')} 筆結果 · ${formatInteger(elapsedMs, 'zh-TW')} 毫秒`,
    filters: {
      scope: (scopeId) => `範圍 ${scopeId}`,
      pathsOnly: '僅路徑',
      textOnly: '僅已抽取文字',
      since: (date) => `自 ${date} UTC 起`,
      before: (date) => `${date} UTC 前`,
      allSources: '所有已授權本機來源',
    },
    explanation: {
      filenameAndText: '精確檔名 + 已抽取文字',
      filename: '精確檔名',
      pathAndText: '路徑 + 已抽取文字',
      path: '檔名或路徑',
      text: '已抽取文字',
    },
    ocr: {
      controlsAria: '本機截圖 OCR 控制項',
      notRead: '尚未讀取截圖文字',
      description: '只會重新驗證並在這台電腦上讀取這張已掃描截圖。',
      cancel: '取消 OCR',
      stopping: '正在安全停止…',
      retryQueued: '重試排隊中的 OCR',
      resume: '繼續截圖 OCR',
      readAgain: '再次讀取截圖',
      read: '在本機讀取截圖文字',
      untrustedText: '未受信任的本機文字',
      queued: '等待開始',
      running: '正在抽取受限文字…',
      reading: '正在本機讀取截圖文字…',
      completed: '截圖文字已在本機建立索引',
      cancelled: '已安全取消',
      interrupted: '已安全中斷',
      unavailable: '截圖 OCR 無法使用或已安全略過',
      skipped: '檔案已安全略過',
      capacity: '另一項本機 OCR 仍在完成中。此工作會維持排隊；請安全地重試。',
      providerUnavailable: '截圖 OCR 已安全停止。此電腦上的本機提供者可能無法使用。',
      indexed: '截圖文字已在本機建立索引。請再次搜尋以找到內容。',
      cancelledFeedback: '截圖 OCR 已安全取消。未發布任何部分文字。',
      interruptedFeedback: '截圖 OCR 已安全中斷。請繼續以在本機完成。',
      failedFeedback: '截圖 OCR 已安全停止。先前完整的本機索引已保留。',
      denied: 'OCR 已安全拒絕。若檔案已變更，請重新掃描並確認它是支援的截圖。',
      resumeDenied: '繼續操作已安全拒絕。請重新整理本機清單後再試一次。',
      cancelDenied: '無法安全記錄取消要求。',
    },
  },
  actions: {
    kicker: '安全整理預覽',
    heading: '先檢視重新命名，不變更檔案',
    description:
      'DeskGraph 會在寫入預覽紀錄前，重新驗證所選範圍、目前清單快照、檔案身分、唯讀開啟控制代碼、建議名稱與目的地。',
    previewOnly: '僅預覽 · 不執行',
    folderLabel: '已授權資料夾',
    chooseFolder: '選擇已掃描的資料夾',
    scopeOption: (scopeId, path) => `範圍 ${scopeId} · ${path}`,
    sourceLabel: '目前絕對檔案路徑',
    sourcePlaceholder: '/authorized/folder/draft.md',
    newNameLabel: '僅輸入建議檔名',
    newNamePlaceholder: 'final.md',
    validating: '正在安全驗證…',
    createPreview: '建立可持久化預覽',
    validation: {
      chooseFolder: '請先選擇已授權資料夾。',
      required: '請輸入目前絕對檔案路徑與一個建議檔名。',
      denied: '預覽已安全拒絕。請重新掃描已變更檔案、確認已授權資料夾，並選擇未使用且可攜的檔名。',
    },
    previewAria: '已驗證的重新命名預覽',
    plan: (planId) => `計畫 ${planId} · 已驗證預覽`,
    caseOnly: '僅大小寫的重新命名需要未來分階段執行器。',
    direct: '已記錄直接重新命名策略，供未來執行器使用。',
    unchanged: '未變更任何檔案',
    before: '變更前',
    after: '變更後',
    policyAria: '已通過的政策檢查',
    policy: {
      authorizedScope: '位於選取的已授權資料夾內',
      manifestFile: '目前已掃描檔案',
      canonicalSource: '正規化來源仍在範圍內',
      sourceIdentity: '平台身分符合清單',
      readOnlyHandle: '唯讀開啟控制代碼相符',
      portableName: '可攜式單一部分檔名',
      sameParent: '相同的正規化父資料夾',
      destinationScope: '目的地仍在範圍內',
      destinationAvailable: '目的地可用',
    },
    noExecute: '此計畫已寫入日誌但無法執行。復原與還原尚未存在，因此 DeskGraph 不提供操作按鈕。',
    historyHeading: '最近不含路徑的預覽紀錄',
    plans: (count) => `${formatInteger(count, 'zh-TW')} 個計畫`,
    historyEmpty: '尚未寫入任何整理預覽。此應用程式無法變更檔案。',
    historyPlan: (planId) => `重新命名預覽 ${planId}`,
    historyScopeNode: (scopeId, nodeId) => `範圍 ${scopeId} · 節點 ${nodeId}`,
    caseOnlyStaged: '僅大小寫，已分階段',
    directPreviewed: '直接 · 已預覽',
  },
  watch: {
    kicker: '可持久化監看協調',
    heading: '穩定提示，原子化更新清單',
    description:
      '本機核心可防抖不含路徑的事件狀態、拒絕暫存下載，並在重新啟動後繼續協調。原生 OS 事件轉接器與自動重新建立內容索引尚未連接。',
    adapterPending: '核心已就緒 · 轉接器待完成',
    metrics: {
      recent: '最近事件',
      observed: '觀察到的提示',
      reconciled: '已協調',
      attention: '需要注意',
    },
    empty: '未啟用事件來源。檔案仍只會由明確掃描更新。',
    status: {
      stabilizing: '等待穩定快照',
      reconciling: '原子化清單協調',
      completed: '已協調',
      noFailure: '沒有失敗',
    },
    reason: {
      temporary: '已忽略暫存下載',
      hidden: '已忽略隱藏項目',
      unsupported: '已忽略不支援項目',
      unavailable: '來源無法使用',
      failed: '協調已安全失敗',
    },
    event: (eventId, scopeId, hints) =>
      `事件 ${eventId} · 範圍 ${scopeId} · ${formatInteger(hints, 'zh-TW')} 個合併提示`,
    scan: (jobId) => `掃描 ${jobId}`,
    noScan: '尚未掃描',
  },
  extraction: {
    kicker: '受限的本機內容',
    emptyHeading: '尚未抽取任何檔案內容',
    readyHeading: '本機文字已就緒',
    description:
      '只有已掃描且支援的文件與明確選取的截圖符合資格。每個來源都會重新驗證，輸出受大小限制，失敗工作無法取代上次完整文字。',
    neverUploaded: '絕不上傳',
    metrics: {
      files: '含文字的檔案',
      chunks: '有效區塊',
      completed: '已完成工作',
      skipped: '已略過或取消',
    },
    latest: (operation, jobId) => `最新 ${operation} 工作 ${jobId}`,
    operation: { screenshotOcr: '截圖 OCR', content: '內容' },
    progress: (chunks, bytes) =>
      `${formatInteger(chunks, 'zh-TW')} 個區塊 · ${formatInteger(bytes, 'zh-TW')} 位元組`,
    optInEmpty: '內容抽取需明確選擇。授權或掃描資料夾絕不會讀取檔案內容。',
  },
  scope: {
    kicker: '明確授權',
    heading: 'DeskGraph 可檢查的資料夾',
    description: '請輸入既有資料夾路徑。授權與掃描是分開的操作；不會追蹤符號連結或隱藏項目。',
    count: (count) => `${formatInteger(count, 'zh-TW')} 個已授權`,
    inputLabel: '資料夾路徑',
    placeholder: '/Users/you/Documents 或 C:\\Users\\you\\Documents',
    authorize: '授權資料夾',
    emptyHeading: '沒有資料夾存取權',
    emptyDescription: '在這裡新增前，DeskGraph 無法檢查桌面、下載項目或文件。',
    label: (scopeId) => `已授權範圍 ${scopeId}`,
    progress: (processed, queued, issues) =>
      `${formatInteger(processed, 'zh-TW')} / ${formatInteger(queued, 'zh-TW')} 個項目 · ${formatInteger(issues, 'zh-TW')} 個問題`,
    pause: '暫停掃描',
    pausing: '正在暫停…',
    resume: '繼續掃描',
    scan: '掃描中繼資料',
    validation: {
      required: '請先輸入既有資料夾路徑。',
      validating: '正在驗證資料夾邊界…',
      authorized: '資料夾已授權。在你選擇「掃描中繼資料」前不會掃描任何內容。',
      denied: '無法授權此資料夾。請確認它存在且未受保護。',
      reading: '正在讀取已授權資料夾中的中繼資料…',
      complete: (files, folders) =>
        `掃描完成：${formatInteger(files, 'zh-TW')} 個檔案與 ${formatInteger(folders, 'zh-TW')} 個資料夾。`,
      paused: (processed, queued) =>
        `掃描已在發現 ${formatInteger(queued, 'zh-TW')} 個項目中的 ${formatInteger(processed, 'zh-TW')} 個後暫停。`,
      interrupted: '掃描已安全中斷。檢查已授權資料夾後請繼續。',
      stopped: '中繼資料掃描已安全停止。既有清單資料未被部分取代。',
      creating: '正在建立可持久化的本機掃描工作…',
      startDenied: '無法開始新的掃描。若此資料夾已有工作，請繼續該工作。',
      waiting: '正在等待目前中繼資料項目完成…',
      pauseDenied: '無法安全記錄暫停要求。',
      revalidating: '正在重新驗證已授權資料夾邊界…',
      resumeDenied: '繼續操作被拒絕，因為工作或已授權資料夾不再有效。',
    },
    status: {
      pausing: '正在安全暫停…',
      scanning: '正在掃描中繼資料…',
      paused: '已暫停',
      interrupted: '已安全中斷',
      completed: '已完成',
      stopped: '因錯誤停止',
    },
  },
  footer: {
    version: (version) => `DeskGraph ${version}`,
    description: '中繼資料 + 受限本機文字 · 不上傳 · 不進行檔案操作',
  },
} satisfies Catalog;

export const catalogs: Record<Locale, Catalog> = { en, 'zh-TW': zhTW };

export function isLocale(value: unknown): value is Locale {
  return typeof value === 'string' && (LOCALES as readonly string[]).includes(value);
}

export function resolveLocale(
  storedValue: unknown,
  navigatorLanguages: readonly string[] = [],
): Locale {
  if (isLocale(storedValue)) return storedValue;
  for (const language of navigatorLanguages) {
    const normalized = language.trim().toLowerCase();
    if (!normalized) continue;
    if (normalized === 'en' || normalized.startsWith('en-')) return 'en';
    if (
      normalized === 'zh' ||
      normalized === 'zh-tw' ||
      normalized === 'zh-hk' ||
      normalized === 'zh-mo' ||
      normalized === 'zh-hant' ||
      normalized.startsWith('zh-hant-') ||
      normalized.startsWith('zh-hk-') ||
      normalized.startsWith('zh-mo-')
    )
      return 'zh-TW';
    if (normalized.startsWith('zh-')) return 'en';
  }
  return 'en';
}

export function collectLanguagePreferences(
  languages: readonly string[] | undefined,
  language: string | undefined,
): readonly string[] {
  const preferences = Array.isArray(languages) ? [...languages] : [];
  if (language && !preferences.includes(language)) preferences.push(language);
  return preferences;
}

export function readStoredLocale(storage: Pick<LocaleStorage, 'getItem'>): Locale | null {
  try {
    const value = storage.getItem(LOCALE_STORAGE_KEY);
    return isLocale(value) ? value : null;
  } catch {
    return null;
  }
}

export function loadLocale(
  storage: Pick<LocaleStorage, 'getItem'>,
  navigatorLanguages: readonly string[] = [],
): Locale {
  return resolveLocale(readStoredLocale(storage), navigatorLanguages);
}

export function storeLocale(storage: Pick<LocaleStorage, 'setItem'>, locale: Locale): boolean {
  try {
    storage.setItem(LOCALE_STORAGE_KEY, locale);
    return true;
  } catch {
    return false;
  }
}

export function getCatalog(locale: Locale): Catalog {
  return catalogs[locale];
}

export function getLocalizedMetadata(locale: Locale): Catalog['metadata'] {
  return catalogs[locale].metadata;
}

export function formatInteger(value: number, locale: Locale): string {
  return new Intl.NumberFormat(locale).format(value);
}

export function formatUtcDate(value: Date | number | string, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, {
    timeZone: 'UTC',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(new Date(value));
}
