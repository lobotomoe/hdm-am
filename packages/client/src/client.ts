import type { paths } from './generated/openapi.js';
import {
  HdmBridgeError,
  HdmTransportError,
  HdmValidationError,
  isErrorBody,
} from './errors.js';
import type {
  CashInOutRequest,
  Connection,
  DateTimeResponse,
  EmptyResponse,
  ErrorBody,
  FiscalReportRequest,
  GetReturnableReceiptRequest,
  HdmIdentity,
  HdmInfo,
  HealthStatus,
  ListOpsAndDepsResponse,
  PaymentSystemsListResponse,
  PrintReceiptRequest,
  PrintReturnReceiptRequest,
  ReceiptResponse,
  ReturnableReceiptResponse,
  ReturnReceiptResponse,
  SetupHeaderFooterRequest,
  SetupHeaderLogoRequest,
  SingleEmarkRequest,
  StatusOk,
} from './types.js';

/** Any path the bridge actually exposes — typos are caught at compile time. */
type ApiPath = Extract<keyof paths, string>;

/** Public alias for bridge paths that can be used as validation-map keys. */
export type HdmApiPath = ApiPath;

/** A schema validator for an already-parsed JSON value. Throw to reject the value. */
export type HdmTransportValidator = (value: unknown) => void;

/** Validators keyed by bridge route path. Paths without a validator are left unchecked. */
export type HdmTransportValidatorMap = Partial<Record<HdmApiPath, HdmTransportValidator>>;

/** Optional runtime validation hooks for request envelopes and successful response payloads. */
export interface HdmValidationOptions {
  /** Validate request envelopes before serializing them. Defaults to `true` when validators exist. */
  requests?: boolean;
  /** Validate successful response payloads after parsing JSON. Defaults to `true` when validators exist. */
  responses?: boolean;
  /** Request-envelope validators keyed by route path. */
  requestValidators?: HdmTransportValidatorMap;
  /** Successful-response validators keyed by route path. */
  responseValidators?: HdmTransportValidatorMap;
}

/** Configuration for an {@link HdmBridgeClient}. */
export interface HdmClientOptions {
  /** Base URL of the bridge, e.g. `http://127.0.0.1:8077`. A trailing slash is ignored. */
  baseUrl: string;
  /** Bearer token (the bridge's `HDM_BRIDGE_TOKEN`). Omit only against an `--insecure-no-auth` bridge. */
  token?: string;
  /** Default device connection, merged under any per-request override. */
  connection?: Connection;
  /** `fetch` implementation. Defaults to the global `fetch` (browser / Node 18+). */
  fetch?: typeof fetch;
  /** Default per-request timeout in milliseconds. Omit to rely on the bridge's own 50s device cap. */
  timeoutMs?: number;
  /**
   * Optional runtime transport validation. The base package is validator-agnostic; use
   * `@hdm-am/client/zod` for generated Zod validators.
   */
  validation?: HdmValidationOptions;
}

/** Per-call options applied to a single operation. */
export interface RequestOptions {
  /** Connection override for this call, merged over the client default field-by-field. */
  connection?: Connection;
  /** External abort signal; aborting it rejects the call with an {@link HdmTransportError}. */
  signal?: AbortSignal;
  /** Timeout for this call in milliseconds; overrides the client default. */
  timeoutMs?: number;
}

interface Envelope {
  connection?: Connection;
  params?: unknown;
}

interface SignalScope {
  signal?: AbortSignal;
  cleanup: () => void;
}

function resolveFetch(provided: HdmClientOptions['fetch']): typeof fetch {
  if (provided) {
    return provided;
  }
  if (typeof globalThis.fetch !== 'function') {
    throw new TypeError(
      'no global fetch available; pass `fetch` in HdmClientOptions (Node < 18 or a non-fetch runtime)',
    );
  }
  return globalThis.fetch.bind(globalThis);
}

function normalizeBaseUrl(baseUrl: string): string {
  const trimmed = baseUrl.trim();
  if (trimmed.length === 0) {
    throw new TypeError('HdmBridgeClient requires a non-empty baseUrl');
  }
  return trimmed.replace(/\/+$/, '');
}

