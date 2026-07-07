import { HdmBridgeClient } from './client.js';
import type {
  HdmApiPath,
  HdmClientOptions,
  HdmTransportValidator,
  HdmTransportValidatorMap,
  HdmValidationOptions,
} from './client.js';
import * as schemas from './generated/zod.js';
import type { z } from 'zod';

export * from './generated/zod.js';

export interface HdmZodValidationOptions {
  /** Validate request envelopes before they are sent. Defaults to `true`. */
  requests?: boolean;
  /** Validate successful response payloads after they are parsed. Defaults to `true`. */
  responses?: boolean;
}

type ZodSchema = z.ZodType;

function validateWith(schema: ZodSchema): HdmTransportValidator {
  return (value: unknown): void => {
    const result = schema.safeParse(value);
    if (!result.success) {
      throw result.error;
    }
  };
}

export const hdmZodRequestValidators = {
  '/v1/cash': validateWith(schemas.CashInOutBody),
  '/v1/datetime': validateWith(schemas.DateTimeBody),
  '/v1/emark': validateWith(schemas.EmarkBody),
  '/v1/header-footer': validateWith(schemas.HeaderFooterBody),
  '/v1/login': validateWith(schemas.LoginBody),
  '/v1/logo': validateWith(schemas.HeaderLogoBody),
  '/v1/operators': validateWith(schemas.OperatorsBody),
  '/v1/payment-systems': validateWith(schemas.PaymentSystemsBody),
  '/v1/receipt': validateWith(schemas.PrintReceiptBody),
  '/v1/receipt/last': validateWith(schemas.PrintLastReceiptBody),
  '/v1/receipt/lookup': validateWith(schemas.LookupReceiptBody),
  '/v1/report': validateWith(schemas.ReportBody),
  '/v1/return': validateWith(schemas.PrintReturnBody),
  '/v1/sample': validateWith(schemas.ReceiptSampleBody),
  '/v1/time-sync': validateWith(schemas.TimeSyncBody),
} satisfies HdmTransportValidatorMap;

export const hdmZodResponseValidators = {
  '/v1/cash': validateWith(schemas.CashInOutResponse),
  '/v1/datetime': validateWith(schemas.DateTimeResponse),
  '/v1/emark': validateWith(schemas.EmarkResponse),
  '/v1/header-footer': validateWith(schemas.HeaderFooterResponse),
  '/v1/health': validateWith(schemas.HealthResponse),
  '/v1/info': validateWith(schemas.InfoResponse),
  '/v1/login': validateWith(schemas.LoginResponse),
  '/v1/logo': validateWith(schemas.HeaderLogoResponse),
  '/v1/openapi.json': validateWith(schemas.OpenapiDocumentResponse),
  '/v1/operators': validateWith(schemas.OperatorsResponse),
  '/v1/payment-systems': validateWith(schemas.PaymentSystemsResponse),
  '/v1/receipt': validateWith(schemas.PrintReceiptResponse),
  '/v1/receipt/last': validateWith(schemas.PrintLastReceiptResponse),
  '/v1/receipt/lookup': validateWith(schemas.LookupReceiptResponse),
  '/v1/report': validateWith(schemas.ReportResponse),
  '/v1/return': validateWith(schemas.PrintReturnResponse),
  '/v1/sample': validateWith(schemas.ReceiptSampleResponse),
  '/v1/time-sync': validateWith(schemas.TimeSyncResponse),
} satisfies HdmTransportValidatorMap;

const requestPaths = Object.keys(hdmZodRequestValidators) as HdmApiPath[];
const responsePaths = Object.keys(hdmZodResponseValidators) as HdmApiPath[];

export function createZodValidation(
  options: HdmZodValidationOptions = {},
): HdmValidationOptions {
  const validation: HdmValidationOptions = {
    requestValidators: hdmZodRequestValidators,
    responseValidators: hdmZodResponseValidators,
  };
  if (options.requests !== undefined) {
    validation.requests = options.requests;
  }
  if (options.responses !== undefined) {
    validation.responses = options.responses;
  }
  return validation;
}

export function withZodValidation(
  options: HdmClientOptions,
  validationOptions: HdmZodValidationOptions = {},
): HdmClientOptions {
  return {
    ...options,
    validation: createZodValidation(validationOptions),
  };
}

export function createValidatedClient(
  options: HdmClientOptions,
  validationOptions: HdmZodValidationOptions = {},
): HdmBridgeClient {
  return new HdmBridgeClient(withZodValidation(options, validationOptions));
}

export { createValidatedClient as createZodValidatedClient };
export const hdmZodRequestPaths = requestPaths;
export const hdmZodResponsePaths = responsePaths;
