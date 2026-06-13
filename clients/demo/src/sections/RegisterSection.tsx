import { useState, type ReactNode } from 'react';
import {
  useCashInOut,
  usePrintLastReceipt,
  usePrintReceipt,
  useReceiptSample,
} from '@hdm-am/react';
import { Button, Field, JsonBlock, Result, Section } from '../ui.js';

const SIMPLE_MODE = 1;

function toAmount(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function RegisterSection(): ReactNode {
  const printReceipt = usePrintReceipt();
  const cash = useCashInOut();
  const last = usePrintLastReceipt();
  const sample = useReceiptSample();

  const [cashAmount, setCashAmount] = useState('1000');
  const [dept, setDept] = useState('1');
  const [drawerAmount, setDrawerAmount] = useState('5000');

  const receipt = printReceipt.data;

  return (
    <Section title="Cash register">
      <div className="subform">
        <h3>Simple receipt</h3>
        <Field label="Cash amount (AMD)" value={cashAmount} onChange={setCashAmount} type="number" />
        <Field label="Department" value={dept} onChange={setDept} type="number" />
        <Button
          onClick={() => {
            void printReceipt.mutate({
              mode: SIMPLE_MODE,
              paidAmount: toAmount(cashAmount),
              paidAmountCard: 0,
              partialAmount: 0,
              prePaymentAmount: 0,
              useExtPOS: false,
              dep: Number.parseInt(dept, 10),
            });
          }}
          disabled={printReceipt.loading}
        >
          Print receipt
        </Button>
        <Result loading={printReceipt.loading} error={printReceipt.error}>
          {receipt ? (
            <div className="ok">
              fiscal {receipt.fiscal ?? '?'} · total {receipt.total ?? 0}
              {receipt.change ? ` · change ${String(receipt.change)}` : ''}
            </div>
          ) : null}
        </Result>
        {receipt ? <JsonBlock value={receipt} /> : null}
      </div>

      <div className="subform">
        <h3>Cash drawer</h3>
        <Field label="Amount (AMD)" value={drawerAmount} onChange={setDrawerAmount} type="number" />
        <div className="row">
          <Button
            onClick={() => {
              void cash.mutate({ amount: toAmount(drawerAmount), isCashIn: true });
            }}
            disabled={cash.loading}
          >
            Cash in
          </Button>
          <Button
            onClick={() => {
              void cash.mutate({ amount: toAmount(drawerAmount), isCashIn: false });
            }}
            disabled={cash.loading}
          >
            Cash out
          </Button>
        </div>
        <Result loading={cash.loading} error={cash.error}>
          {cash.data ? <span className="ok">recorded</span> : null}
        </Result>
      </div>

      <div className="subform">
        <h3>Other</h3>
        <div className="row">
          <Button
            onClick={() => {
              void last.mutate();
            }}
            disabled={last.loading}
          >
            Reprint last
          </Button>
          <Button
            onClick={() => {
              void sample.mutate();
            }}
            disabled={sample.loading}
          >
            Print sample
          </Button>
        </div>
        <Result loading={last.loading || sample.loading} error={last.error ?? sample.error} />
      </div>
    </Section>
  );
}
