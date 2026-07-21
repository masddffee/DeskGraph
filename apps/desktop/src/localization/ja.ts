import { formatIntegerForLocale, type Catalog } from './catalog';

export const ja = {
  metadata: {
    title: 'DeskGraph — プレリリース',
    description: 'DeskGraph プレリリース版：ローカルファーストのコンピューターコンテキストグラフ',
  },
  language: { selectorLabel: '表示言語' },
  navigation: {
    ariaLabel: 'メインナビゲーション',
    skipToContent: 'メインコンテンツへ移動',
    brandDescription: 'ローカルファーストのコンピューターコンテキスト',
    localOnly: 'ローカルのみ',
    noNetwork: 'ネットワーク不要',
    views: {
      home: {
        label: 'ホーム',
        title: 'ローカルコンテキストをひと目で確認',
        description:
          'このコンピューター上の許可、メタデータスキャン、ローカルマニフェストの状態を確認します。',
      },
      search: {
        label: '検索',
        title: 'ローカルコンテキストを検索',
        description:
          '現在のファイル名と明示的に抽出したテキストを検索し、クエリを外部へ送信しません。',
      },
      projects: {
        label: 'プロジェクト',
        title: '許可済みフォルダーとメタデータスキャン',
        description:
          'フォルダーを許可してメタデータをスキャンします。この画面では完全なプロジェクト検出はまだ利用できません。',
      },
      inbox: {
        label: '受信箱',
        title: 'ローカル候補を確認',
        description:
          '現在のローカルクリーンアップ候補を確認します。この受信箱からファイルの変更、ゴミ箱への移動、削除、Undo はできません。',
      },
      history: {
        label: '履歴',
        title: 'プレビュー履歴',
        description:
          'パスを含まない名前変更プレビューを確認します。この画面ではファイル操作を実行またはUndoできません。',
      },
      settings: {
        label: '設定',
        title: 'ローカルランタイムとプライバシー',
        description: '表示言語を選び、ローカルのみのランタイムとプライバシー境界を確認します。',
      },
    },
  },
  hero: {
    eyebrow: 'DeskGraph · M2 ローカルコンテキスト',
    heading: 'コンピューターをグラフ化。',
    description:
      '1 つ以上のローカルフォルダーを一度に明示的に確認し、メタデータのマニフェストを作成します。制限付きテキスト抽出も、パスや内容をアップロードせず、このコンピューターに保持します。',
    release: 'プレリリース',
  },
  journey: {
    kicker: '安全な最初の操作',
    heading: '3 つの明示的な手順で、有用なローカルコンテキストを確認します。',
    description:
      '対象範囲を選び、メタデータをスキャンしてから、要求したローカル候補だけを確認します。DeskGraph が最初からコンピューター全体を読むことはありません。',
    privacy: 'アップロードなし · 自動スキャンなし · ファイル変更なし',
    scope: {
      title: '1 · 対象範囲を選ぶ',
      description:
        'ネイティブピッカーで 1 つ以上のフォルダーを選択します。許可しただけでは内容の読み取りやスキャンは行いません。',
      action: 'ローカルフォルダーを選ぶ',
    },
    search: {
      title: '2 · スキャンしてから検索',
      description:
        '選んだフォルダーの初回メタデータスキャンを実行してから、SQLite 内でファイル名と明示的に抽出したテキストを検索します。',
      action: '検索を開く',
      scanAction: 'スキャン操作を開く',
    },
    review: {
      title: '3 · 根拠のある候補を確認',
      description:
        'ローカルのプロジェクト候補とクリーンアッププレビューを確認します。どちらの画面でもファイルの変更、ゴミ箱への移動、削除、Undo はできません。',
      projectsAction: 'プロジェクトを確認',
      cleanupAction: 'クリーンアップ候補を確認',
    },
    mcp: {
      title: '読み取り専用 MCP は別途起動',
      description:
        'スキャン完了後、独立して起動する macOS/Linux のローカル MCP スライスは、起動時に許可された範囲だけを検索できます。この Desktop 画面での設定や書き込みツールの提供はありません。',
    },
    status: {
      noScope: '開始するには対象範囲を選択してください',
      scopesReady: (count) => `${count} 件のフォルダーを許可済み · スキャンは明示的に開始します`,
      scanNeeded:
        '検索、プロジェクト、クリーンアップでこのフォルダーを評価する前に初回スキャンを完了してください',
      scanReady: '完了したローカルスキャンは検索と確認に利用できます',
    },
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
    content: {
      controlsAria: '制限付きローカルテキスト抽出コントロール',
      notRead: 'ファイル内容はまだ読み取られていません',
      description:
        '明示的に選択したスキャン済みのテキスト、PDF、Office ファイルだけを再検証し、ローカルで読み取ります。',
      read: 'テキストをローカルで抽出',
      readAgain: '再度抽出',
      runQueued: '待機中の抽出を実行',
      reading: '安全に抽出中…',
      completed: '制限付きテキストをローカルでインデックスしました。',
      searchExtracted: '抽出済みテキストを検索',
      indexed: 'テキストをローカルでインデックスしました。文書内の語句を入力して検索してください。',
      failed:
        '抽出は安全に停止しました。部分的なテキストが以前の完全なインデックスを置き換えることはありません。',
      denied:
        '抽出は安全に拒否されました。変更されたファイルを再スキャンするか、対応ファイルを選択してください。',
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
    direct: '直接名前変更戦略をこの永続プレビューに記録しました。',
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
      'この Desktop 画面ではプレビューの作成と表示のみ行います。実行または Undo のコントロールは表示しません。',
    historyHeading: '最近のパスを含まない整理履歴',
    plans: (count) => `${formatIntegerForLocale(count, 'ja')} 件の計画`,
    historyEmpty:
      '整理計画はまだ記録されていません。この Desktop 画面では計画を実行または Undo できません。',
    historyPlan: (planId) => `名前変更計画 ${planId}`,
    historyScopeNode: (scopeId, nodeId) => `範囲 ${scopeId} · ノード ${nodeId}`,
    caseOnlyStaged: '大文字小文字のみ · 段階的',
    directStrategy: '直接',
    historyState: {
      previewed: 'プレビュー済み',
      pending: '保留中',
      executed: '実行済み',
      undone: 'Undo 済み',
      needsAttention: '要確認',
    },
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
  cleanup: {
    kicker: 'スマートクリーンアップ受信箱',
    heading: 'ローカルのクリーンアップ候補を確認',
    description:
      '許可済みフォルダーを明示的に選ぶと、重複、旧バージョン、スクリーンショットグループの現在の証拠を更新します。読み取るのはローカル証拠のみです。',
    suggestionOnly: '候補のみ · ファイルは変更しません',
    scopeLabel: '許可済みフォルダー',
    chooseScope: '許可済みフォルダーを選択',
    refresh: '候補を更新',
    refreshing: 'ローカル証拠を更新中…',
    controlsAria: 'スマートクリーンアップ受信箱のコントロール',
    authorizationRequired:
      'ローカルのクリーンアップ候補を要求する前に、フォルダーを許可してください。',
    empty: 'この許可済みフォルダーには、現在確認できるクリーンアップ候補はありません。',
    partial: (notCurrent) =>
      notCurrent === 0
        ? '今回の制限付き更新は未完了です。一部のソースを安全に評価できませんでした。'
        : `今回の制限付き更新は未完了です。${formatIntegerForLocale(notCurrent, 'ja')} 件の古い候補は、証拠が現在のものではないため除外されました。`,
    error: '候補を安全に更新できませんでした。ファイルは変更されていません。',
    exactDuplicate: '完全一致の重複',
    version: '旧バージョン候補',
    screenshotReviewGroup: 'スクリーンショット確認グループ',
    exactDuplicateExplanation:
      'このペアは制限付きの全バイト比較で一致しました。保持するファイルの選択やクリーンアップの許可ではありません。',
    versionExplanation:
      '明示的な数値 vN ファイル名規則が小さい番号から大きい番号への方向だけを示します。古いファイルを安全に破棄できる証明ではありません。',
    screenshotReviewGroupExplanation:
      '同じ寸法、10 分の時間枠、現在の OCR 来歴は確認用のグループ化にのみ使われます。スクリーンショット由来、内容の類似、重複、破棄可能性を証明しません。',
    itemMeta: (members, evidenceScoreBasisPoints, observedAt) =>
      `${formatIntegerForLocale(members, 'ja')} 件 · 証拠スコア ${formatIntegerForLocale(evidenceScoreBasisPoints, 'ja')} / 10,000 · 観測 ${observedAt}`,
    verification:
      '確認画面を開くと現在の証拠を再検証し、その後に永続プレビューを作成できます。この受信箱から確認、移動、ゴミ箱への移動、削除、復元、元に戻す操作はできません。',
    review: {
      open: 'ファイルを確認してプレビューを作成',
      loading: 'この候補をローカルで再検証しています…',
      close: 'ファイル確認を閉じる',
      transientNotice:
        'パスは、この明示的なローカル確認中だけ表示します。クリーンアップ計画、履歴、ログ、言語設定には保存しません。',
      selectionLegend: 'クリーンアップのプレビュー対象を 1 件選択',
      selectTarget: 'プレビュー対象として選択',
      keeper: '比較用ファイルとして保持',
      keeperSwitch: 'このファイルを保持 · 選択すると対象を切り替えます',
      noKeeper:
        'スクリーンショット確認では保持ファイルを指定しません。各プレビューは対象 1 件だけを固定します。',
      roles: {
        duplicateCandidate: '完全一致の重複候補',
        olderVersion: '旧バージョン · 対象に選択可能',
        newerVersion: '新バージョン · 保持が必須',
        screenshotCandidate: 'スクリーンショットグループの項目',
      },
      memberSize: (bytes) => `${formatIntegerForLocale(bytes, 'ja')} バイト`,
      createPreview: '永続プレビューを作成',
      creatingPreview: '現在のファイル証拠を固定しています…',
      selectionRequired: '続行する前に、プレビューするファイルを 1 件選択してください。',
      detailError:
        'この候補は安全に確認できなくなりました。受信箱を更新してから再試行してください。',
      previewError:
        'プレビューを固定する前にファイルまたは証拠が変わりました。操作は許可されていません。',
      previewReady: (planId) =>
        `プレビュー ${formatIntegerForLocale(planId, 'ja')} をローカルで固定しました`,
      expectedBytesLabel: '固定したファイルサイズ',
      expectedBytes: (bytes) =>
        `${formatIntegerForLocale(bytes, 'ja')} バイトをこのプレビューに固定`,
      journalLabel: 'ジャーナル',
      journalSequence: (sequence) =>
        `変更不可のジャーナル連番 ${formatIntegerForLocale(sequence, 'ja')}`,
      checksLabel: 'ポリシー検証',
      checksPassed: (count) => `${formatIntegerForLocale(count, 'ja')} 件のポリシーチェックを記録`,
      noExecution:
        'プレビューのみです。ファイルの確認、移動、ゴミ箱への移動、削除、復元、元に戻す操作は行いません。',
    },
  },
  projects: {
    kicker: 'プロジェクト検出',
    heading: 'ローカルのプロジェクト候補を確認',
    description:
      'スキャン済みの許可フォルダーを 1 つ明示的に選び、決定的なローカルフォルダーマーカーで評価します。',
    scopeLabel: '許可済みフォルダー',
    chooseScope: '許可済みフォルダーを選択',
    discover: 'プロジェクトを検出',
    discovering: 'ローカルフォルダーの証拠を評価中…',
    checkingReadiness: '永続化されたスキャン状態を確認しています…',
    readinessError: 'このフォルダーのスキャン状態を安全に確認できませんでした。',
    controlsAria: 'プロジェクト検出コントロール',
    authorizationRequired:
      'ローカルのプロジェクト候補を要求する前に、フォルダーを許可してください。',
    scanRequired:
      'プロジェクトを検出する前に、このフォルダーのメタデータスキャンを完了してください。',
    empty: '現在のフォルダーマーカーに一致するプロジェクト候補はありません。',
    partial: 'この制限付き評価は部分的です。一部のルートを安全に評価できませんでした。',
    error: 'プロジェクト候補を安全に評価できませんでした。',
    candidateMeta: (confidence, observedAt) =>
      `証拠スコア ${formatIntegerForLocale(confidence, 'ja')} / 10,000 · 観測 ${observedAt}`,
    viewEvidence: '証拠を確認',
    suggestionOnly: '候補のみ',
    noAutomaticMembership: 'ルートを承認してもファイルの所属は作成されません。',
    noFileActions: 'ファイルを移動、名前変更、削除、ゴミ箱への移動、復元しません。',
    state: {
      suggested: '候補のプロジェクトルート',
      accepted: '承認済みプロジェクトルート',
      rejected: '拒否済みプロジェクトルート',
    },
    detail: {
      loading: 'このプロジェクト候補をローカルで再検証中…',
      transientNotice:
        'この明示的なローカル確認でのみ現在のパスを表示します。通常の一覧やログには含まれません。',
      rootLabel: '現在のプロジェクトルート',
      signalsLabel: '検出されたフォルダーマーカー',
      close: 'プロジェクト確認を閉じる',
      accept: 'プロジェクトルートを承認',
      reject: 'プロジェクトルートを拒否',
      saving: '追記専用の判断を保存中…',
      decisionError: '判断を安全に保存できませんでした。',
      detailError: 'このプロジェクト候補は現在の状態ではありません。更新して再試行してください。',
    },
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
      '同じシステム選択画面で 1 つ以上のフォルダーを選びます。DeskGraph がアクセスするのは確認した範囲だけです。許可とスキャンは別の操作で、選択だけではスキャンを開始せず、シンボリックリンクや非表示エントリも追跡しません。',
    count: (count) => `${formatIntegerForLocale(count, 'ja')} 件を許可済み`,
    pickerAriaLabel: 'システム選択画面で 1 つ以上のフォルダーを選ぶ',
    authorize: '許可するフォルダーを選ぶ',
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
      cancelled: 'フォルダー選択をキャンセルしました。許可の変更は確定されていません。',
      validating: 'フォルダーの選択を待機してから、すべての境界を検証しています…',
      authorized: (count) =>
        `${formatIntegerForLocale(count, 'ja')} 件のフォルダーをまとめて許可しました。まだスキャンしていません。準備ができた範囲ごとに「メタデータをスキャン」を選んでください。`,
      refreshFailed: (count) =>
        `${formatIntegerForLocale(count, 'ja')} 件のフォルダーを許可し、まだスキャンしていませんが、最新のローカル状態を更新できませんでした。画面を再読み込みして同期してください。`,
      denied:
        '選択したフォルダーを 1 つのセットとして許可できず、変更は確定されませんでした。利用不可、保護対象、重複、または重なっているフォルダーを外して再試行してください。',
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
  hardExclusion: {
    kicker: '対象範囲のプライバシー',
    heading: 'ハード除外',
    description:
      'ハード除外はローカルのアクセスと索引を拒否します。検索結果を隠すだけではありません。',
    scopeLabel: '許可済みルート',
    chooseScope: '許可済みルートを選択',
    loading: '現在のポリシーを読み込み中…',
    policyRevision: (revision) => `ポリシー改訂 ${revision}`,
    addFolders: 'フォルダーを追加…',
    addFiles: 'ファイルを追加…',
    noExclusions: 'このルートにハード除外はありません。',
    currentExclusions: '現在のハード除外',
    removalUnavailable: 'このリリーススライスでは、ハード除外の解除は利用できません。',
    previewHeading: 'ハード除外を確認',
    previewNotice:
      'DeskGraph は影響するローカル索引データを消去します。元ファイルを移動または削除しません。',
    previewEmpty: 'ネイティブ選択をキャンセルしました。ポリシーは変更されていません。',
    notConfirmable:
      '現在のファイル操作または復元可能なジャーナル作業が完了していないため、この除外は確定できません。元ファイルは変更されていません。',
    item: {
      willAdd: '除外予定',
      alreadyExcluded: 'すでに除外済み',
      coveredByParent: '選択したフォルダーに含まれます',
      file: 'ファイル',
      folder: 'フォルダー',
    },
    impact: (locations, content, graph, candidates, jobs, actions) =>
      `ローカルで ${locations} 件の場所、${content} 件の内容／OCR チャンク、${graph} 件のグラフ事実、${candidates} 件の派生候補、${jobs} 件の保留ジョブ、この変更を妨げる ${actions} 件の操作安全記録に影響します。`,
    confirm: '除外を確定してローカル索引を消去',
    confirming: 'プライバシー消去を確定中…',
    cancel: 'プレビューをキャンセル',
    cancelled: 'プレビューをキャンセルしました。ポリシーは変更されていません。',
    committed: (count) =>
      `${count} 件のハード除外を保存し、影響するローカル索引データを消去しました。`,
    refreshFailed: (count) =>
      `${count} 件のハード除外を保存してローカルデータを消去しましたが、再読み込みに失敗しました。`,
    error: 'ハード除外は確定されませんでした。ローカルのプライバシー消去完了とは表示しません。',
    sourceSafe: '元ファイルを移動、変更、削除しません。',
  },
  rootRevocation: {
    kicker: '対象範囲のアクセス',
    heading: '許可済みルートを取り消す',
    description:
      '取り消すと、このルートに由来する DeskGraph のローカル索引データを恒久的に消去し、このランタイムのアクセス権も破棄します。元ファイルの移動、ゴミ箱への移動、削除は行いません。',
    empty: '取り消せる有効な許可済みルートはありません。',
    revoke: 'ルートを取り消す…',
    previewHeading: 'ルート取り消しを確認',
    exclusionCount: (count) => `${count} 件のハード除外をこのルートとともに削除します。`,
    previewNotice:
      '影響するローカル派生索引データを恒久的に消去し、このルートを DeskGraph から削除します。この画面から元に戻すことはできません。',
    sourceSafe: '元ファイルの移動、変更、ゴミ箱への移動、削除は行いません。',
    noAutomaticRead:
      'この取り消されたルートに対して、新しいスキャン、抽出、OCR、embedding、その他のファイルシステム読み取りは開始されません。ほかの許可済みルートで進行中の処理は継続できます。',
    impact: (
      locations,
      content,
      graph,
      candidates,
      actionPlans,
      cleanupActionPlans,
      jobs,
      actions,
    ) =>
      `確認後、ローカルで ${locations} 件の場所、${content} 件の内容／OCR チャンク、${graph} 件のグラフ事実、${candidates} 件の派生候補、${actionPlans} 件の名前変更／移動プレビュー、${cleanupActionPlans} 件の安全クリーンアッププレビュー、${jobs} 件の保留ジョブを消去します。${actions === 0 ? 'この取り消しを妨げる操作安全記録はありません。' : `${actions} 件の操作安全記録が取り消しを妨げており、それらは消去されません。`}`,
    confirm: '取り消しを確定してローカル索引を消去',
    loading: 'ローカル限定の取り消し確認を準備しています…',
    confirming: 'ローカル対象範囲を安全に取り消しています…',
    cancel: 'プレビューをキャンセル',
    cancelled: '取り消しプレビューをキャンセルしました。このルートは許可されたままです。',
    notConfirmable:
      '操作ジャーナルの安全記録がこの取り消しを妨げており、消去されません。別途審査された復旧フローでの解決が必要で、待機や再試行だけでは解除できません。元ファイルは変更されていません。',
    committed: (exclusions) =>
      `ルートのアクセスを取り消してローカル派生データを消去しました。${exclusions} 件のハード除外もルートとともに削除しました。`,
    refreshFailed: (exclusions) =>
      `ルートのアクセスを取り消し、${exclusions} 件のハード除外を削除しましたが、ローカルダッシュボードの再読み込みに失敗しました。`,
    watchSyncPending: (callbackRetired, runtimeStopped) =>
      runtimeStopped
        ? 'アクセスは取り消されました。ネイティブ callback を退役させ、キュー済みヒントを消去し、自動監視を完全に停止しました。ルートを再認証する前に DeskGraph を再起動してください。'
        : callbackRetired
          ? 'アクセスは取り消されました。ネイティブ callback は退役し、キュー済みヒントも消去しましたが、coordinator の停止は確認できていません。DeskGraph は OS の監視登録が閉じたとは報告しません。再認証前に再起動してください。'
          : 'アクセスは取り消されました。新しい callback の受付を閉じ、キュー済みヒントを消去しましたが、実行中 callback と coordinator の停止は確認できていません。DeskGraph は OS の監視登録が閉じたとは報告しません。再認証前に再起動してください。',
    stale:
      'プレビューを開いている間にこのルートが変わりました。取り消しは適用されていません。現在のローカル対象範囲をもう一度確認してください。',
    error:
      '取り消しの最終状態を確認できませんでした。再試行する前にローカル対象範囲を再読み込みしてください。この操作で元ファイルは変更されていません。',
  },
  footer: {
    version: (version) => `DeskGraph ${version}`,
    description: 'メタデータ + 制限付きローカルテキスト · アップロードなし · ファイル操作なし',
  },
} satisfies Catalog;
