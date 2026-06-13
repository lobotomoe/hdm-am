import { useState, type ReactNode } from 'react';
import { Banknote, Printer, ReceiptText } from 'lucide-react';
import {
  useCashInOut,
  usePrintLastReceipt,
  usePrintReceipt,
  useReceiptSample,
} from '@hdm-am/react';
import { Field, JsonBlock, OkBadge, Outcome } from '../ui.js';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';

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
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <ReceiptText className="size-5" />
          Cash register
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="space-y-3">
          <h3 className="text-sm font-medium">Simple receipt</h3>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field id="cashAmount" label="Cash amount (AMD)" type="number" value={cashAmount} onChange={setCashAmount} />
            <Field id="dept" label="Department" type="number" value={dept} onChange={setDept} />
          </div>
          <Button
            data-testid="print-receipt"
            disabled={printReceipt.loading}
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
          >
            <Printer className="size-4" />
            Print receipt
          </Button>
          <Outcome loading={printReceipt.loading} error={printReceipt.error} testId="receipt-outcome">
            {receipt ? (
              <OkBadge>
                fiscal {receipt.fiscal ?? '?'} · total {receipt.total ?? 0}
                {receipt.change ? ` · change ${String(receipt.change)}` : ''}
              </OkBadge>
            ) : null}
          </Outcome>
          {receipt ? <JsonBlock value={receipt} /> : null}
        </div>

        <Separator />

        <div className="space-y-3">
          <h3 className="text-sm font-medium">Cash drawer</h3>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field id="drawerAmount" label="Amount (AMD)" type="number" value={drawerAmount} onChange={setDrawerAmount} />
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              data-testid="cash-in"
              variant="outline"
              disabled={cash.loading}
              onClick={() => {
                void cash.mutate({ amount: toAmount(drawerAmount), isCashIn: true });
              }}
            >
              <Banknote className="size-4" />
              Cash in
            </Button>
            <Button
              data-testid="cash-out"
              variant="outline"
              disabled={cash.loading}
              onClick={() => {
                void cash.mutate({ amount: toAmount(drawerAmount), isCashIn: false });
              }}
            >
              <Banknote className="size-4" />
              Cash out
            </Button>
          </div>
          <Outcome loading={cash.loading} error={cash.error}>
            {cash.data ? <OkBadge>recorded</OkBadge> : null}
          </Outcome>
        </div>

        <Separator />

        <div className="space-y-3">
          <h3 className="text-sm font-medium">Other</h3>
          <div className="flex flex-wrap gap-2">
            <Button
              data-testid="reprint-last"
              variant="outline"
              disabled={last.loading}
              onClick={() => {
                void last.mutate();
              }}
            >
              Reprint last
            </Button>
            <Button
              data-testid="print-sample"
              variant="outline"
              disabled={sample.loading}
              onClick={() => {
                void sample.mutate();
              }}
            >
              Print sample
            </Button>
          </div>
          <Outcome loading={last.loading || sample.loading} error={last.error ?? sample.error}>
            {last.data || sample.data ? <OkBadge>done</OkBadge> : null}
          </Outcome>
        </div>
      </CardContent>
    </Card>
  );
}
