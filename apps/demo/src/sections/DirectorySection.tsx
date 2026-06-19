import { useState, type ReactNode } from 'react';
import { KeyRound, ListTree, Users } from 'lucide-react';
import { useLogin, useOperators, usePaymentSystems } from '@hdm-am/react';
import type { DepartmentInfo, OperatorInfo } from '@hdm-am/react';
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
  const departments = operators.data?.d ?? [];
  const departmentsById = new Map(departments.map((department) => [department.id, department]));
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
              <ul className="space-y-3 text-sm text-muted-foreground">
                {ops.map((op) => {
                  const assignedDepartments = assignedDepartmentDetails(op, departmentsById);
                  return (
                    <li key={op.id}>
                      <div>
                        <span className="font-medium text-foreground">#{op.id}</span>{' '}
                        {displayName(op.name, '[operator name not provided]')}
                      </div>
                      {assignedDepartments.length > 0 ? (
                        <div className="mt-1 flex flex-wrap gap-1.5">
                          {assignedDepartments.map((department) => (
                            <Badge
                              key={department.id}
                              variant={department.missing ? 'destructive' : 'secondary'}
                              className="h-auto max-w-full justify-start whitespace-normal py-1 text-left leading-snug"
                            >
                              #{department.id} · {department.name} · {department.taxation}
                            </Badge>
                          ))}
                        </div>
                      ) : (
                        <div className="mt-1 text-xs">No departments assigned</div>
                      )}
                    </li>
                  );
                })}
              </ul>
            </div>
            <div>
              <h3 className="mb-2 text-sm font-medium">Departments</h3>
              <ul className="space-y-1 text-sm text-muted-foreground">
                {departments.map((dep) => (
                  <li key={dep.id}>
                    <span className="font-medium text-foreground">#{dep.id}</span>{' '}
                    {displayName(dep.name, '[department name not provided]')} ·{' '}
                    {taxationLabel(dep.type)}
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

interface AssignedDepartment {
  id: number;
  name: string;
  taxation: string;
  missing: boolean;
}

function assignedDepartmentDetails(
  operator: OperatorInfo,
  departmentsById: Map<number, DepartmentInfo>,
): AssignedDepartment[] {
  return (operator.deps ?? []).map((id) => {
    const department = departmentsById.get(id);
    if (!department) {
      return {
        id,
        name: 'Unknown department',
        taxation: 'not in device directory',
        missing: true,
      };
    }
    return {
      id,
      name: displayName(department.name, '[department name not provided]'),
      taxation: taxationLabel(department.type),
      missing: false,
    };
  });
}

function displayName(name: string | undefined, fallback: string): string {
  return name && name.length > 0 ? name : fallback;
}

function taxationLabel(code: number | undefined): string {
  switch (code) {
    case 1:
      return 'VAT-taxable';
    case 2:
      return 'not VAT-taxable';
    case 3:
      return 'turnover tax';
    case 4:
      return 'production licensee';
    case 5:
      return 'patented';
    case 6:
      return 'family business';
    case 7:
      return 'micro-business';
    case 0:
    case undefined:
      return 'unknown';
    default:
      return `unknown (${String(code)})`;
  }
}
