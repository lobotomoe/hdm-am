import { useState, type ReactNode } from 'react';
import { KeyRound, ListTree, Users } from 'lucide-react';
import { useLogin, useOperators, usePaymentSystems } from '@hdm-am/react';
import { OkBadge, Outcome } from '../ui.js';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';

export function DirectorySection(): ReactNode {
  const login = useLogin();
  const [armed, setArmed] = useState(false);
  const operators = useOperators({ enabled: armed });
  const paymentSystems = usePaymentSystems({ enabled: armed });

  const ops = operators.data?.c ?? [];
  const deps = operators.data?.d ?? [];
  const systems = paymentSystems.data?.PaymentSystems ?? [];

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Users className="size-5" />
          Operators &amp; departments
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            size="sm"
            data-testid="btn-login"
            disabled={login.loading}
            onClick={() => {
              void login.mutate();
            }}
          >
            <KeyRound className="size-4" />
            Verify login
          </Button>
          {login.data?.ok ? <OkBadge>credentials OK</OkBadge> : null}

          <Separator orientation="vertical" className="h-6" />

          <Button
            variant="outline"
            size="sm"
            data-testid="btn-load-directory"
            onClick={() => {
              setArmed(true);
              operators.refetch();
              paymentSystems.refetch();
            }}
          >
            <ListTree className="size-4" />
            Load directory
          </Button>
        </div>
        <Outcome
          loading={login.loading || operators.loading || paymentSystems.loading}
          error={login.error ?? operators.error ?? paymentSystems.error}
        />

        {operators.data ? (
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <h3 className="mb-2 text-sm font-medium">Operators</h3>
              <ul className="space-y-1 text-sm text-muted-foreground">
                {ops.map((op) => (
                  <li key={op.id}>
                    <span className="font-medium text-foreground">#{op.id}</span> {op.name ?? ''} ·
                    deps [{(op.deps ?? []).join(', ')}]
                  </li>
                ))}
              </ul>
            </div>
            <div>
              <h3 className="mb-2 text-sm font-medium">Departments</h3>
              <ul className="space-y-1 text-sm text-muted-foreground">
                {deps.map((dep) => (
                  <li key={dep.id}>
                    <span className="font-medium text-foreground">#{dep.id}</span> {dep.name ?? ''} ·
                    tax {dep.type ?? 0}
                  </li>
                ))}
              </ul>
            </div>
          </div>
        ) : null}

        {paymentSystems.data ? (
          <div>
            <h3 className="mb-2 text-sm font-medium">Payment systems</h3>
            <div className="flex flex-wrap gap-2">
              {systems.map((ps) => (
                <Badge key={ps.code} variant="secondary">
                  {ps.code} · {ps.name ?? ''}
                </Badge>
              ))}
            </div>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
