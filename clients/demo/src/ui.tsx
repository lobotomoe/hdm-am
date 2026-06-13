import type { ChangeEvent, ReactNode } from 'react';
import { HdmBridgeError, HdmTransportError } from '@hdm-am/react';

export function Section({ title, children }: { title: string; children: ReactNode }): ReactNode {
  return (
    <section className="card">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

export function Field({
  label,
  value,
  onChange,
  type = 'text',
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: string;
  placeholder?: string;
}): ReactNode {
  return (
    <label className="field">
      <span>{label}</span>
      <input
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(e: ChangeEvent<HTMLInputElement>) => {
          onChange(e.target.value);
        }}
      />
    </label>
  );
}

export function Button({
  onClick,
  disabled,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  children: ReactNode;
}): ReactNode {
  return (
    <button type="button" className="btn" onClick={onClick} disabled={disabled ?? false}>
      {children}
    </button>
  );
}

export function JsonBlock({ value }: { value: unknown }): ReactNode {
  return <pre className="json">{JSON.stringify(value, null, 2)}</pre>;
}

/** Render any error from a hook (typed bridge errors, transport errors, or anything else). */
export function ErrorView({ error }: { error: unknown }): ReactNode {
  if (error === undefined || error === null) {
    return null;
  }
  if (error instanceof HdmBridgeError) {
    return (
      <div className="error">
        <strong>
          {error.kind}
          {error.code !== undefined ? ` (code ${String(error.code)})` : ''}
        </strong>
        <div>{error.message}</div>
        {(error.retryable || error.requiresRelogin || error.requiresReconnect) && (
          <div className="hints">
            {error.retryable ? 'retryable ' : ''}
            {error.requiresRelogin ? 'requires re-login ' : ''}
            {error.requiresReconnect ? 'requires reconnect' : ''}
          </div>
        )}
      </div>
    );
  }
  if (error instanceof HdmTransportError) {
    return <div className="error">Transport error: {error.message}</div>;
  }
  const message =
    error instanceof Error ? error.message : typeof error === 'string' ? error : 'Unknown error';
  return <div className="error">{message}</div>;
}

export function Result({
  loading,
  error,
  children,
}: {
  loading: boolean;
  error: unknown;
  children?: ReactNode;
}): ReactNode {
  return (
    <div className="result">
      {loading && <span className="muted">…</span>}
      <ErrorView error={error} />
      {children}
    </div>
  );
}
