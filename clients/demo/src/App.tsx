import { useMemo, useState, type ReactNode } from 'react';
import { HdmProvider } from '@hdm-am/react';
import { EMPTY_CONFIG, loadConfig, saveConfig, toClientOptions, type DemoConfig } from './config.js';
import { DeviceSection } from './sections/DeviceSection.js';
import { DirectorySection } from './sections/DirectorySection.js';
import { RegisterSection } from './sections/RegisterSection.js';
import { Button, Field } from './ui.js';

export function App(): ReactNode {
  const [draft, setDraft] = useState<DemoConfig>(() => loadConfig());
  // Only an explicitly "connected" config drives the client, so editing fields doesn't fire calls.
  const [active, setActive] = useState<DemoConfig | null>(null);

  const options = useMemo(() => (active ? toClientOptions(active) : null), [active]);

  const set = (key: keyof DemoConfig) => (value: string) => {
    setDraft((prev) => ({ ...prev, [key]: value }));
  };

  return (
    <main>
      <header>
        <h1>HDM Bridge Demo</h1>
        <p className="muted">
          Drive an Armenian fiscal cash register from the browser, through the local{' '}
          <code>hdm-bridge</code>.
        </p>
      </header>

      <section className="card">
        <h2>Connection</h2>
        <div className="grid">
          <Field label="Bridge URL" value={draft.baseUrl} onChange={set('baseUrl')} placeholder="http://127.0.0.1:8077" />
          <Field label="Bridge token" value={draft.token} onChange={set('token')} type="password" />
          <Field label="Device host" value={draft.host} onChange={set('host')} placeholder="192.168.1.4" />
          <Field label="Port" value={draft.port} onChange={set('port')} placeholder="1025" />
          <Field label="Device password" value={draft.password} onChange={set('password')} type="password" />
          <Field label="Cashier" value={draft.cashier} onChange={set('cashier')} placeholder="3" />
          <Field label="PIN" value={draft.pin} onChange={set('pin')} type="password" />
          <Field label="Timeout (s)" value={draft.timeoutSecs} onChange={set('timeoutSecs')} placeholder="50" />
        </div>
        <div className="row">
          <Button
            onClick={() => {
              saveConfig(draft);
              setActive(draft);
            }}
          >
            Connect
          </Button>
          <Button
            onClick={() => {
              setActive(null);
              setDraft(EMPTY_CONFIG);
            }}
          >
            Reset
          </Button>
          {active ? <span className="ok">connected to {active.baseUrl}</span> : <span className="muted">not connected</span>}
        </div>
        <p className="note">
          Secrets (token, password, PIN) are kept only in this tab&apos;s session storage and are
          dropped when it closes. Start the bridge with an allow-origin for this page, e.g.
          <br />
          <code>hdm-bridge --insecure-no-auth --allow-origin http://localhost:5173</code>
        </p>
      </section>

      {options ? (
        <HdmProvider options={options}>
          <DeviceSection />
          <DirectorySection />
          <RegisterSection />
        </HdmProvider>
      ) : null}
    </main>
  );
}