function normalizeTimeoutMs(timeoutMs: number | undefined): number | undefined {
  if (timeoutMs === undefined) {
    return undefined;
  }
  if (!Number.isFinite(timeoutMs) || timeoutMs < 0) {
    throw new RangeError('timeoutMs must be a finite non-negative number');
  }
  return timeoutMs;
}

function timeoutReason(path: string): Error | DOMException {
  const message = `request to ${path} timed out`;
  if (typeof DOMException === 'function') {
    return new DOMException(message, 'TimeoutError');
  }
  const error = new Error(message);
  error.name = 'TimeoutError';
  return error;
}

function createSignalScope(path: ApiPath, timeoutMs: number | undefined, signal?: AbortSignal): SignalScope {
  const signals: AbortSignal[] = [];
  let timer: ReturnType<typeof setTimeout> | undefined;
  let timeoutController: AbortController | undefined;
  if (timeoutMs !== undefined) {
    timeoutController = new AbortController();
    timer = setTimeout(() => {
      timeoutController?.abort(timeoutReason(path));
    }, timeoutMs);
    signals.push(timeoutController.signal);
  }
  if (signal) {
    signals.push(signal);
  }

  if (signals.length === 0) {
    return {
      cleanup: () => {
        if (timer !== undefined) {
          clearTimeout(timer);
        }
      },
    };
  }
  if (signals.length === 1) {
    const onlySignal = signals[0];
    if (!onlySignal) {
      throw new Error('internal error: missing abort signal');
    }
    return {
      signal: onlySignal,
      cleanup: () => {
        if (timer !== undefined) {
          clearTimeout(timer);
        }
      },
    };
  }

  const controller = new AbortController();
  const removers: (() => void)[] = [];
  const abortFrom = (source: AbortSignal): void => {
    if (!controller.signal.aborted) {
      controller.abort(source.reason);
    }
  };

  for (const source of signals) {
    if (source.aborted) {
      abortFrom(source);
      break;
    }
    const onAbort = (): void => {
      abortFrom(source);
    };
    source.addEventListener('abort', onAbort, { once: true });
    removers.push(() => {
      source.removeEventListener('abort', onAbort);
    });
  }

  return {
    signal: controller.signal,
    cleanup: () => {
      if (timer !== undefined) {
        clearTimeout(timer);
      }
      for (const remove of removers) {
        remove();
      }
    },
  };
}

function jsonParseError(path: ApiPath, cause: unknown): HdmTransportError {
  return new HdmTransportError(`request to ${path} returned invalid JSON`, { cause });
}

function parseJson(text: string, path: ApiPath): unknown {
  try {
    return JSON.parse(text);
  } catch (cause) {
    throw jsonParseError(path, cause);
  }
}

function stringifyUnknown(value: unknown): string {
  if (typeof value === 'string') {
    return value;
  }
  if (value instanceof Error) {
    return value.message;
  }
  if (value === undefined || typeof value === 'function' || typeof value === 'symbol') {
    return String(value);
  }
  try {
    return JSON.stringify(value);
  } catch (cause) {
    return cause instanceof Error ? `unserializable value: ${cause.message}` : 'unserializable value';
  }
}

/**
 * Isomorphic client for the HDM bridge — one method per protocol operation. Runs in the browser and
 * in Node 18+ (or any runtime with `fetch`, or with a `fetch` injected via {@link HdmClientOptions}).
 *
 * Every method merges this client's default {@link Connection} with a per-call override, posts the
 * `{ connection?, params? }` envelope the bridge expects, and returns the typed response. A non-2xx
 * response becomes an {@link HdmBridgeError}; a network/abort/timeout failure becomes an
 * {@link HdmTransportError}.
 */
export class HdmBridgeClient {
  private readonly baseUrl: string;
  private readonly token: string | undefined;
  private readonly connection: Connection | undefined;
  private readonly fetchImpl: typeof fetch;
  private readonly timeoutMs: number | undefined;
  private readonly validation: HdmValidationOptions | undefined;

  constructor(options: HdmClientOptions) {
    this.baseUrl = normalizeBaseUrl(options.baseUrl);
    this.token = options.token;
    this.connection = options.connection;
    this.fetchImpl = resolveFetch(options.fetch);
    this.timeoutMs = normalizeTimeoutMs(options.timeoutMs);
    this.validation = options.validation;
  }

