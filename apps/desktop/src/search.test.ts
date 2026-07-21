import { describe, expect, it, vi } from 'vitest';

import {
  LIST_SEARCH_FOLDERS_COMMAND,
  SEARCH_LOCAL_COMMAND,
  listSearchFolders,
  parseSearchFoldersResponse,
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
  filters: {
    scope_id: 1,
    folder_node_id: 4,
    source: 'extracted_text',
    extension: 'md',
    modified_since_unix_seconds: 1,
    modified_before_unix_seconds: 2,
  },
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
    expect(() =>
      parseSearchResponse({
        ...response,
        filters: { ...response.filters, extension: '../md' },
      }),
    ).toThrow('Invalid search filter response');
    expect(() =>
      parseSearchResponse({
        ...response,
        filters: {
          ...response.filters,
          folder_node_id: null,
          modified_since_unix_seconds: 2,
          modified_before_unix_seconds: 2,
        },
      }),
    ).toThrow('Invalid search filter response');
    expect(() =>
      parseSearchResponse({
        ...response,
        filters: { ...response.filters, scope_id: null },
      }),
    ).toThrow('Invalid search filter response');
  });

  it('uses one narrow read-only Tauri command with explicit bounds', async () => {
    const invokeCommand = vi.fn().mockResolvedValue(response);
    await expect(
      searchLocal(
        '專案 context',
        {
          scopeId: 1,
          folderNodeId: 4,
          source: 'extracted_text',
          extension: '.MD',
          modifiedSinceUnixSeconds: 1,
          modifiedBeforeUnixSeconds: 2,
          limit: 20,
        },
        invokeCommand,
      ),
    ).resolves.toEqual(response);
    expect(invokeCommand).toHaveBeenCalledWith(SEARCH_LOCAL_COMMAND, {
      query: '專案 context',
      filters: {
        scope_id: 1,
        folder_node_id: 4,
        source: 'extracted_text',
        extension: '.MD',
        modified_since_unix_seconds: 1,
        modified_before_unix_seconds: 2,
      },
      limit: 20,
    });
  });

  it('rejects folder filters without one valid scope before invoking Tauri', async () => {
    const invokeCommand = vi.fn();
    await expect(searchLocal('context', { folderNodeId: 4 }, invokeCommand)).rejects.toThrow(
      'Invalid search options',
    );
    await expect(searchLocal('context', { scopeId: 0 }, invokeCommand)).rejects.toThrow(
      'Invalid search options',
    );
    expect(invokeCommand).not.toHaveBeenCalled();
  });

  it('accepts folder choices from only their requested scope', async () => {
    const folders = {
      api_version: 'deskgraph.search-folders.v1' as const,
      scope_id: 1,
      folder_count: 1,
      folders: [
        {
          scope_id: 1,
          folder_node_id: 4,
          display_path: '/authorized/notes',
        },
      ],
      truncated: false,
    };
    expect(parseSearchFoldersResponse(folders)).toEqual(folders);
    await expect(listSearchFolders(1, vi.fn().mockResolvedValue(folders))).resolves.toEqual(
      folders,
    );
    await expect(
      listSearchFolders(
        1,
        vi.fn().mockResolvedValue({
          ...folders,
          scope_id: 2,
          folders: [{ ...folders.folders[0], scope_id: 2 }],
        }),
      ),
    ).rejects.toThrow('Invalid search folders response');
    expect(() =>
      parseSearchFoldersResponse({
        ...folders,
        folders: [{ ...folders.folders[0], folder_node_id: 0 }],
      }),
    ).toThrow('Invalid search folders response');
  });

  it('uses the dedicated read-only folder listing command', async () => {
    const invokeCommand = vi.fn().mockResolvedValue({
      api_version: 'deskgraph.search-folders.v1',
      scope_id: 1,
      folder_count: 0,
      folders: [],
      truncated: false,
    });
    await listSearchFolders(1, invokeCommand);
    expect(invokeCommand).toHaveBeenCalledWith(LIST_SEARCH_FOLDERS_COMMAND, { scopeId: 1 });
  });
});
