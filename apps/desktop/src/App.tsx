import { useEffect, useState } from 'react';

import { lifecycleLabel, loadHealthReport, type HealthReport } from './health';

type HealthViewState =
  { kind: 'loading' } | { kind: 'ready'; report: HealthReport } | { kind: 'error' };

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

function PrivacyItem({ enabled, children }: { enabled: boolean; children: string }) {
  return (
    <li>
      <span
        aria-hidden="true"
        className={enabled ? 'privacy-dot privacy-dot--on' : 'privacy-dot'}
      />
      {children}
    </li>
  );
}

export default function App() {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<HealthViewState>({ kind: 'loading' });

  useEffect(() => {
    let active = true;
    setState({ kind: 'loading' });

    void loadHealthReport()
      .then((report) => {
        if (active) {
          setState({ kind: 'ready', report });
        }
      })
      .catch(() => {
        if (active) {
          setState({ kind: 'error' });
        }
      });

    return () => {
      active = false;
    };
  }, [attempt]);

  return (
    <main className="app-shell">
      <header className="hero">
        <div>
          <p className="eyebrow">DeskGraph · M0 Foundation</p>
          <h1>Graphify your computer.</h1>
          <p className="hero-copy">
            A local-first context graph for files you explicitly authorize. This pre-release build
            currently proves only the privacy-safe application health path.
          </p>
          <p className="hero-copy hero-copy--zh">
            本機優先、明確授權、可解釋且可復原。目前僅驗證安全的應用程式健康狀態流程。
          </p>
        </div>
        <span className="release-badge">PRE-RELEASE</span>
      </header>

      {state.kind === 'loading' ? (
        <section className="state-card" aria-live="polite" aria-busy="true">
          <span className="loader" aria-hidden="true" />
          <div>
            <h2>Checking the local runtime</h2>
            <p>No folders are scanned during this check.</p>
          </div>
        </section>
      ) : null}

      {state.kind === 'error' ? (
        <section className="state-card state-card--error" role="alert">
          <div>
            <h2>Health check unavailable</h2>
            <p>
              The local backend did not return a validated health response. No raw error details are
              shown.
            </p>
          </div>
          <button type="button" onClick={() => setAttempt((value) => value + 1)}>
            Retry health check
          </button>
        </section>
      ) : null}

      {state.kind === 'ready' ? (
        <div className="dashboard" aria-live="polite">
          <section className="panel" aria-labelledby="runtime-title">
            <div className="panel-heading">
              <div>
                <p className="panel-kicker">Local runtime</p>
                <h2 id="runtime-title">Foundation is connected</h2>
              </div>
              <span className="connected-indicator">Connected</span>
            </div>

            <div className="status-list">
              <StatusRow
                label="Platform"
                value={`${state.report.platform.os} · ${state.report.platform.architecture}`}
                tone="ok"
              />
              <StatusRow
                label="Manifest database"
                value={lifecycleLabel(state.report.database.state)}
              />
              <StatusRow
                label="OCR provider"
                value={lifecycleLabel(state.report.providers.ocr.state)}
              />
              <StatusRow
                label="Embedding provider"
                value={lifecycleLabel(state.report.providers.embeddings.state)}
              />
              <StatusRow
                label="Local LLM"
                value={lifecycleLabel(state.report.providers.local_llm.state)}
              />
            </div>
          </section>

          <section className="panel panel--privacy" aria-labelledby="privacy-title">
            <p className="panel-kicker">Privacy boundary</p>
            <h2 id="privacy-title">Nothing indexed yet</h2>
            <p>
              M1 will add explicit folder authorization. Until then, the backend has zero scopes and
              reports no filesystem locations.
            </p>
            <ul className="privacy-list">
              <PrivacyItem enabled={state.report.privacy.local_only_default}>
                Local-only default
              </PrivacyItem>
              <PrivacyItem enabled={!state.report.privacy.network_required}>
                No network required
              </PrivacyItem>
              <PrivacyItem enabled={!state.report.privacy.filesystem_locations_included}>
                No locations in health payload
              </PrivacyItem>
              <PrivacyItem enabled={state.report.privacy.authorized_scope_count === 0}>
                Zero authorized scopes
              </PrivacyItem>
            </ul>
          </section>
        </div>
      ) : null}

      <footer>
        <span>DeskGraph {state.kind === 'ready' ? state.report.app_version : '0.1.0'}</span>
        <span>Scan · Search · Organize · MCP are not enabled in M0</span>
      </footer>
    </main>
  );
}