  // ---- Meta (public; no auth required) ----

  /** Liveness probe. */
  health(opts: RequestOptions = {}): Promise<HealthStatus> {
    return this.request('GET', '/v1/health', undefined, opts);
  }

  /** Bridge metadata and the operation list. */
  info(opts: RequestOptions = {}): Promise<HdmInfo> {
    return this.request('GET', '/v1/info', undefined, opts);
  }

  /** The bridge's own OpenAPI 3.1 document. */
  openapiDocument(opts: RequestOptions = {}): Promise<unknown> {
    return this.request('GET', '/v1/openapi.json', undefined, opts);
  }

  // ---- Operations ----

  /** Probe an endpoint and confirm it speaks the HDM protocol. */
  probe(opts: RequestOptions = {}): Promise<HdmIdentity> {
    return this.request('POST', '/v1/probe', this.envelope(undefined, opts), opts);
  }

  /** List the device's operators and departments. */
  operators(opts: RequestOptions = {}): Promise<ListOpsAndDepsResponse> {
    return this.request('POST', '/v1/operators', this.envelope(undefined, opts), opts);
  }

  /** Verify operator login credentials. */
  login(opts: RequestOptions = {}): Promise<StatusOk> {
    return this.request('POST', '/v1/login', this.envelope(undefined, opts), opts);
  }

  /** Print a fiscal receipt. */
  printReceipt(params: PrintReceiptRequest, opts: RequestOptions = {}): Promise<ReceiptResponse> {
    return this.request('POST', '/v1/receipt', this.envelope(params, opts), opts);
  }

  /** Print a copy of the last receipt. */
  printLastReceipt(opts: RequestOptions = {}): Promise<EmptyResponse> {
    return this.request('POST', '/v1/receipt/last', this.envelope(undefined, opts), opts);
  }

  /** Look up a returnable receipt's contents (read-only). */
  lookupReceipt(
    params: GetReturnableReceiptRequest,
    opts: RequestOptions = {},
  ): Promise<ReturnableReceiptResponse> {
    return this.request('POST', '/v1/receipt/lookup', this.envelope(params, opts), opts);
  }

  /** Print a return receipt. */
  printReturn(
    params: PrintReturnReceiptRequest,
    opts: RequestOptions = {},
  ): Promise<ReturnReceiptResponse> {
    return this.request('POST', '/v1/return', this.envelope(params, opts), opts);
  }

  /** Print an X or Z fiscal report. */
  report(params: FiscalReportRequest, opts: RequestOptions = {}): Promise<EmptyResponse> {
    return this.request('POST', '/v1/report', this.envelope(params, opts), opts);
  }

  /** Register a cash-drawer in or out. */
  cashInOut(params: CashInOutRequest, opts: RequestOptions = {}): Promise<EmptyResponse> {
    return this.request('POST', '/v1/cash', this.envelope(params, opts), opts);
  }

  /** Get the device date and time. */
  dateTime(opts: RequestOptions = {}): Promise<DateTimeResponse> {
    return this.request('POST', '/v1/datetime', this.envelope(undefined, opts), opts);
  }

  /** Synchronize the device clock. */
  timeSync(opts: RequestOptions = {}): Promise<EmptyResponse> {
    return this.request('POST', '/v1/time-sync', this.envelope(undefined, opts), opts);
  }

  /** List the payment systems configured on the device. */
  paymentSystems(opts: RequestOptions = {}): Promise<PaymentSystemsListResponse> {
    return this.request('POST', '/v1/payment-systems', this.envelope(undefined, opts), opts);
  }

  /** Validate a single eMark code. */
  emark(params: SingleEmarkRequest, opts: RequestOptions = {}): Promise<EmptyResponse> {
    return this.request('POST', '/v1/emark', this.envelope(params, opts), opts);
  }

  /** Print a sample receipt. */
  receiptSample(opts: RequestOptions = {}): Promise<EmptyResponse> {
    return this.request('POST', '/v1/sample', this.envelope(undefined, opts), opts);
  }

