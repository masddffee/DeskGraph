import { describe, expect, it } from 'vitest';

import appSource from './App.tsx?raw';
import {
  LOCALES,
  LOCALE_STORAGE_KEY,
  catalogs,
  collectLanguagePreferences,
  englishCount,
  formatInteger,
  formatUtcDate,
  getLocalizedMetadata,
  isLocale,
  loadLocale,
  localeOptions,
  localeRegistry,
  readStoredLocale,
  resolveLocale,
  storeLocale,
  type Locale,
} from './i18n';

function shape(value: unknown): unknown {
  if (typeof value === 'function') return `function:${value.length}`;
  if (Array.isArray(value)) return value.map(shape);
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, shape(child)]));
  }
  return typeof value;
}

function staticStrings(value: unknown): string[] {
  if (typeof value === 'string') return [value];
  if (Array.isArray(value)) return value.flatMap(staticStrings);
  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(staticStrings);
  }
  return [];
}

function occurrences(value: string, needle: string): number {
  return value.split(needle).length - 1;
}

describe('i18n catalog and registry contract', () => {
  it('keeps every registered catalog nested key and function shape identical', () => {
    expect(LOCALES).toEqual(['en', 'zh-TW', 'zh-CN', 'ja']);
    expect(Object.keys(catalogs)).toEqual(LOCALES);
    for (const locale of LOCALES) {
      expect(shape(catalogs[locale])).toEqual(shape(catalogs.en));
    }
  });

  it('defines each locale once with a unique autonym and data-driven option', () => {
    expect(localeOptions).toEqual([
      { value: 'en', label: 'English' },
      { value: 'zh-TW', label: '繁體中文' },
      { value: 'zh-CN', label: '简体中文' },
      { value: 'ja', label: '日本語' },
    ]);
    expect(new Set(localeOptions.map(({ label }) => label)).size).toBe(LOCALES.length);
    for (const locale of LOCALES) {
      expect(isLocale(locale)).toBe(true);
      expect(localeRegistry[locale]).toMatchObject({ htmlLang: locale, dir: 'ltr' });
      expect(localeRegistry[locale].catalog).toBe(catalogs[locale]);
    }
  });

  it('contains translated critical loading, safe-error, OCR, scan, and aria messages', () => {
    expect(catalogs.en.loading.heading).toBe('Opening the local manifest');
    expect(catalogs['zh-TW'].backendError.retry).toBe('重試');
    expect(catalogs['zh-CN'].search.ocr.controlsAria).toBe('本机截图 OCR 控件');
    expect(catalogs.ja.scope.status.scanning).toBe('メタデータをスキャン中…');
    for (const locale of LOCALES) {
      expect(catalogs[locale].actions.policyAria.length).toBeGreaterThan(0);
      expect(catalogs[locale].backendError.description.length).toBeGreaterThan(0);
      expect(catalogs[locale].scope.validation.denied.length).toBeGreaterThan(0);
    }
  });

  it('keeps preview, upload, extraction, and watch limitations explicit in every locale', () => {
    const safetyPhrases: Record<
      Locale,
      { hintOnly: string; noDeadline: string; noExecute: string; noUpload: string }
    > = {
      en: {
        hintOnly: 'hints only',
        noDeadline: 'does not promise',
        noExecute: 'no Execute or Undo control',
        noUpload: 'No uploads',
      },
      'zh-TW': {
        hintOnly: '僅是提示',
        noDeadline: '不保證',
        noExecute: '不提供執行或 Undo 控制項',
        noUpload: '不上傳',
      },
      'zh-CN': {
        hintOnly: '仅是提示',
        noDeadline: '不保证',
        noExecute: '不提供执行或 Undo 控件',
        noUpload: '不上传',
      },
      ja: {
        hintOnly: 'ヒントにすぎません',
        noDeadline: '保証するものではありません',
        noExecute: '実行または Undo のコントロールは表示しません',
        noUpload: 'アップロードなし',
      },
    };

    for (const locale of LOCALES) {
      const phrases = safetyPhrases[locale];
      expect(catalogs[locale].watch.description).toContain(phrases.hintOnly);
      expect(catalogs[locale].watch.description).toContain(phrases.noDeadline);
      expect(catalogs[locale].actions.noExecute).toContain(phrases.noExecute);
      expect(catalogs[locale].footer.description).toContain(phrases.noUpload);
      expect(catalogs[locale].extraction.optInEmpty.length).toBeGreaterThan(20);
    }
  });

  it('keeps Smart Cleanup Inbox suggestion-only and localized in every catalog', () => {
    const phrases: Record<
      Locale,
      { suggestion: string; noChange: string; evidenceScore: string; screenshotDisclaimer: string }
    > = {
      en: {
        suggestion: 'Suggestions only',
        noChange: 'no file changes',
        evidenceScore: 'evidence score',
        screenshotDisclaimer: 'do not prove',
      },
      'zh-TW': {
        suggestion: '僅建議',
        noChange: '不變更檔案',
        evidenceScore: '證據分數',
        screenshotDisclaimer: '不證明',
      },
      'zh-CN': {
        suggestion: '仅建议',
        noChange: '不更改文件',
        evidenceScore: '证据分数',
        screenshotDisclaimer: '不证明',
      },
      ja: {
        suggestion: '候補のみ',
        noChange: 'ファイルは変更しません',
        evidenceScore: '証拠スコア',
        screenshotDisclaimer: '証明しません',
      },
    };

    for (const locale of LOCALES) {
      expect(catalogs[locale].cleanup.suggestionOnly).toContain(phrases[locale].suggestion);
      expect(catalogs[locale].cleanup.suggestionOnly).toContain(phrases[locale].noChange);
      expect(catalogs[locale].cleanup.itemMeta(2, 6_000, 'date')).toContain(
        phrases[locale].evidenceScore,
      );
      expect(catalogs[locale].cleanup.itemMeta(2, 6_000, 'date')).not.toContain('%');
      expect(catalogs[locale].cleanup.screenshotReviewGroupExplanation).toContain(
        phrases[locale].screenshotDisclaimer,
      );
      expect(catalogs[locale].cleanup.verification.length).toBeGreaterThan(30);
    }
    expect(appSource).toContain('refreshSmartCleanupInbox(cleanupScopeId)');
    expect(appSource).toContain('getCleanupSourceDetail(item)');
    expect(appSource).toContain('createCleanupActionPreview(detail, targetNodeId, keeperNodeId)');
    expect(appSource).toContain("kind: inbox.evaluation_complete ? 'ready' : 'partial'");
  });

  it('keeps explicit cleanup Preview review transient and non-executable in every catalog', () => {
    const phrases: Record<Locale, { transient: string; previewOnly: string }> = {
      en: { transient: 'not saved', previewOnly: 'Preview only' },
      'zh-TW': { transient: '不會寫入', previewOnly: '僅供預覽' },
      'zh-CN': { transient: '不会写入', previewOnly: '仅供预览' },
      ja: { transient: '保存しません', previewOnly: 'プレビューのみ' },
    };

    for (const locale of LOCALES) {
      expect(catalogs[locale].cleanup.review.transientNotice).toContain(phrases[locale].transient);
      expect(catalogs[locale].cleanup.review.noExecution).toContain(phrases[locale].previewOnly);
      expect(catalogs[locale].cleanup.review.previewReady(12)).toBeTruthy();
      expect(catalogs[locale].cleanup.review.roles.olderVersion).toBeTruthy();
      expect(catalogs[locale].cleanup.review.roles.newerVersion).toBeTruthy();
      expect(catalogs[locale].cleanup.review.noKeeper).toBeTruthy();
    }
    expect(appSource).toContain('cleanupReviewGenerationRef.current !== generation');
    expect(appSource).toContain("if (nextView !== 'inbox') invalidateCleanupReview()");
    expect(appSource).toContain("cleanupReviewState.kind === 'creating'");
  });

  it('keeps primary navigation complete and honest in every catalog', () => {
    const phrases: Record<
      Locale,
      {
        localOnly: string;
        noNetwork: string;
        projectsNotDiscovery: string;
        inboxNoAction: string;
        historyNoAction: string;
      }
    > = {
      en: {
        localOnly: 'Local only',
        noNetwork: 'No network required',
        projectsNotDiscovery: 'not available',
        inboxNoAction: 'cannot change, trash, delete, or undo',
        historyNoAction: 'does not execute or undo',
      },
      'zh-TW': {
        localOnly: '僅限本機',
        noNetwork: '不需要網路',
        projectsNotDiscovery: '尚未提供',
        inboxNoAction: '無法變更、移至垃圾桶、刪除或復原',
        historyNoAction: '不會執行或復原',
      },
      'zh-CN': {
        localOnly: '仅限本机',
        noNetwork: '不需要网络',
        projectsNotDiscovery: '尚未提供',
        inboxNoAction: '无法更改、移至废纸篓、删除或撤销',
        historyNoAction: '不会执行或撤销',
      },
      ja: {
        localOnly: 'ローカルのみ',
        noNetwork: 'ネットワーク不要',
        projectsNotDiscovery: 'まだ利用できません',
        inboxNoAction: '変更、ゴミ箱への移動、削除、Undo はできません',
        historyNoAction: '実行またはUndoできません',
      },
    };
    const views = ['home', 'search', 'projects', 'inbox', 'history', 'settings'] as const;

    for (const locale of LOCALES) {
      const navigation = catalogs[locale].navigation;
      expect(navigation.ariaLabel.length).toBeGreaterThan(0);
      expect(navigation.skipToContent.length).toBeGreaterThan(0);
      expect(navigation.brandDescription.length).toBeGreaterThan(0);
      expect(navigation.localOnly).toContain(phrases[locale].localOnly);
      expect(navigation.noNetwork).toContain(phrases[locale].noNetwork);
      for (const view of views) {
        expect(navigation.views[view].label.length).toBeGreaterThan(0);
        expect(navigation.views[view].title.length).toBeGreaterThan(0);
        expect(navigation.views[view].description.length).toBeGreaterThan(0);
      }
      expect(navigation.views.projects.description).toContain(phrases[locale].projectsNotDiscovery);
      expect(navigation.views.inbox.description).toContain(phrases[locale].inboxNoAction);
      expect(navigation.views.history.description).toContain(phrases[locale].historyNoAction);
    }
  });

  it('keeps catalogs text-only and user-owned values outside static message data', () => {
    const htmlElement = /<\/?[a-z][^>]*>/i;
    for (const locale of LOCALES) {
      for (const value of staticStrings(catalogs[locale])) {
        expect(value).not.toMatch(htmlElement);
      }
    }

    const sentinel = '<script>ignore()</script> /Users/私人/../prompt-injection';
    for (const locale of LOCALES) {
      const empty = catalogs[locale].search.empty(sentinel);
      const scope = catalogs[locale].actions.scopeOption(3, sentinel);
      expect(occurrences(empty, sentinel)).toBe(1);
      expect(occurrences(scope, sentinel)).toBe(1);
    }
  });
});

