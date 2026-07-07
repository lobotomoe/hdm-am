export { HdmProvider, useHdmClient } from './context.js';
export type { HdmProviderProps } from './context.js';
export { useQuery, useMutation } from './use-async.js';
export type { QueryResult, QueryOptions, MutationResult } from './use-async.js';
export {
  useHdmHealth,
  useHdmInfo,
  useOperators,
  usePaymentSystems,
  useDateTime,
  useLogin,
  usePrintReceipt,
  usePrintLastReceipt,
  useLookupReceipt,
  usePrintReturn,
  useReport,
  useCashInOut,
  useTimeSync,
  useReceiptSample,
  useEmark,
  useHeaderFooter,
  useHeaderLogo,
} from './hooks.js';

// Re-export the client surface so consumers can use one import for both layers.
export * from '@hdm-am/client';
