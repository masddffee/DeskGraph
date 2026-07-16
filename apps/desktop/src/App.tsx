import { useEffect, useState } from 'react';

import { lifecycleLabel, loadHealthReport, type HealthReport } from './health';
import {
  addAuthorizedScope,
  loadAuthorizedScopes,
  loadManifestStatus,
  runManifestScan,
  type AuthorizedScope,
  type ManifestStats,
} from './manifest';

type ReadyState = {
  kind: 'ready';
  report: HealthReport;
  manifest: ManifestStats;
  scopes: AuthorizedScope[];
};
type ViewState = { kind: 'loading' } | ReadyState | { kind: 'error' };
type ActionState =
  | { kind: 'idle' }
  | { kind: 'working'; label: string }
  | { kind: 'success'; message: string }
  | { kind: 'error'; message: string };

interface StatusRowProps {
  label: string;
  value: string;
  tone?: 'ok' | 'quiet';
}

function StatusRow({ label, value, tone = 'quiet' }: StatusRowProps) {
  return (
    <div className="status-row">
      <span>{label}</span>
      <span className={`status-pill status-pill--${tone}`}>{value}</span>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div className="metric">
      <strong>{value.toLocaleString()}</strong>
      <span>{label}</span>
    </div>
  );
}

async function loadDashboard(): Promise<ReadyState> {
  const [report, manifest, scopes] = await Promise.all([
    loadHealthReport(),
    loadManifestStatus(),
    loadAuthorizedScopes(),
  ]);
  return { kind: 'ready', report, manifest, scopes };
}