describe('locale selection and storage', () => {
  it('uses any stored allowlisted locale before browser preferences', () => {
    for (const locale of LOCALES) {
      expect(resolveLocale(locale, ['en-US'])).toBe(locale);
    }
  });

  it('maps English, Traditional Chinese, Simplified Chinese, and Japanese tags', () => {
    for (const tag of ['en', 'en-US', 'en-Latn-US-u-ca-gregory']) {
      expect(resolveLocale(null, [tag])).toBe('en');
    }
    for (const tag of [
      'zh',
      'zh-TW',
      'zh-Hant',
      'zh-Hant-HK',
      'zh-HK',
      'zh-MO-x-private',
      'zh-TW-u-ca-chinese',
    ]) {
      expect(resolveLocale(null, [tag])).toBe('zh-TW');
    }
    for (const tag of ['zh-CN', 'zh-SG', 'zh-MY', 'zh-Hans', 'zh-Hans-CN', 'zh-CN-u-nu-hanidec']) {
      expect(resolveLocale(null, [tag])).toBe('zh-CN');
    }
    for (const tag of ['ja', 'ja-JP', 'ja-JP-u-ca-japanese']) {
      expect(resolveLocale(null, [tag])).toBe('ja');
    }
  });

  it('walks preferences in order and ignores unsupported, malformed, or ambiguous tags', () => {
    expect(resolveLocale(null, ['fr-FR', 'ja-JP', 'en-US'])).toBe('ja');
    expect(resolveLocale(null, ['fr-FR', 'zh-CN', 'zh-TW'])).toBe('zh-CN');
    expect(resolveLocale(null, ['zh_Hant_TW', 'ja-JP'])).toBe('ja');
    expect(resolveLocale(null, ['zh-Hans-TW', 'en-US'])).toBe('en');
    expect(resolveLocale(null, [42, null, 'zh-TW'])).toBe('zh-TW');
    expect(resolveLocale('private-invalid-value', ['unknown', ''])).toBe('en');
  });

  it('keeps navigator.languages order and falls back to navigator.language once', () => {
    expect(collectLanguagePreferences([], 'zh-Hant-TW')).toEqual(['zh-Hant-TW']);
    expect(collectLanguagePreferences(['fr-FR'], 'ja-JP')).toEqual(['fr-FR', 'ja-JP']);
    expect(collectLanguagePreferences(['en-US'], 'en-US')).toEqual(['en-US']);
    expect(collectLanguagePreferences(undefined, undefined)).toEqual([]);
  });

  it('reads and writes only allowlisted values and handles storage exceptions', () => {
    const writes: Array<[string, string]> = [];
    const storage = {
      getItem: () => 'private-invalid-value',
      setItem: (key: string, value: string) => writes.push([key, value]),
    };
    expect(readStoredLocale(storage)).toBeNull();
    expect(loadLocale(storage, ['zh-CN'])).toBe('zh-CN');
    expect(writes).toEqual([]);
    expect(storeLocale(storage, 'fr-FR')).toBe(false);
    expect(storeLocale(storage, { locale: 'ja' })).toBe(false);
    expect(writes).toEqual([]);
    for (const locale of LOCALES) expect(storeLocale(storage, locale)).toBe(true);
    expect(writes).toEqual(LOCALES.map((locale) => [LOCALE_STORAGE_KEY, locale]));

    const failing = {
      getItem: () => {
        throw new Error('private storage failure');
      },
      setItem: () => {
        throw new Error('private storage failure');
      },
    };
    expect(loadLocale(failing, ['ja-JP'])).toBe('ja');
    expect(storeLocale(failing, 'ja')).toBe(false);
  });
});

