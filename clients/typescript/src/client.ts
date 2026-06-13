import type { paths } from './generated/openapi.js';
import { HdmBridgeError, HdmTransportError, isErrorBody } from './errors.js';
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

  constructor(options: HdmClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/+$/, '');
    this.token = options.token;
    this.connection = options.connection;
    this.fetchImpl = resolveFetch(options.fetch);
    this.timeoutMs = options.timeoutMs;
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
    const headers: Record<string, string> = {};
    if (this.token) {
      headers.authorization = `Bearer ${this.token}`;
    }
    let payload: string | undefined;
    if (body !== undefined) {
      headers['content-type'] = 'application/json';
      payload = JSON.stringify(body);
    }

    const timeoutMs = opts.timeoutMs ?? this.timeoutMs;
    let signal: AbortSignal | undefined = opts.signal;
    let timer: ReturnType<typeof setTimeout> | undefined;
    if (timeoutMs !== undefined) {
      const controller = new AbortController();
      timer = setTimeout(() => {
        controller.abort(new DOMException(`request to ${path} timed out`, 'TimeoutError'));
      }, timeoutMs);
      signal = opts.signal ? AbortSignal.any([controller.signal, opts.signal]) : controller.signal;
    }

    // Build the init incrementally so optional fields are omitted, not set to `undefined`
    // (required under exactOptionalPropertyTypes).
    const init: RequestInit = { method, headers };
    if (payload !== undefined) {
      init.body = payload;
    }
    if (signal !== undefined) {
      init.signal = signal;
    }

    let response: Response;
    try {
      response = await this.fetchImpl(`${this.baseUrl}${path}`, init);
    } catch (cause) {
      throw new HdmTransportError(`request to ${path} failed`, { cause });
    } finally {
      if (timer !== undefined) {
        clearTimeout(timer);
      }
    }

    if (!response.ok) {
      throw new HdmBridgeError(response.status, await this.parseError(response));
    }

    const text = await response.text();
    return (text.length > 0 ? JSON.parse(text) : {}) as T;
  }

  private async parseError(response: Response): Promise<ErrorBody> {
    let data: unknown;
    try {
      data = await response.json();
    } catch {
      return synthesizeError(response.statusText || 'request failed');
    }
    if (isErrorBody(data)) {
      return data;
    }
    return synthesizeError(typeof data === 'string' ? data : JSON.stringify(data));
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
