import type { ErrorBody } from './types.js';

/**
 * The set of stable `kind` tags the bridge currently emits. The wire type is an open `string`, so
 * treat this as a convenience for branching — always handle the `default` case.
 */
export const HDM_ERROR_KINDS = [
  'bad_request',
  'unauthorized',
  'internal',
  'transport',
  'transport_timeout',
  'device_error',
  'crypto',
  'decode',
  'encode',
  'not_logged_in',
  'payload_too_large',
  'not_hdm',
  'device',
] as const;

export type HdmErrorKind = (typeof HDM_ERROR_KINDS)[number];

/**
 * An error returned by the bridge: a non-2xx response carrying the standard error envelope.
 *
 * Branch on {@link kind} (stable machine tag) and {@link code} (the device/spec response code, when
 * the device itself rejected the request). The {@link retryable}, {@link requiresRelogin}, and
 * {@link requiresReconnect} flags tell a caller how to recover.
 */
export class HdmBridgeError extends Error {
  /** HTTP status code of the response. */
  readonly status: number;
  /** Stable machine-readable error tag (see {@link HDM_ERROR_KINDS}). */
  readonly kind: string;
  /** Device/spec response code when present (e.g. `174` = receipt-to-return does not exist). */
  readonly code: number | undefined;
  /** Whether retrying the same request may succeed. */
  readonly retryable: boolean;
  /** Whether the caller must log in again before retrying. */
  readonly requiresRelogin: boolean;
  /** Whether the caller must re-establish the device connection before retrying. */
  readonly requiresReconnect: boolean;
  /** The raw error envelope as received. */
  readonly body: ErrorBody;

  constructor(status: number, body: ErrorBody) {
    super(body.error.message);
    this.name = 'HdmBridgeError';
    Object.setPrototypeOf(this, new.target.prototype);
    this.status = status;
    const detail = body.error;
    this.kind = detail.kind;
    this.code = detail.code ?? undefined;
    this.retryable = detail.retryable;
    this.requiresRelogin = detail.requires_relogin;
    this.requiresReconnect = detail.requires_reconnect;
    this.body = body;
  }
}

/**
 * A failure that prevented a response from being received at all: a network error, a CORS rejection,
 * an aborted request, or a timeout. Distinct from {@link HdmBridgeError}, which carries a real HTTP
 * response. The underlying cause is preserved on {@link Error.cause}.
 */
export class HdmTransportError extends Error {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = 'HdmTransportError';
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export type HdmValidationDirection = 'request' | 'response';

/**
 * A local contract-validation failure. This is raised only when validation hooks are configured
 * (for example by `@hdm-am/client/zod`) and a request envelope or successful response does not
 * match the generated transport schema.
 */
export class HdmValidationError extends Error {
  /** Which side of the transport boundary failed validation. */
  readonly direction: HdmValidationDirection;
  /** Bridge path whose request/response failed validation. */
  readonly path: string;
  /** HTTP status for response validation failures. */
  readonly status: number | undefined;

  constructor(
    direction: HdmValidationDirection,
    path: string,
    options: { status?: number; cause?: unknown } = {},
  ) {
    super(`${direction} payload for ${path} failed validation`, { cause: options.cause });
    this.name = 'HdmValidationError';
    Object.setPrototypeOf(this, new.target.prototype);
    this.direction = direction;
    this.path = path;
    this.status = options.status;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/** Narrow an unknown JSON value to the bridge error envelope. */
export function isErrorBody(value: unknown): value is ErrorBody {
  if (!isRecord(value) || !isRecord(value.error)) {
    return false;
  }
  const { error } = value;
  return (
    typeof error.kind === 'string' &&
    typeof error.message === 'string' &&
    typeof error.retryable === 'boolean' &&
    typeof error.requires_relogin === 'boolean' &&
    typeof error.requires_reconnect === 'boolean' &&
    (error.code === undefined || error.code === null || typeof error.code === 'number')
  );
}