describe('localized helpers and UI wiring', () => {
  it('localizes metadata and formatting without altering caller-owned content', () => {
    expect(getLocalizedMetadata('zh-TW')).toMatchObject({
      htmlLang: 'zh-TW',
      dir: 'ltr',
      title: 'DeskGraph — 預先發行版',
    });
    expect(getLocalizedMetadata('zh-CN').title).toBe('DeskGraph — 预发布版');
    expect(getLocalizedMetadata('ja').title).toBe('DeskGraph — プレリリース');
    expect(formatInteger(1234567, 'en')).toBe('1,234,567');
    for (const locale of LOCALES) {
      expect(formatInteger(1234567, locale)).toContain('1');
      const formatted = formatUtcDate('2026-07-18T23:00:00-04:00', locale);
      expect(formatted).toContain('2026');
      expect(formatted).not.toContain('18');
    }
  });

  it('formats plural-sensitive and dynamic safety copy', () => {
    expect(englishCount(0, 'result')).toBe('0 results');
    expect(englishCount(1, 'result')).toBe('1 result');
    expect(englishCount(2, 'result')).toBe('2 results');
    expect(catalogs.en.watch.event(1, 2, 1)).toContain('1 coalesced hint');
    expect(catalogs.en.watch.event(1, 2, 2)).toContain('2 coalesced hints');
    expect(catalogs['zh-TW'].scope.validation.complete(2, 1)).toBe(
      '掃描完成：2 個檔案與 1 個資料夾。',
    );
    expect(catalogs['zh-CN'].scope.validation.complete(2, 1)).toContain('2 个文件');
    expect(catalogs.ja.scope.validation.complete(2, 1)).toContain('2 個のファイル');
  });

  it('labels durable action history states while keeping the Desktop surface preview-only', () => {
    for (const locale of LOCALES) {
      const history = catalogs[locale].actions.historyState;
      expect(history.previewed.length).toBeGreaterThan(0);
      expect(history.pending.length).toBeGreaterThan(0);
      expect(history.executed.length).toBeGreaterThan(0);
      expect(history.undone.length).toBeGreaterThan(0);
      expect(history.needsAttention.length).toBeGreaterThan(0);
      expect(catalogs[locale].actions.noExecute).toMatch(/Desktop/);
    }
    expect(appSource).toContain('actionPlanStateLabel(plan.state, catalog)');
    expect(appSource).not.toContain('execute_action_plan');
    expect(appSource).not.toContain('undo_action_plan');
  });

  it('generates the selector from the registry and updates document language and direction', () => {
    expect(appSource).toContain('localeOptions.map');
    expect(appSource).toContain('document.documentElement.lang = metadata.htmlLang');
    expect(appSource).toContain('document.documentElement.dir = metadata.dir');
    expect(appSource).toContain('<main id="main-content" className="app-shell" tabIndex={-1}>');
    expect(appSource).toContain('<h1 ref={viewHeadingRef} tabIndex={-1}>');
    expect(appSource).not.toMatch(/<option\s+value=["'](?:en|zh-TW|zh-CN|ja)["']/);
    expect(appSource).not.toContain('className="dashboard" aria-live="polite"');
  });
});
