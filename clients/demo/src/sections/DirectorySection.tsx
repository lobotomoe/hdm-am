import { useState, type ReactNode } from 'react';
import { useLogin, useOperators, usePaymentSystems } from '@hdm-am/react';
import { Button, Result, Section } from '../ui.js';

export function DirectorySection(): ReactNode {
  const login = useLogin();
  const [armed, setArmed] = useState(false);
  const operators = useOperators({ enabled: armed });
  const paymentSystems = usePaymentSystems({ enabled: armed });

  return (
    <Section title="Operators & departments">
      <div className="row">
        <Button
          onClick={() => {
            void login.mutate();
          }}
          disabled={login.loading}
        >
          Verify login
        </Button>
        {login.data?.ok ? <span className="ok">credentials OK</span> : null}
        <Result loading={login.loading} error={login.error} />
      </div>

      <div className="row">
        <Button
          onClick={() => {
            setArmed(true);
            operators.refetch();
            paymentSystems.refetch();
          }}
        >
          Load directory
        </Button>
        <Result
          loading={operators.loading || paymentSystems.loading}
          error={operators.error ?? paymentSystems.error}
        />
      </div>

      {operators.data ? (
        <div className="columns">
          <div>
            <h3>Operators</h3>
            <ul>
              {(operators.data.c ?? []).map((op) => (
                <li key={op.id}>
                  #{op.id} {op.name ?? ''} · deps [{(op.deps ?? []).join(', ')}]
                </li>
              ))}
            </ul>
          </div>
          <div>
            <h3>Departments</h3>
            <ul>
              {(operators.data.d ?? []).map((dep) => (
                <li key={dep.id}>
                  #{dep.id} {dep.name ?? ''} · tax {dep.type ?? 0}
                </li>
              ))}
            </ul>
          </div>
        </div>
      ) : null}

      {paymentSystems.data ? (
        <div>
          <h3>Payment systems</h3>
          <ul>
            {(paymentSystems.data.PaymentSystems ?? []).map((ps) => (
              <li key={ps.code}>
                {ps.code} · {ps.name ?? ''}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </Section>
  );
}