export default function App() {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<ViewState>({ kind: 'loading' });
  const [scopePath, setScopePath] = useState('');
  const [action, setAction] = useState<ActionState>({ kind: 'idle' });

  useEffect(() => {
    let active = true;
    setState({ kind: 'loading' });

    void loadDashboard()
      .then((dashboard) => {
        if (active) setState(dashboard);
      })
      .catch(() => {
        if (active) setState({ kind: 'error' });
      });

    return () => {
      active = false;
    };
  }, [attempt]);

  async function refreshManifest() {
    const [manifest, scopes] = await Promise.all([loadManifestStatus(), loadAuthorizedScopes()]);
    setState((current) => (current.kind === 'ready' ? { ...current, manifest, scopes } : current));
  }

  async function authorizeRequestedScope() {
    const requestedPath = scopePath.trim();
    if (!requestedPath) {
      setAction({ kind: 'error', message: 'Enter an existing folder path first.' });
      return;
    }
    setAction({ kind: 'working', label: 'Validating the folder boundary…' });
    try {
      await addAuthorizedScope(requestedPath);
      await refreshManifest();
      setScopePath('');
      setAction({
        kind: 'success',
        message: 'Folder authorized. Nothing was scanned until you choose Scan metadata.',
      });
    } catch {
      setAction({
        kind: 'error',
        message: 'The folder could not be authorized. Check that it exists and is not protected.',
      });
    }
  }

  async function scan(scope: AuthorizedScope) {
    setAction({ kind: 'working', label: 'Reading metadata inside the authorized folder…' });
    try {
      const report = await runManifestScan(scope.id);
      await refreshManifest();
      setAction({
        kind: 'success',
        message: `Scan complete: ${report.discovered_files} files and ${report.discovered_folders} folders.`,
      });
    } catch {
      setAction({
        kind: 'error',
        message:
          'The metadata scan stopped safely. Existing manifest data was not partially replaced.',
      });
    }
  }

  return (
    <main className="app-shell">
      <header className="hero">
        <div>
          <p className="eyebrow">DeskGraph · M1 Manifest Graph</p>
          <h1>Graphify your computer.</h1>
          <p className="hero-copy">
            Authorize one local folder at a time, then build a metadata-only manifest without
            uploading paths or file contents.
          </p>
          <p className="hero-copy hero-copy--zh">
            一次明確授權一個本機資料夾；只建立 metadata manifest，不上傳路徑或檔案內容。
          </p>
        </div>
        <span className="release-badge">PRE-RELEASE</span>
      </header>

      {state.kind === 'loading' ? (
        <section className="state-card" aria-live="polite" aria-busy="true">
          <span className="loader" aria-hidden="true" />
          <div>
            <h2>Opening the local manifest</h2>
            <p>No authorized folder is scanned automatically.</p>
          </div>
        </section>
      ) : null}

      {state.kind === 'error' ? (
        <section className="state-card state-card--error" role="alert">
          <div>
            <h2>Local manifest unavailable</h2>
            <p>The backend returned no validated status. Raw local errors and paths are hidden.</p>
          </div>
          <button type="button" onClick={() => setAttempt((value) => value + 1)}>
            Retry
          </button>
        </section>
      ) : null}

      {state.kind === 'ready' ? (
        <div className="dashboard" aria-live="polite">
          <section className="panel" aria-labelledby="runtime-title">
            <div className="panel-heading">
              <div>
                <p className="panel-kicker">Local runtime</p>
                <h2 id="runtime-title">Manifest is ready</h2>
              </div>
              <span className="connected-indicator">Local only</span>
            </div>
            <div className="status-list">
              <StatusRow
                label="Platform"
                value={`${state.report.platform.os} · ${state.report.platform.architecture}`}
                tone="ok"
              />
              <StatusRow label="SQLite manifest" value="Ready" tone="ok" />
              <StatusRow
                label="Optional local LLM"
                value={lifecycleLabel(state.report.providers.local_llm.state)}
              />
              <StatusRow label="Network required" value="No" tone="ok" />
            </div>
          </section>

          <section className="panel panel--privacy" aria-labelledby="manifest-title">
            <p className="panel-kicker">Current graph</p>
            <h2 id="manifest-title">
              {state.manifest.completed_scan_count === 0
                ? 'Nothing indexed yet'
                : 'Metadata indexed'}
            </h2>
            <div className="metrics">
              <Metric label="Files" value={state.manifest.file_count} />
              <Metric label="Folders" value={state.manifest.folder_count} />
              <Metric label="Locations" value={state.manifest.active_location_count} />
              <Metric label="Scan issues" value={state.manifest.issue_count} />
            </div>
          </section>

          <section className="panel panel--scopes" aria-labelledby="scopes-title">
            <div className="panel-heading panel-heading--wrap">
              <div>
                <p className="panel-kicker">Explicit authorization</p>
                <h2 id="scopes-title">Folders DeskGraph may inspect</h2>
                <p>
                  Enter an existing folder path. Authorization and scanning are separate actions;
                  symlinks and hidden entries are not followed.
                </p>
              </div>
              <span className="scope-count">{state.scopes.length} authorized</span>
            </div>

            <div className="scope-form">
              <label htmlFor="scope-path">Folder path</label>
              <div className="scope-form-row">
                <input
                  id="scope-path"
                  type="text"
                  value={scopePath}
                  onChange={(event) => setScopePath(event.target.value)}
                  placeholder="/Users/you/Documents or C:\Users\you\Documents"
                  autoComplete="off"
                  spellCheck="false"
                />
                <button
                  type="button"
                  disabled={action.kind === 'working'}
                  onClick={() => void authorizeRequestedScope()}
                >
                  Authorize folder
                </button>
              </div>
            </div>

            {action.kind !== 'idle' ? (
              <p className={`action-message action-message--${action.kind}`} role="status">
                {action.kind === 'working' ? action.label : action.message}
              </p>
            ) : null}

            {state.scopes.length === 0 ? (
              <div className="empty-scope">
                <strong>No folder access</strong>
                <span>
                  DeskGraph cannot inspect Desktop, Downloads, or Documents until added here.
                </span>
              </div>
            ) : (
              <ul className="scope-list">
                {state.scopes.map((scope) => (
                  <li key={scope.id}>
                    <div>
                      <span className="scope-label">Authorized scope {scope.id}</span>
                      <code>{scope.display_path}</code>
                    </div>
                    <button
                      type="button"
                      disabled={action.kind === 'working'}
                      onClick={() => void scan(scope)}
                    >
                      Scan metadata
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      ) : null}

      <footer>
        <span>DeskGraph {state.kind === 'ready' ? state.report.app_version : '0.1.0'}</span>
        <span>Metadata only · No content extraction · No file operations</span>
      </footer>
    </main>
  );
}