  /** Configure receipt header and footer lines. */
  headerFooter(params: SetupHeaderFooterRequest, opts: RequestOptions = {}): Promise<EmptyResponse> {
    return this.request('POST', '/v1/header-footer', this.envelope(params, opts), opts);
  }

  /** Configure the receipt header logo. */
  headerLogo(params: SetupHeaderLogoRequest, opts: RequestOptions = {}): Promise<EmptyResponse> {
    return this.request('POST', '/v1/logo', this.envelope(params, opts), opts);
  }

  // ---- Internals ----

  private envelope(params: unknown, opts: RequestOptions): Envelope {
    const connection = this.mergeConnection(opts.connection);
    const body: Envelope = {};
    if (connection) {
      body.connection = connection;
    }
    if (params !== undefined) {
      body.params = params;
    }
    return body;
  }

  private mergeConnection(override: Connection | undefined): Connection | undefined {
    if (!this.connection && !override) {
      return undefined;
    }
    return { ...this.connection, ...override };
  }

  private async request<T>(
    method: 'GET' | 'POST',
    path: ApiPath,
    body: Envelope | undefined,
    opts: RequestOptions,
  ): Promise<T> {
    const headers: Record<string, string> = { accept: 'application/json' };
    if (this.token) {
      headers.authorization = `Bearer ${this.token}`;
    }
    let payload: string | undefined;
    if (body !== undefined) {
      this.validate('request', path, body);
      headers['content-type'] = 'application/json';
      payload = JSON.stringify(body);
    }

    const timeoutMs = normalizeTimeoutMs(opts.timeoutMs ?? this.timeoutMs);
    const signalScope = createSignalScope(path, timeoutMs, opts.signal);

    // Build the init incrementally so optional fields are omitted, not set to `undefined`
    // (required under exactOptionalPropertyTypes).
    const init: RequestInit = { method, headers };
    if (payload !== undefined) {
      init.body = payload;
    }
    if (signalScope.signal !== undefined) {
      init.signal = signalScope.signal;
    }

    let response: Response;
    try {
      response = await this.fetchImpl(`${this.baseUrl}${path}`, init);
    } catch (cause) {
      throw new HdmTransportError(`request to ${path} failed`, { cause });
    } finally {
      signalScope.cleanup();
    }

    if (!response.ok) {
      throw new HdmBridgeError(response.status, await this.parseError(response));
    }

    let text: string;
    try {
      text = await response.text();
    } catch (cause) {
      throw new HdmTransportError(`request to ${path} failed while reading the response`, { cause });
    }
    const data = text.trim().length === 0 ? {} : parseJson(text, path);
    this.validate('response', path, data, response.status);
    return data as T;
  }

  private validate(
    direction: 'request' | 'response',
    path: ApiPath,
    value: unknown,
    status?: number,
  ): void {
    const enabled =
      direction === 'request'
        ? (this.validation?.requests ?? true)
        : (this.validation?.responses ?? true);
    if (!enabled) {
      return;
    }
    const validators =
      direction === 'request'
        ? this.validation?.requestValidators
        : this.validation?.responseValidators;
    const validator = validators?.[path];
    if (!validator) {
      return;
    }
    try {
      validator(value);
    } catch (cause) {
      throw new HdmValidationError(
        direction,
        path,
        status === undefined ? { cause } : { status, cause },
      );
    }
  }

  private async parseError(response: Response): Promise<ErrorBody> {
    let text = '';
    try {
      text = await response.text();
    } catch {
      return synthesizeError(response.statusText || 'request failed');
    }
    if (text.trim().length === 0) {
      return synthesizeError(response.statusText || 'request failed');
    }
    let data: unknown;
    try {
      data = JSON.parse(text);
    } catch {
      return synthesizeError(text);
    }
    if (isErrorBody(data)) {
      return data;
    }
    return synthesizeError(stringifyUnknown(data));
  }
}

/** Build a minimal error envelope for a non-conforming error response (the bridge always conforms). */
function synthesizeError(message: string): ErrorBody {
  return {
    error: {
      kind: 'bad_request',
      message,
      retryable: false,
      requires_relogin: false,
      requires_reconnect: false,
    },
  };
}
