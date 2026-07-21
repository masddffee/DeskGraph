import { invoke } from '@tauri-apps/api/core';

export const SEARCH_LOCAL_COMMAND = 'search_local';
export const LIST_SEARCH_FOLDERS_COMMAND = 'list_search_folders';
const MAX_FILTER_UNIX_SECONDS = 9_223_372_036;

export type SearchMatchedField = 'metadata_path' | 'extracted_text';
export type SearchSourceFilter = 'all' | 'metadata_path' | 'extracted_text';

export interface SearchFilters {
  scope_id: number | null;
  folder_node_id: number | null;
  source: SearchSourceFilter;
  extension: string | null;
  modified_since_unix_seconds: number | null;
  modified_before_unix_seconds: number | null;
}

export interface SearchFolder {
  scope_id: number;
  folder_node_id: number;
  display_path: string;
}

export interface SearchFoldersResponse {
  api_version: 'deskgraph.search-folders.v1';
  scope_id: number;
  folder_count: number;
  folders: SearchFolder[];
  truncated: boolean;
}

export interface SearchResult {
  scope_id: number;
  node_id: number;
  location_id: number;
  display_path: string;
  snippet: string | null;
  matched_fields: SearchMatchedField[];
  explanation:
    | 'exact_filename_and_extracted_text'
    | 'exact_filename'
    | 'path_and_extracted_text_substring'
    | 'path_substring'
    | 'extracted_text_substring';
  lexical_rank: number;
}

