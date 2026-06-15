import type {
  CashInOutRequest,
  FiscalReportRequest,
  GetReturnableReceiptRequest,
  PrintReceiptRequest,
  PrintReturnReceiptRequest,
  RequestOptions,
  SetupHeaderFooterRequest,
  SetupHeaderLogoRequest,
  SingleEmarkRequest,
} from '@hdm-am/client';
import { useHdmClient } from './context.js';
import { useMutation, useQuery } from './use-async.js';
import type { MutationResult, QueryOptions, QueryResult } from './use-async.js';

// ---- Queries (read-only; run automatically) ----

/** Bridge liveness. */
export function useHdmHealth(options?: QueryOptions): QueryResult<Awaited<ReturnType<HdmHealth>>> {
  const client = useHdmClient();
  return useQuery((signal) => client.health({ signal }), [client], options);
}
type HdmHealth = ReturnType<typeof useHdmClient>['health'];

/** Bridge metadata and the operation list. */
export function useHdmInfo(options?: QueryOptions): QueryResult<Awaited<ReturnType<HdmInfo>>> {
  const client = useHdmClient();
  return useQuery((signal) => client.info({ signal }), [client], options);
}
type HdmInfo = ReturnType<typeof useHdmClient>['info'];

/** Probe the configured device and read its identity. */
export function useProbe(options?: QueryOptions): QueryResult<Awaited<ReturnType<Probe>>> {
  const client = useHdmClient();
  return useQuery((signal) => client.probe({ signal }), [client], options);
}
type Probe = ReturnType<typeof useHdmClient>['probe'];

/** The device's operators and departments. */
export function useOperators(options?: QueryOptions): QueryResult<Awaited<ReturnType<Operators>>> {
  const client = useHdmClient();
  return useQuery((signal) => client.operators({ signal }), [client], options);
}
type Operators = ReturnType<typeof useHdmClient>['operators'];

/** Payment systems configured on the device. */
export function usePaymentSystems(
  options?: QueryOptions,
): QueryResult<Awaited<ReturnType<PaymentSystems>>> {
  const client = useHdmClient();
  return useQuery((signal) => client.paymentSystems({ signal }), [client], options);
}
type PaymentSystems = ReturnType<typeof useHdmClient>['paymentSystems'];

/** The device date and time. */
export function useDateTime(options?: QueryOptions): QueryResult<Awaited<ReturnType<DateTime>>> {
  const client = useHdmClient();
  return useQuery((signal) => client.dateTime({ signal }), [client], options);
}
type DateTime = ReturnType<typeof useHdmClient>['dateTime'];

// ---- Mutations (triggered explicitly) ----

/** Verify operator login credentials. */
export function useLogin(): MutationResult<[RequestOptions?], Awaited<ReturnType<Login>>> {
  const client = useHdmClient();
  return useMutation((opts?: RequestOptions) => client.login(opts));
}
type Login = ReturnType<typeof useHdmClient>['login'];

/** Print a fiscal receipt. */
export function usePrintReceipt(): MutationResult<
  [PrintReceiptRequest, RequestOptions?],
  Awaited<ReturnType<PrintReceipt>>
> {
  const client = useHdmClient();
  return useMutation((params: PrintReceiptRequest, opts?: RequestOptions) =>
    client.printReceipt(params, opts),
  );
}
type PrintReceipt = ReturnType<typeof useHdmClient>['printReceipt'];

/** Print a copy of the last receipt. */
export function usePrintLastReceipt(): MutationResult<
  [RequestOptions?],
  Awaited<ReturnType<PrintLast>>
> {
  const client = useHdmClient();
  return useMutation((opts?: RequestOptions) => client.printLastReceipt(opts));
}
type PrintLast = ReturnType<typeof useHdmClient>['printLastReceipt'];

/** Look up a returnable receipt's contents. */
export function useLookupReceipt(): MutationResult<
  [GetReturnableReceiptRequest, RequestOptions?],
  Awaited<ReturnType<Lookup>>
> {
  const client = useHdmClient();
  return useMutation((params: GetReturnableReceiptRequest, opts?: RequestOptions) =>
    client.lookupReceipt(params, opts),
  );
}
type Lookup = ReturnType<typeof useHdmClient>['lookupReceipt'];

/** Print a return receipt. */
export function usePrintReturn(): MutationResult<
  [PrintReturnReceiptRequest, RequestOptions?],
  Awaited<ReturnType<PrintReturn>>
> {
  const client = useHdmClient();
  return useMutation((params: PrintReturnReceiptRequest, opts?: RequestOptions) =>
    client.printReturn(params, opts),
  );
}
type PrintReturn = ReturnType<typeof useHdmClient>['printReturn'];

/** Print an X or Z fiscal report. */
export function useReport(): MutationResult<
  [FiscalReportRequest, RequestOptions?],
  Awaited<ReturnType<Report>>
> {
  const client = useHdmClient();
  return useMutation((params: FiscalReportRequest, opts?: RequestOptions) =>
    client.report(params, opts),
  );
}
type Report = ReturnType<typeof useHdmClient>['report'];

/** Register a cash-drawer in or out. */
export function useCashInOut(): MutationResult<
  [CashInOutRequest, RequestOptions?],
  Awaited<ReturnType<CashInOut>>
> {
  const client = useHdmClient();
  return useMutation((params: CashInOutRequest, opts?: RequestOptions) =>
    client.cashInOut(params, opts),
  );
}
type CashInOut = ReturnType<typeof useHdmClient>['cashInOut'];

/** Synchronize the device clock. */
export function useTimeSync(): MutationResult<[RequestOptions?], Awaited<ReturnType<TimeSync>>> {
  const client = useHdmClient();
  return useMutation((opts?: RequestOptions) => client.timeSync(opts));
}
type TimeSync = ReturnType<typeof useHdmClient>['timeSync'];

/** Print a sample receipt. */
export function useReceiptSample(): MutationResult<
  [RequestOptions?],
  Awaited<ReturnType<Sample>>
> {
  const client = useHdmClient();
  return useMutation((opts?: RequestOptions) => client.receiptSample(opts));
}
type Sample = ReturnType<typeof useHdmClient>['receiptSample'];

/** Validate a single eMark code. */
export function useEmark(): MutationResult<
  [SingleEmarkRequest, RequestOptions?],
  Awaited<ReturnType<Emark>>
> {
  const client = useHdmClient();
  return useMutation((params: SingleEmarkRequest, opts?: RequestOptions) =>
    client.emark(params, opts),
  );
}
type Emark = ReturnType<typeof useHdmClient>['emark'];

/** Configure receipt header and footer lines. */
export function useHeaderFooter(): MutationResult<
  [SetupHeaderFooterRequest, RequestOptions?],
  Awaited<ReturnType<HeaderFooter>>
> {
  const client = useHdmClient();
  return useMutation((params: SetupHeaderFooterRequest, opts?: RequestOptions) =>
    client.headerFooter(params, opts),
  );
}
type HeaderFooter = ReturnType<typeof useHdmClient>['headerFooter'];

/** Configure the receipt header logo. */
export function useHeaderLogo(): MutationResult<
  [SetupHeaderLogoRequest, RequestOptions?],
  Awaited<ReturnType<HeaderLogo>>
> {
  const client = useHdmClient();
  return useMutation((params: SetupHeaderLogoRequest, opts?: RequestOptions) =>
    client.headerLogo(params, opts),
  );
}
type HeaderLogo = ReturnType<typeof useHdmClient>['headerLogo'];
