import { describe, expect, it, vi } from 'vitest';

import {
  SEARCH_LOCAL_COMMAND,
  parseSearchResponse,
  parseSearchResult,
  searchLocal,
  type SearchResponse,
  type SearchResult,
} from './search';

const result: SearchResult = {
  scope_id: 1,
  node_id: 2,
  location_id: 3,
  display_path: '/authorized/專案-context.md',
  snippet: 'Traditional Chinese [專案脈絡] and English context',
  matched_fields: ['metadata_path', 'extracted_text'],
  explanation: 'path_and_extracted_text_substring',
  lexical_rank: 1,
};

const response: SearchResponse = {
  api_version: 'deskgraph.search.v1',
  mode: 'lexical',
  embeddings_enabled: false,
  query: '專案 context',
  result_count: 1,
  results: [result],
  elapsed_ms: 3,
};

describe('search contract', () => {
  it('accepts the closed lexical result and preserves untrusted text as a string', () => {
    expect(parseSearchResponse(response)).toEqual(response);
    expect(
      parseSearchResult({ ...result, snippet: '<script>untrusted text only</script>' }).snippet,
    ).toBe('<script>untrusted text only</script>');
  });

  it('rejects unknown explanations, duplicate fields, and inconsistent counts', () => {
    expect(() => parseSearchResult({ ...result, explanation: 'model_said_so' })).toThrow(
      'Invalid search result response',
    );
    expect(() =>
      parseSearchResult({ ...result, matched_fields: ['metadata_path', 'metadata_path'] }),
    ).toThrow('Invalid search result response');
    expect(() => parseSearchResponse({ ...response, result_count: 2 })).toThrow(
      'Invalid search response',
    );
  });

  it('uses one narrow read-only Tauri command with explicit bounds', async () => {
    const invokeCommand = vi.fn().mockResolvedValue(response);
    await expect(searchLocal('專案 context', 1, 20, invokeCommand)).resolves.toEqual(response);
    expect(invokeCommand).toHaveBeenCalledWith(SEARCH_LOCAL_COMMAND, {
      query: '專案 context',
      scopeId: 1,
      limit: 20,
    });
  });
});
