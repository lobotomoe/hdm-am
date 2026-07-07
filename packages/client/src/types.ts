// Domain-facing type aliases over the OpenAPI-generated schemas. Application code should import
// from here (or the package root), never from `./generated/openapi` directly — this layer is the
// stable contract; the generated module is an implementation detail that may be regenerated.

import type { components } from './generated/openapi.js';

type Schemas = components['schemas'];

/** Per-request connection override. Every field is optional and merges over the bridge's default. */
export type Connection = Schemas['PartialConn'];

// ---- Requests (operation params) ----
export type PrintReceiptRequest = Schemas['PrintReceiptRequest'];
export type ReceiptItem = Schemas['ReceiptItem'];
export type GetReturnableReceiptRequest = Schemas['GetReturnableReceiptRequest'];
export type PrintReturnReceiptRequest = Schemas['PrintReturnReceiptRequest'];
export type ReturnItem = Schemas['ReturnItem'];
export type FiscalReportRequest = Schemas['FiscalReportRequest'];
export type CashInOutRequest = Schemas['CashInOutRequest'];
export type SingleEmarkRequest = Schemas['SingleEmarkRequest'];
export type SetupHeaderFooterRequest = Schemas['SetupHeaderFooterRequest'];
export type SetupHeaderLogoRequest = Schemas['SetupHeaderLogoRequest'];
export type TextLine = Schemas['TextLine'];

// ---- Responses ----
export type ListOpsAndDepsResponse = Schemas['ListOpsAndDepsResponse'];
export type OperatorInfo = Schemas['OperatorInfo'];
export type DepartmentInfo = Schemas['DepartmentInfo'];
export type ReceiptResponse = Schemas['ReceiptResponse'];
export type ReturnableReceiptResponse = Schemas['ReturnableReceiptResponse'];
export type ReturnableReceiptItem = Schemas['ReturnableReceiptItem'];
export type ReturnReceiptResponse = Schemas['ReturnReceiptResponse'];
export type DateTimeResponse = Schemas['DateTimeResponse'];
export type PaymentSystemsListResponse = Schemas['PaymentSystemsListResponse'];
export type PaymentSystemEntry = Schemas['PaymentSystemEntry'];
export type EmptyResponse = Schemas['EmptyResponse'];

// ---- Meta ----
export type HdmInfo = Schemas['Info'];
export type HealthStatus = Schemas['HealthOk'];
export type StatusOk = Schemas['StatusOk'];

// ---- Error envelope ----
export type ErrorBody = Schemas['ErrorBody'];
export type ErrorDetail = Schemas['ErrorDetail'];
