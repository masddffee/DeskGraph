import { invoke } from '@tauri-apps/api/core';

export const SEARCH_LOCAL_COMMAND = 'search_local';

export type SearchMatchedField = 'metadata_path' | 'extracted_text';

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
  result_count: number;
  results: SearchResult[];
  elapsed_ms: number;
}

type InvokeCommand = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

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
  const valid =
    value.api_version === 'deskgraph.search.v1' &&
    value.mode === 'lexical' &&
    value.embeddings_enabled === false &&
    typeof value.query === 'string' &&
    isCount(value.result_count) &&
    value.result_count === results.length &&
    isCount(value.elapsed_ms);
  if (!valid) throw new Error('Invalid search response');
  return { ...(value as unknown as SearchResponse), results };
}

export async function searchLocal(
  query: string,
  scopeId: number | null = null,
  limit = 20,
  invokeCommand: InvokeCommand = (command, args) => invoke(command, args),
): Promise<SearchResponse> {
  return parseSearchResponse(await invokeCommand(SEARCH_LOCAL_COMMAND, { query, scopeId, limit }));
}
