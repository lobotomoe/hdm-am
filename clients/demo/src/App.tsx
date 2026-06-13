import { useMemo, useState, type ReactNode } from 'react';
import { Plug, RotateCcw } from 'lucide-react';
import { HdmProvider } from '@hdm-am/react';
import { EMPTY_CONFIG, loadConfig, saveConfig, toClientOptions, type DemoConfig } from './config.js';
import { DeviceSection } from './sections/DeviceSection.js';
import { DirectorySection } from './sections/DirectorySection.js';
import { RegisterSection } from './sections/RegisterSection.js';
import { ThemeToggle } from './components/theme-toggle.js';
import { Field } from './ui.js';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';

export function App(): ReactNode {
  const [draft, setDraft] = useState<DemoConfig>(() => loadConfig());
  // Only an explicitly "connected" config drives the client, so editing fields doesn't fire calls.
  const [active, setActive] = useState<DemoConfig | null>(null);

  const options = useMemo(() => (active ? toClientOptions(active) : null), [active]);

  const set = (key: keyof DemoConfig) => (value: string) => {
    setDraft((prev) => ({ ...prev, [key]: value }));
  };

  return (
    <div className="min-h-screen bg-background">
      <div className="mx-auto max-w-3xl px-4 py-8">
        <header className="mb-6 flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">HDM Bridge</h1>
            <p className="mt-1 text-sm text-muted-foreground">
              Drive an Armenian fiscal cash register from the browser, through the local{' '}
              <code className="rounded bg-muted px-1 py-0.5 text-xs">hdm-bridge</code>.
            </p>
          </div>
          <ThemeToggle />
        </header>

        <Card>
          <CardHeader>
            <CardTitle>Connection</CardTitle>
            <CardDescription>Point at your running bridge and the device behind it.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <Field id="baseUrl" label="Bridge URL" value={draft.baseUrl} onChange={set('baseUrl')} placeholder="http://127.0.0.1:8077" />
              <Field id="token" label="Bridge token" value={draft.token} onChange={set('token')} type="password" placeholder="optional (insecure bridge)" />
              <Field id="host" label="Device host" value={draft.host} onChange={set('host')} placeholder="192.168.1.4" />
              <Field id="port" label="Port" value={draft.port} onChange={set('port')} placeholder="1025" />
              <Field id="password" label="Device password" value={draft.password} onChange={set('password')} type="password" />
              <Field id="cashier" label="Cashier" value={draft.cashier} onChange={set('cashier')} placeholder="3" />
              <Field id="pin" label="PIN" value={draft.pin} onChange={set('pin')} type="password" />
              <Field id="timeout" label="Timeout (s)" value={draft.timeoutSecs} onChange={set('timeoutSecs')} placeholder="50" />
            </div>
            <p className="text-xs text-muted-foreground">
              Secrets (token, password, PIN) are kept only in this tab&apos;s session storage and are
              dropped when it closes. Start the bridge with an allow-origin for this page:
            </p>
            <pre className="overflow-auto rounded-md bg-muted p-3 text-xs">
              hdm-bridge --insecure-no-auth --allow-origin http://localhost:5173
            </pre>
          </CardContent>
          <CardFooter className="flex flex-wrap items-center gap-3">
            <Button
              data-testid="connect"
              onClick={() => {
                saveConfig(draft);
                setActive(draft);
              }}
            >
              <Plug className="size-4" />
              Connect
            </Button>
            <Button
              data-testid="reset"
              variant="outline"
              onClick={() => {
                setActive(null);
                setDraft(EMPTY_CONFIG);
              }}
            >
              <RotateCcw className="size-4" />
              Reset
            </Button>
            {active ? (
              <Badge
                data-testid="conn-status"
                className="border-transparent bg-emerald-600 text-white hover:bg-emerald-600"
              >
                Connected · {active.baseUrl}
              </Badge>
            ) : (
              <Badge data-testid="conn-status" variant="secondary">
                Not connected
              </Badge>
            )}
          </CardFooter>
        </Card>

        {options ? (
          <HdmProvider options={options}>
            <div className="mt-6 space-y-6">
              <DeviceSection />
              <DirectorySection />
              <RegisterSection />
            </div>
          </HdmProvider>
        ) : null}
      </div>
    </div>
  );
}
