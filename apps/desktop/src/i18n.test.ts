import { describe, expect, it } from 'vitest';

import {
  LOCALE_STORAGE_KEY,
  catalogs,
  collectLanguagePreferences,
  formatInteger,
  formatUtcDate,
  englishCount,
  getLocalizedMetadata,
  loadLocale,
  readStoredLocale,
  resolveLocale,
  storeLocale,
} from './i18n';

function shape(value: unknown): unknown {
  if (typeof value === 'function') return `function:${value.length}`;
  if (Array.isArray(value)) return value.map(shape);
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, shape(child)]));
  }
  return typeof value;
}

describe('i18n catalog contract', () => {
  it('keeps the English and Traditional Chinese nested key/function shape identical', () => {
    expect(shape(catalogs.en)).toEqual(shape(catalogs['zh-TW']));
  });

  it('contains translated critical loading, safe-error, OCR, scan, and aria messages', () => {
    expect(catalogs.en.loading.heading).toBe('Opening the local manifest');
    expect(catalogs['zh-TW'].backendError.retry).toBe('重試');
    expect(catalogs['zh-TW'].search.ocr.controlsAria).toBe('本機截圖 OCR 控制項');
    expect(catalogs['zh-TW'].scope.status.scanning).toBe('正在掃描中繼資料…');
    expect(catalogs.en.actions.policyAria).toBe('Passed policy checks');
  });

  it('labels the polling fallback and deferred backlog honestly in both locales', () => {
    expect(catalogs.en.watch.heading).toBe('Bounded metadata polling fallback');
    expect(catalogs.en.watch.metrics.deferred).toBe('Deferred folders');
    expect(catalogs.en.watch.description).toContain('It is not native event watching');
    expect(catalogs['zh-TW'].watch.heading).toBe('受限的 metadata 輪詢備援');
    expect(catalogs['zh-TW'].watch.metrics.deferred).toBe('延後資料夾');
    expect(catalogs['zh-TW'].watch.description).toContain('這不是原生事件監看');
  });
});

describe('locale selection and storage', () => {
  it('uses a stored allowlisted locale before browser tags', () => {
    expect(resolveLocale('en', ['zh-TW'])).toBe('en');
    expect(resolveLocale('zh-TW', ['en-US'])).toBe('zh-TW');
  });

  it('maps Traditional Chinese tags to zh-TW and other Chinese tags safely to English', () => {
    for (const tag of ['zh', 'zh-TW', 'zh-Hant', 'zh-Hant-TW', 'zh-HK', 'zh-MO']) {
      expect(resolveLocale(null, [tag])).toBe('zh-TW');
    }
    for (const tag of ['zh-CN', 'zh-SG', 'zh-Hans', 'en-US']) {
      expect(resolveLocale(null, [tag])).toBe('en');
    }
  });

  it('uses the first recognizable browser preference, including English variants', () => {
    expect(resolveLocale(null, ['en-US', 'zh-TW'])).toBe('en');
    expect(resolveLocale(null, ['fr-FR', 'zh-Hant-HK', 'en-US'])).toBe('zh-TW');
    expect(resolveLocale(null, ['fr-FR', 'zh-CN', 'zh-TW'])).toBe('en');
  });

  it('keeps navigator.languages order and falls back to navigator.language once', () => {
    expect(collectLanguagePreferences([], 'zh-Hant-TW')).toEqual(['zh-Hant-TW']);
    expect(collectLanguagePreferences(['fr-FR'], 'zh-TW')).toEqual(['fr-FR', 'zh-TW']);
    expect(collectLanguagePreferences(['en-US'], 'en-US')).toEqual(['en-US']);
    expect(collectLanguagePreferences(undefined, undefined)).toEqual([]);
  });

  it('does not persist while reading, and treats corrupt values and storage exceptions as absent', () => {
    const writes: Array<[string, string]> = [];
    const storage = {
      getItem: () => 'private-invalid-value',
      setItem: (key: string, value: string) => writes.push([key, value]),
    };
    expect(readStoredLocale(storage)).toBeNull();
    expect(loadLocale(storage, ['zh-TW'])).toBe('zh-TW');
    expect(writes).toEqual([]);
    expect(storeLocale(storage, 'en')).toBe(true);
    expect(writes).toEqual([[LOCALE_STORAGE_KEY, 'en']]);
    const failing = {
      getItem: () => {
        throw new Error('private storage failure');
      },
      setItem: () => {
        throw new Error('private storage failure');
      },
    };
    expect(loadLocale(failing, ['zh-TW'])).toBe('zh-TW');
    expect(storeLocale(failing, 'zh-TW')).toBe(false);
  });
});

describe('localized helpers and dynamic messages', () => {
  it('localizes metadata, integer/date formatting, and does not alter caller-owned content', () => {
    expect(getLocalizedMetadata('zh-TW')).toMatchObject({
      htmlLang: 'zh-TW',
      title: 'DeskGraph — 預先發行版',
    });
    expect(formatInteger(1234567, 'en')).toBe('1,234,567');
    expect(formatInteger(1234567, 'zh-TW')).toMatch(/1,234,567/);
    expect(formatUtcDate('2026-07-18T23:00:00-04:00', 'en')).toBe('07/19/2026');
    expect(catalogs['zh-TW'].search.empty('/private/query.txt')).toContain('/private/query.txt');
    expect(catalogs.en.actions.scopeOption(3, '/private/path')).toContain('/private/path');
  });

  it('formats plural-sensitive and dynamic safety copy for both locales', () => {
    expect(englishCount(0, 'result')).toBe('0 results');
    expect(englishCount(1, 'result')).toBe('1 result');
    expect(englishCount(2, 'result')).toBe('2 results');
    expect(catalogs.en.watch.event(1, 2, 1)).toContain('1 coalesced hint');
    expect(catalogs.en.watch.event(1, 2, 2)).toContain('2 coalesced hints');
    expect(catalogs['zh-TW'].scope.validation.complete(2, 1)).toBe(
      '掃描完成：2 個檔案與 1 個資料夾。',
    );
    expect(catalogs.en.search.summary(1, 7)).toBe('1 result · 7 ms');
  });
});
