import { type Catalog } from './localization/catalog';
import { en } from './localization/en';
import { ja } from './localization/ja';
import { zhCN } from './localization/zhCN';
import { zhTW } from './localization/zhTW';

export type { Catalog } from './localization/catalog';
export { englishCount } from './localization/catalog';

export interface LocaleStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export const LOCALE_STORAGE_KEY = 'deskgraph.locale';

export type TextDirection = 'ltr' | 'rtl';

type LocaleDefinition = {
  autonym: string;
  htmlLang: string;
  dir: TextDirection;
  catalog: Catalog;
};

export const localeRegistry = {
  en: {
    autonym: 'English',
    htmlLang: 'en',
    dir: 'ltr',
    catalog: en,
  },
  'zh-TW': {
    autonym: '繁體中文',
    htmlLang: 'zh-TW',
    dir: 'ltr',
    catalog: zhTW,
  },
  'zh-CN': {
    autonym: '简体中文',
    htmlLang: 'zh-CN',
    dir: 'ltr',
    catalog: zhCN,
  },
  ja: {
    autonym: '日本語',
    htmlLang: 'ja',
    dir: 'ltr',
    catalog: ja,
  },
} as const satisfies Record<string, LocaleDefinition>;

export type Locale = keyof typeof localeRegistry;

export const LOCALES = Object.freeze(Object.keys(localeRegistry) as Locale[]);

export const catalogs = Object.freeze(
  Object.fromEntries(LOCALES.map((locale) => [locale, localeRegistry[locale].catalog])),
) as Readonly<Record<Locale, Catalog>>;

export type LocaleOption = {
  value: Locale;
  label: string;
};

export const localeOptions = Object.freeze(
  LOCALES.map((locale) =>
    Object.freeze({
      value: locale,
      label: localeRegistry[locale].autonym,
    }),
  ),
) as readonly LocaleOption[];

export function isLocale(value: unknown): value is Locale {
  return typeof value === 'string' && Object.prototype.hasOwnProperty.call(localeRegistry, value);
}

type ParsedBrowserLocale = {
  language: string;
  script: string | null;
  region: string | null;
  hasVariant: boolean;
};

function parseBrowserLocale(value: unknown): ParsedBrowserLocale | null {
  if (typeof value !== 'string') return null;
  const tag = value.trim();
  if (tag.length === 0 || tag.length > 255 || !/^[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*$/.test(tag)) {
    return null;
  }

  const parts = tag.toLowerCase().split('-');
  const language = parts[0];
  let index = 1;
  let script: string | null = null;
  let region: string | null = null;
  let hasVariant = false;

  if (index < parts.length && /^[a-z]{4}$/.test(parts[index])) {
    script = parts[index];
    index += 1;
  }
  if (index < parts.length && (/^[a-z]{2}$/.test(parts[index]) || /^\d{3}$/.test(parts[index]))) {
    region = parts[index];
    index += 1;
  }
  while (
    index < parts.length &&
    (/^[a-z0-9]{5,8}$/.test(parts[index]) || /^\d[a-z0-9]{3}$/.test(parts[index]))
  ) {
    hasVariant = true;
    index += 1;
  }

  const extensionSingletons = new Set<string>();
  while (index < parts.length && /^[0-9a-wy-z]$/.test(parts[index])) {
    const singleton = parts[index];
    if (extensionSingletons.has(singleton)) return null;
    extensionSingletons.add(singleton);
    index += 1;
    const extensionStart = index;
    while (index < parts.length && /^[a-z0-9]{2,8}$/.test(parts[index])) index += 1;
    if (index === extensionStart) return null;
  }

  if (index < parts.length && parts[index] === 'x') {
    index += 1;
    const privateUseStart = index;
    while (index < parts.length && /^[a-z0-9]{1,8}$/.test(parts[index])) index += 1;
    if (index === privateUseStart) return null;
  }

  if (index !== parts.length) return null;
  return { language, script, region, hasVariant };
}

function matchBrowserLocale(value: unknown): Locale | null {
  const parsed = parseBrowserLocale(value);
  if (!parsed) return null;

  const { language, script, region, hasVariant } = parsed;
  if (language === 'en') return 'en';
  if (language === 'ja') return 'ja';
  if (language !== 'zh') return null;

  if (!script && !region && !hasVariant) return 'zh-TW';

  const simplified = script === 'hans' || ['cn', 'sg', 'my'].includes(region ?? '');
  const traditional = script === 'hant' || ['tw', 'hk', 'mo'].includes(region ?? '');

  if (simplified === traditional) return null;
  return simplified ? 'zh-CN' : 'zh-TW';
}

export function resolveLocale(
  storedValue: unknown,
  navigatorLanguages: readonly unknown[] = [],
): Locale {
  if (isLocale(storedValue)) return storedValue;
  for (const language of navigatorLanguages) {
    const matched = matchBrowserLocale(language);
    if (matched) return matched;
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
  navigatorLanguages: readonly unknown[] = [],
): Locale {
  return resolveLocale(readStoredLocale(storage), navigatorLanguages);
}

export function storeLocale(storage: Pick<LocaleStorage, 'setItem'>, locale: unknown): boolean {
  if (!isLocale(locale)) return false;
  try {
    storage.setItem(LOCALE_STORAGE_KEY, locale);
    return true;
  } catch {
    return false;
  }
}

export function getCatalog(locale: Locale): Catalog {
  return localeRegistry[locale].catalog;
}

export type LocalizedMetadata = Catalog['metadata'] & {
  htmlLang: string;
  dir: TextDirection;
};

export function getLocalizedMetadata(locale: Locale): LocalizedMetadata {
  const definition = localeRegistry[locale];
  return {
    ...definition.catalog.metadata,
    htmlLang: definition.htmlLang,
    dir: definition.dir,
  };
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
