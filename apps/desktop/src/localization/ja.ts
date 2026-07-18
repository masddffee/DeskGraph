import { formatIntegerForLocale, type Catalog } from './catalog';

export const ja = {
  metadata: {
    title: 'DeskGraph — プレリリース',
    description: 'DeskGraph プレリリース版：ローカルファーストのコンピューターコンテキストグラフ',
  },
  language: { selectorLabel: '表示言語' },
  hero: {
    eyebrow: 'DeskGraph · M2 ローカルコンテキスト',
    heading: 'コンピューターをグラフ化。',
    description:
      '一度に許可するローカルフォルダーは 1 つだけです。メタデータのマニフェストを作成し、制限付きのテキスト抽出も、パスや内容をアップロードせず、このコンピューターに保持します。',
    release: 'プレリリース',
  },
  loading: {
    heading: 'ローカルマニフェストを開いています',
    description: '許可済みフォルダーが自動でスキャンされることはありません。',
  },
  backendError: {
    heading: 'ローカルマニフェストを利用できません',
    description:
      'バックエンドから検証済みの状態が返されませんでした。未加工のローカルエラーとパスは表示しません。',
    retry: '再試行',
  },
  runtime: {
    kicker: 'ローカルランタイム',
    heading: 'マニフェストの準備ができました',
    localOnly: 'ローカルのみ',
    platform: 'プラットフォーム',
    sqliteManifest: 'SQLite マニフェスト',
    optionalLocalLlm: '任意のローカル LLM',
    networkRequired: 'ネットワークが必要',
    ready: '準備完了',
    no: 'いいえ',
    lifecycle: { notInitialized: '未初期化', disabled: '無効' },
  },
  manifest: {
    kicker: '現在のグラフ',
    emptyHeading: 'まだ何もインデックスされていません',
    readyHeading: 'メタデータをインデックスしました',
    files: 'ファイル',
    folders: 'フォルダー',
    locations: '場所',
    scanIssues: 'スキャンの問題',
  },
  search: {
    kicker: '決定的なローカル検索',
    heading: 'ファイル名と抽出済みテキストを検索',
    description:
      '画面は日本語表示ですが、テキスト抽出と検索で現在検証済みの言語は繁体字中国語と英語です。すべてのクエリは SQLite 内にとどまり、各結果には一致したローカルフィールドが示されます。',
    mode: '字句検索 · オフライン',
    queryLabel: 'ローカルコンテキストを検索',
    queryPlaceholder: 'プロジェクトの文脈 または project context',
    scopeAria: '検索するフォルダー範囲',
    allFolders: 'すべての許可済みフォルダー',
    authorizedScope: (scopeId) => `許可済み範囲 ${scopeId}`,
    search: '検索',
    searching: '検索中…',
    filtersAria: '制限付きローカル検索フィルター',
    sourceLabel: '一致元',
    sources: {
      all: 'パス + 抽出済みテキスト',
      paths: 'ファイル名とパスのみ',
      extractedText: '抽出済みテキストのみ',
    },
    fileType: 'ファイル形式',
    fileTypePlaceholder: 'md または pdf',
    modifiedSince: '更新日時の開始（UTC）',
    modifiedBefore: '更新日時の終了（UTC、含まない）',
    validation: {
      query: 'ローカル検索を制限内に保つため、3 文字以上入力してください。',
      extension: 'ファイル形式は md、pdf、docx のような 1〜16 文字の拡張子にしてください。',
      dateRange:
        '有効な UTC の日付範囲を選択してください。「更新日時の開始」は「更新日時の終了」より前である必要があります。',
      request:
        '検索は安全に停止しました。より短いクエリを試すか、ローカルマニフェストを更新してください。',
    },
    empty: (query) =>
      `現在のパスまたは有効な抽出済みテキストに「${query}」と一致するものはありません。`,
    summary: (count, elapsedMs) =>
      `${formatIntegerForLocale(count, 'ja')} 件の結果 · ${formatIntegerForLocale(elapsedMs, 'ja')} ms`,
    filters: {
      scope: (scopeId) => `範囲 ${scopeId}`,
      pathsOnly: 'パスのみ',
      textOnly: '抽出済みテキストのみ',
      since: (date) => `${date} UTC 以降`,
      before: (date) => `${date} UTC より前`,
      allSources: 'すべての許可済みローカルソース',
    },
    explanation: {
      filenameAndText: '完全一致のファイル名 + 抽出済みテキスト',
      filename: '完全一致のファイル名',
      pathAndText: 'パス + 抽出済みテキスト',
      path: 'ファイル名またはパス',
      text: '抽出済みテキスト',
    },
    ocr: {
      controlsAria: 'ローカルスクリーンショット OCR コントロール',
      notRead: 'スクリーンショットのテキストはまだ読み取られていません',
      description:
        'すでにスキャン済みのこのスクリーンショットだけを再検証し、このコンピューター上で読み取ります。現在検証済みの認識言語は繁体字中国語と英語です。',
      cancel: 'OCR をキャンセル',
      stopping: '安全に停止しています…',
      retryQueued: '待機中の OCR を再試行',
      resume: 'スクリーンショット OCR を再開',
      readAgain: 'スクリーンショットを再度読み取る',
      read: 'スクリーンショットのテキストをローカルで読み取る',
      untrustedText: '信頼されていないローカルテキスト',
      queued: '開始待ち',
      running: '制限付きテキストを抽出中…',
      reading: 'スクリーンショットのテキストをローカルで読み取り中…',
      completed: 'スクリーンショットのテキストをローカルでインデックスしました',
      cancelled: '安全にキャンセルしました',
      interrupted: '安全に中断しました',
      unavailable: 'スクリーンショット OCR は利用できないか、安全にスキップされました',
      skipped: 'ファイルを安全にスキップしました',
      capacity:
        '別のローカル OCR がまだ完了処理中です。このジョブは待機したままです。安全に再試行してください。',
      providerUnavailable:
        'スクリーンショット OCR は安全に停止しました。このコンピューターではローカルプロバイダーを利用できない可能性があります。',
      indexed:
        'スクリーンショットのテキストをローカルでインデックスしました。内容を見つけるには再度検索してください。',
      cancelledFeedback:
        'スクリーンショット OCR は安全にキャンセルされました。部分的なテキストは公開されていません。',
      interruptedFeedback:
        'スクリーンショット OCR は安全に中断されました。ローカルで完了するには再開してください。',
      failedFeedback:
        'スクリーンショット OCR は安全に停止しました。以前の完全なローカルインデックスは保持されています。',
      denied:
        'OCR は安全に拒否されました。ファイルが変更された場合は再スキャンし、対応するスクリーンショットであることを確認してください。',
      resumeDenied:
        '再開は安全に拒否されました。もう一度試す前にローカルマニフェストを更新してください。',
      cancelDenied: 'キャンセル要求を安全に記録できませんでした。',
    },
  },
  actions: {
    kicker: '安全な整理プレビュー',
    heading: 'ファイルを変更せずに名前変更を確認',
    description:
      'DeskGraph はプレビューを記録する前に、選択した範囲、現在のマニフェストスナップショット、ファイル ID、読み取り専用のオープンハンドル、提案名、保存先を再検証します。',
    previewOnly: 'プレビューのみ · 実行しない',
    folderLabel: '許可済みフォルダー',
    chooseFolder: 'スキャン済みフォルダーを選択',
    scopeOption: (scopeId, path) => `範囲 ${scopeId} · ${path}`,
    sourceLabel: '現在の絶対ファイルパス',
    sourcePlaceholder: '/authorized/folder/draft.md',
    newNameLabel: '提案するファイル名のみ',
    newNamePlaceholder: 'final.md',
    validating: '安全に検証中…',
    createPreview: '永続プレビューを作成',
    validation: {
      chooseFolder: 'まず許可済みフォルダーを選択してください。',
      required: '現在の絶対ファイルパスと、提案するファイル名を 1 つ入力してください。',
      denied:
        'プレビューは安全に拒否されました。変更されたファイルを再スキャンし、許可済みフォルダーを確認して、未使用で移植可能なファイル名を選んでください。',
    },
    previewAria: '検証済みの名前変更プレビュー',
    plan: (planId) => `計画 ${planId} · 検証済みプレビュー`,
    caseOnly: '大文字小文字だけの名前変更には、将来実装する段階的な実行機能が必要です。',
    direct: '将来の実行機能用に直接名前変更戦略を記録しました。',
    unchanged: 'ファイルは変更されていません',
    before: '変更前',
    after: '変更後',
    policyAria: '通過したポリシーチェック',
    policy: {
      authorizedScope: '選択した許可済みフォルダー内にあります',
      manifestFile: '現在スキャン済みのファイルです',
      canonicalSource: '正規化されたソースは範囲内にあります',
      sourceIdentity: 'プラットフォーム ID はマニフェストと一致します',
      readOnlyHandle: '読み取り専用のオープンハンドルが一致します',
      portableName: '移植可能な単一要素のファイル名です',
      sameParent: '同じ正規化済み親フォルダーです',
      destinationScope: '保存先は範囲内にあります',
      destinationAvailable: '保存先を利用できます',
    },
    noExecute:
      'この計画は記録されていますが実行できません。復旧と Undo はまだ存在しないため、DeskGraph は操作ボタンを表示しません。',
    historyHeading: '最近のパスを含まないプレビュー履歴',
    plans: (count) => `${formatIntegerForLocale(count, 'ja')} 件の計画`,
    historyEmpty:
      '整理プレビューはまだ記録されていません。このアプリからファイルを変更することはできません。',
    historyPlan: (planId) => `名前変更プレビュー ${planId}`,
    historyScopeNode: (scopeId, nodeId) => `範囲 ${scopeId} · ノード ${nodeId}`,
    caseOnlyStaged: '大文字小文字のみ · 段階的',
    directPreviewed: '直接 · プレビュー済み',
  },
  watch: {
    kicker: 'ネイティブイベントのヒントと安全な照合',
    heading: 'ネイティブイベントのヒント + 5 分ごとの安全な照合',
    description:
      'ネイティブファイルシステムイベントはヒントにすぎません。5 分ごとの定期的な安全照合でも、条件を満たし、以前にスキャンされたフォルダーを制限付きバッチで確認します。増分コンテンツ更新や完了期限を保証するものではありません。',
    adapterActive: 'ネイティブヒント + 5 分ごとの安全な照合 · 実行中',
    adapterStarting: 'ネイティブヒント + 5 分ごとの安全な照合 · 起動中',
    adapterDegraded: 'ネイティブヒントまたは定期のみのフォールバック · 要確認',
    adapterStopped: 'ネイティブヒント + 5 分ごとの安全な照合 · 停止',
    metrics: {
      recent: '最近のイベント',
      observed: '観測されたヒント',
      reconciled: '照合済み',
      deferred: '延期されたフォルダー',
      attention: '要確認',
    },
    empty:
      'まだ変更は観測されていません。期限を迎え、以前にスキャンされたフォルダーはローカルで制限付きバッチにより予定されます。新たに許可したフォルダーには、引き続き明示的な初回スキャンが必要です。',
    status: {
      stabilizing: '安定したスナップショットを待機中',
      reconciling: '原子的なマニフェスト照合',
      completed: '照合済み',
      noFailure: '失敗なし',
    },
    reason: {
      temporary: '一時ダウンロードを無視しました',
      hidden: '非表示エントリを無視しました',
      unsupported: '未対応のエントリを無視しました',
      unavailable: 'ソースを利用できません',
      failed: '照合は安全に失敗しました',
    },
    event: (eventId, scopeId, hints) =>
      `イベント ${eventId} · 範囲 ${scopeId} · ${formatIntegerForLocale(hints, 'ja')} 件の統合ヒント`,
    scan: (jobId) => `スキャン ${jobId}`,
    noScan: 'まだスキャンしていません',
  },
  extraction: {
    kicker: '制限付きローカルコンテンツ',
    emptyHeading: 'まだファイル内容を抽出していません',
    readyHeading: 'ローカルテキストの準備ができました',
    description:
      '対象となるのは、すでにスキャン済みの対応ドキュメントと明示的に選択したスクリーンショットだけです。各ソースを再検証し、出力サイズを制限します。失敗したジョブが最後の完全なテキストを置き換えることはありません。',
    neverUploaded: 'アップロードしません',
    metrics: {
      files: 'テキストを含むファイル',
      chunks: '有効なチャンク',
      completed: '完了したジョブ',
      skipped: 'スキップまたはキャンセル',
    },
    latest: (operation, jobId) => `最新の ${operation} ジョブ ${jobId}`,
    operation: { screenshotOcr: 'スクリーンショット OCR', content: 'コンテンツ' },
    progress: (chunks, bytes) =>
      `${formatIntegerForLocale(chunks, 'ja')} チャンク · ${formatIntegerForLocale(bytes, 'ja')} バイト`,
    optInEmpty:
      'コンテンツ抽出はオプトインです。フォルダーを許可またはスキャンしても、ファイル内容が読み取られることはありません。',
  },
  scope: {
    kicker: '明示的な許可',
    heading: 'DeskGraph が検査できるフォルダー',
    description:
      '存在するフォルダーパスを入力してください。許可とスキャンは別の操作です。シンボリックリンクと非表示エントリは追跡しません。',
    count: (count) => `${formatIntegerForLocale(count, 'ja')} 件を許可済み`,
    inputLabel: 'フォルダーパス',
    placeholder: '/Users/you/Documents or C:\\Users\\you\\Documents',
    authorize: 'フォルダーを許可',
    emptyHeading: 'フォルダーへのアクセス権がありません',
    emptyDescription:
      'ここに追加するまで、DeskGraph はデスクトップ、ダウンロード、またはドキュメントを検査できません。',
    label: (scopeId) => `許可済み範囲 ${scopeId}`,
    progress: (processed, queued, issues) =>
      `${formatIntegerForLocale(processed, 'ja')} / ${formatIntegerForLocale(queued, 'ja')} 件の項目 · ${formatIntegerForLocale(issues, 'ja')} 件の問題`,
    pause: 'スキャンを一時停止',
    pausing: '一時停止中…',
    resume: 'スキャンを再開',
    scan: 'メタデータをスキャン',
    validation: {
      required: 'まず存在するフォルダーパスを入力してください。',
      validating: 'フォルダー境界を検証中…',
      authorized:
        'フォルダーを許可しました。「メタデータをスキャン」を選ぶまで、何もスキャンされません。',
      denied: 'フォルダーを許可できませんでした。存在し、保護されていないことを確認してください。',
      reading: '許可済みフォルダー内のメタデータを読み取り中…',
      complete: (files, folders) =>
        `スキャン完了：${formatIntegerForLocale(files, 'ja')} 個のファイルと ${formatIntegerForLocale(folders, 'ja')} 個のフォルダー。`,
      paused: (processed, queued) =>
        `スキャンは、検出した ${formatIntegerForLocale(queued, 'ja')} 件の項目中 ${formatIntegerForLocale(processed, 'ja')} 件を処理した後に一時停止しました。`,
      interrupted:
        'スキャンは安全に中断されました。許可済みフォルダーを確認してから再開してください。',
      stopped:
        'メタデータスキャンは安全に停止しました。既存のマニフェストデータは部分的に置き換えられていません。',
      creating: '永続的なローカルスキャンジョブを作成中…',
      startDenied:
        '新しいスキャンを開始できませんでした。このフォルダーに既存のジョブがある場合は再開してください。',
      waiting: '現在のメタデータ項目が完了するのを待っています…',
      pauseDenied: '一時停止要求を安全に記録できませんでした。',
      revalidating: '許可済みフォルダー境界を再検証中…',
      resumeDenied: 'ジョブまたは許可済みフォルダーが有効でなくなったため、再開は拒否されました。',
    },
    status: {
      pausing: '安全に一時停止中…',
      scanning: 'メタデータをスキャン中…',
      paused: '一時停止',
      interrupted: '安全に中断',
      completed: '完了',
      stopped: 'エラーにより停止',
    },
  },
  footer: {
    version: (version) => `DeskGraph ${version}`,
    description: 'メタデータ + 制限付きローカルテキスト · アップロードなし · ファイル操作なし',
  },
} satisfies Catalog;