export interface SearchResponse {
  api_version: 'deskgraph.search.v1';
  mode: 'lexical';
  embeddings_enabled: false;
  query: string;
  filters: SearchFilters;
  result_count: number;
  results: SearchResult[];
  elapsed_ms: number;
}

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export interface SearchOptions {
  scopeId?: number | null;
  folderNodeId?: number | null;
  source?: SearchSourceFilter;
  extension?: string | null;
  modifiedSinceUnixSeconds?: number | null;
  modifiedBeforeUnixSeconds?: number | null;
  limit?: number;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function isId(value: unknown): value is number {
  return isCount(value) && value > 0;
}

function isMatchedField(value: unknown): value is SearchMatchedField {
  return value === 'metadata_path' || value === 'extracted_text';
}

function isSourceFilter(value: unknown): value is SearchSourceFilter {
  return value === 'all' || value === 'metadata_path' || value === 'extracted_text';
}

function isNullableId(value: unknown): value is number | null {
  return value === null || isId(value);
}

function isNullableUnixSeconds(value: unknown): value is number | null {
  return value === null || (isCount(value) && value <= MAX_FILTER_UNIX_SECONDS);
}

function parseSearchFilters(value: unknown): SearchFilters {
  if (!isRecord(value)) throw new Error('Invalid search filter response');
  const valid =
    isNullableId(value.scope_id) &&
    isNullableId(value.folder_node_id) &&
    !(value.scope_id === null && value.folder_node_id !== null) &&
    isSourceFilter(value.source) &&
    (value.extension === null ||
      (typeof value.extension === 'string' && /^[a-z0-9]{1,16}$/.test(value.extension))) &&
    isNullableUnixSeconds(value.modified_since_unix_seconds) &&
    isNullableUnixSeconds(value.modified_before_unix_seconds) &&
    !(
      value.modified_since_unix_seconds !== null &&
      value.modified_before_unix_seconds !== null &&
      value.modified_since_unix_seconds >= value.modified_before_unix_seconds
    );
  if (!valid) throw new Error('Invalid search filter response');
  return value as unknown as SearchFilters;
}

function parseSearchFolder(value: unknown): SearchFolder {
  if (!isRecord(value)) throw new Error('Invalid search folders response');
  const valid =
    isId(value.scope_id) &&
    isId(value.folder_node_id) &&
    typeof value.display_path === 'string' &&
    value.display_path.length > 0;
  if (!valid) throw new Error('Invalid search folders response');
  return value as unknown as SearchFolder;
}

export function parseSearchFoldersResponse(value: unknown): SearchFoldersResponse {
  if (!isRecord(value) || !Array.isArray(value.folders)) {
    throw new Error('Invalid search folders response');
  }
  const folders = value.folders.map(parseSearchFolder);
  const valid =
    value.api_version === 'deskgraph.search-folders.v1' &&
    isId(value.scope_id) &&
    isCount(value.folder_count) &&
    value.folder_count === folders.length &&
    typeof value.truncated === 'boolean' &&
    folders.every((folder) => folder.scope_id === value.scope_id) &&
    new Set(folders.map((folder) => folder.folder_node_id)).size === folders.length;
  if (!valid) throw new Error('Invalid search folders response');
  return { ...(value as unknown as SearchFoldersResponse), folders };
}

function isExplanation(value: unknown): value is SearchResult['explanation'] {
  return (
    value === 'exact_filename_and_extracted_text' ||
    value === 'exact_filename' ||
    value === 'path_and_extracted_text_substring' ||
    value === 'path_substring' ||
    value === 'extracted_text_substring'
  );
}

export function parseSearchResult(value: unknown): SearchResult {
  if (!isRecord(value)) throw new Error('Invalid search result response');
  const valid =
    isId(value.scope_id) &&
    isId(value.node_id) &&
    isId(value.location_id) &&
    typeof value.display_path === 'string' &&
    value.display_path.length > 0 &&
    (value.snippet === null || typeof value.snippet === 'string') &&
    Array.isArray(value.matched_fields) &&
    value.matched_fields.length > 0 &&
    value.matched_fields.length <= 2 &&
    value.matched_fields.every(isMatchedField) &&
    new Set(value.matched_fields).size === value.matched_fields.length &&
    isExplanation(value.explanation) &&
    isId(value.lexical_rank);
  if (!valid) throw new Error('Invalid search result response');
  return value as unknown as SearchResult;
}

export function parseSearchResponse(value: unknown): SearchResponse {
  if (!isRecord(value) || !Array.isArray(value.results)) {
    throw new Error('Invalid search response');
  }
  const results = value.results.map(parseSearchResult);
  const filters = parseSearchFilters(value.filters);
  const valid =
    value.api_version === 'deskgraph.search.v1' &&
    value.mode === 'lexical' &&
    value.embeddings_enabled === false &&
    typeof value.query === 'string' &&
    isCount(value.result_count) &&
    value.result_count === results.length &&
    isCount(value.elapsed_ms);
  if (!valid) throw new Error('Invalid search response');
  return { ...(value as unknown as SearchResponse), filters, results };
}

export async function searchLocal(
  query: string,
  options: SearchOptions = {},
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<SearchResponse> {
  const scopeId = options.scopeId ?? null;
  const folderNodeId = options.folderNodeId ?? null;
  if (
    !isNullableId(scopeId) ||
    !isNullableId(folderNodeId) ||
    (folderNodeId !== null && scopeId === null)
  ) {
    throw new Error('Invalid search options');
  }
  return parseSearchResponse(
    await invokeCommand(SEARCH_LOCAL_COMMAND, {
      query,
      filters: {
        scope_id: scopeId,
        folder_node_id: folderNodeId,
        source: options.source ?? 'all',
        extension: options.extension?.trim() || null,
        modified_since_unix_seconds: options.modifiedSinceUnixSeconds ?? null,
        modified_before_unix_seconds: options.modifiedBeforeUnixSeconds ?? null,
      },
      limit: options.limit ?? 20,
    }),
  );
}

export async function listSearchFolders(
  scopeId: number,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<SearchFoldersResponse> {
  if (!isId(scopeId)) throw new Error('Invalid search folder scope');
  const response = parseSearchFoldersResponse(
    await invokeCommand(LIST_SEARCH_FOLDERS_COMMAND, { scopeId }),
  );
  if (response.scope_id !== scopeId) throw new Error('Invalid search folders response');
  return response;
}
