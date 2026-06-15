import type { ChangeEvent, ReactNode } from 'react';
import { AlertCircle, CheckCircle2, Loader2 } from 'lucide-react';
import { HdmBridgeError, HdmTransportError } from '@hdm-am/react';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

/** Labelled text/number input. */
export function Field({
  id,
  label,
  value,
  onChange,
  type = 'text',
  placeholder,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: string;
  placeholder?: string;
}): ReactNode {
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        data-testid={`field-${id}`}
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(e: ChangeEvent<HTMLInputElement>) => {
          onChange(e.target.value);
        }}
      />
    </div>
  );
}

/** A small inline spinner shown while a call is in flight. */
export function Spinner({ show }: { show: boolean }): ReactNode {
  if (!show) {
    return null;
  }
  return <Loader2 className="size-4 animate-spin text-muted-foreground" aria-label="loading" />;
}

/** A green "success" pill. */
export function OkBadge({ children }: { children: ReactNode }): ReactNode {
  return (
    <Badge className="border-transparent bg-emerald-600 text-white hover:bg-emerald-600">
      <CheckCircle2 className="size-3.5" />
      {children}
    </Badge>
  );
}

/** Pretty-printed JSON for raw responses. */
export function JsonBlock({ value }: { value: unknown }): ReactNode {
  return (
    <pre className="mt-2 max-h-64 overflow-auto rounded-md bg-muted p-3 text-xs text-muted-foreground">
      {JSON.stringify(value, null, 2)}
    </pre>
  );
}

/** Render any error from a hook as a destructive alert with recovery hints. */
export function ErrorView({ error }: { error: unknown }): ReactNode {
  if (error === undefined || error === null) {
    return null;
  }

  let title = 'Error';
  let detail = '';
  const hints: string[] = [];

  if (error instanceof HdmBridgeError) {
    title = error.code !== undefined ? `${error.kind} · code ${String(error.code)}` : error.kind;
    detail = error.message;
    if (error.retryable) {
      hints.push('retryable');
    }
    if (error.requiresRelogin) {
      hints.push('re-login required');
    }
    if (error.requiresReconnect) {
      hints.push('reconnect required');
    }
  } else if (error instanceof HdmTransportError) {
    title = 'Connection failed';
    detail = error.message;
  } else {
    detail = error instanceof Error ? error.message : 'Unknown error';
  }

  return (
    <Alert variant="destructive" className="mt-2" data-testid="error">
      <AlertCircle className="size-4" />
      <AlertTitle>{title}</AlertTitle>
      <AlertDescription>
        <span>{detail}</span>
        {hints.length > 0 ? <span className="italic opacity-80">{hints.join(' · ')}</span> : null}
      </AlertDescription>
    </Alert>
  );
}

/** A consistent "action row footer": optional spinner, error, then any success content. */
export function Outcome({
  loading,
  error,
  testId,
  children,
}: {
  loading: boolean;
  error: unknown;
  testId?: string;
  children?: ReactNode;
}): ReactNode {
  return (
    <div className="space-y-2" data-testid={testId}>
      <div className="flex items-center gap-2">
        <Spinner show={loading} />
        {children}
      </div>
      <ErrorView error={error} />
    </div>
  );
}
