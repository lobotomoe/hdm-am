import type { Connection, HdmClientOptions } from '@hdm-am/react';

/** Form-shaped configuration: all strings, parsed when building client options. */
export interface DemoConfig {
  baseUrl: string;
  token: string;
  host: string;
  port: string;
  password: string;
  cashier: string;
  pin: string;
  timeoutSecs: string;
}

export const EMPTY_CONFIG: DemoConfig = {
  baseUrl: 'http://127.0.0.1:8077',
  token: '',
  host: '',
  port: '',
  password: '',
  cashier: '',
  pin: '',
  timeoutSecs: '',
};

// Non-secret fields persist across reloads in localStorage; secrets only live in sessionStorage so
// they are dropped when the tab closes. Never persist secrets to disk in a real integration either.
const PERSISTENT_KEY = 'hdm-demo-config';
const SECRET_KEY = 'hdm-demo-secrets';
const SECRET_FIELDS = ['token', 'password', 'pin'] as const;
type SecretField = (typeof SECRET_FIELDS)[number];

function isStringRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function pickStrings<K extends string>(source: Record<string, unknown>, keys: readonly K[]): Partial<Record<K, string>> {
  const out: Partial<Record<K, string>> = {};
  for (const key of keys) {
    const value = source[key];
    if (typeof value === 'string') {
      out[key] = value;
    }
  }
  return out;
}

export function loadConfig(): DemoConfig {
  const config = { ...EMPTY_CONFIG };
  try {
    const persistent: unknown = JSON.parse(localStorage.getItem(PERSISTENT_KEY) ?? '{}');
    if (isStringRecord(persistent)) {
      Object.assign(config, pickStrings(persistent, ['baseUrl', 'host', 'port', 'cashier', 'timeoutSecs']));
    }
    const secrets: unknown = JSON.parse(sessionStorage.getItem(SECRET_KEY) ?? '{}');
    if (isStringRecord(secrets)) {
      Object.assign(config, pickStrings(secrets, SECRET_FIELDS));
    }
  } catch {
    // Corrupt storage falls back to defaults — nothing security-sensitive to recover.
  }
  return config;
}

export function saveConfig(config: DemoConfig): void {
  const { token, password, pin, ...persistent } = config;
  const secrets: Record<SecretField, string> = { token, password, pin };
  localStorage.setItem(PERSISTENT_KEY, JSON.stringify(persistent));
  sessionStorage.setItem(SECRET_KEY, JSON.stringify(secrets));
}

function parseOptionalInt(value: string): number | undefined {
  const trimmed = value.trim();
  if (trimmed === '') {
    return undefined;
  }
  const parsed = Number.parseInt(trimmed, 10);
  return Number.isFinite(parsed) ? parsed : undefined;
}

/** Build a {@link Connection} from the form, omitting blank fields. */
export function toConnection(config: DemoConfig): Connection {
  const connection: Connection = {};
  if (config.host.trim()) {
    connection.host = config.host.trim();
  }
  const port = parseOptionalInt(config.port);
  if (port !== undefined) {
    connection.port = port;
  }
  if (config.password) {
    connection.password = config.password;
  }
  const cashier = parseOptionalInt(config.cashier);
  if (cashier !== undefined) {
    connection.cashier = cashier;
  }
  if (config.pin) {
    connection.pin = config.pin;
  }
  const timeout = parseOptionalInt(config.timeoutSecs);
  if (timeout !== undefined) {
    connection.timeout_secs = timeout;
  }
  return connection;
}

/** Build memo-stable {@link HdmClientOptions} from the form. */
export function toClientOptions(config: DemoConfig): HdmClientOptions {
  const options: HdmClientOptions = { baseUrl: config.baseUrl.trim() };
  if (config.token) {
    options.token = config.token;
  }
  const connection = toConnection(config);
  if (Object.keys(connection).length > 0) {
    options.connection = connection;
  }
  return options;
}
