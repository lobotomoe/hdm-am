import { useState, type ReactNode } from 'react';
import {
  useDateTime,
  useHdmHealth,
  useHdmInfo,
  useProbe,
  useTimeSync,
} from '@hdm-am/react';
import { Button, JsonBlock, Result, Section } from '../ui.js';

export function DeviceSection(): ReactNode {
  const health = useHdmHealth();
  const info = useHdmInfo();

  // Device-touching calls are armed by a button so a full TCP round-trip never fires on mount.
  const [probeArmed, setProbeArmed] = useState(false);
  const probe = useProbe({ enabled: probeArmed });
  const [timeArmed, setTimeArmed] = useState(false);
  const dateTime = useDateTime({ enabled: timeArmed });
  const timeSync = useTimeSync();

  return (
    <Section title="Device">
      <div className="row">
        <Button
          onClick={() => {
            health.refetch();
          }}
        >
          Health
        </Button>
        {health.data ? <span className="ok">{health.data.status}</span> : null}
        <Result loading={health.loading} error={health.error} />
      </div>

      <div className="row">
        <Button
          onClick={() => {
            info.refetch();
          }}
        >
          Info
        </Button>
        <Result loading={info.loading} error={info.error}>
          {info.data ? (
            <span className="muted">
              {info.data.name} v{info.data.version} · spec {info.data.spec_version}
            </span>
          ) : null}
        </Result>
      </div>

      <div className="row">
        <Button
          onClick={() => {
            setProbeArmed(true);
            probe.refetch();
          }}
        >
          Probe
        </Button>
        <Result loading={probe.loading} error={probe.error}>
          {probe.data ? <JsonBlock value={probe.data} /> : null}
        </Result>
      </div>

      <div className="row">
        <Button
          onClick={() => {
            setTimeArmed(true);
            dateTime.refetch();
          }}
        >
          Date/time
        </Button>
        <Button
          onClick={() => {
            void timeSync.mutate();
          }}
          disabled={timeSync.loading}
        >
          Sync clock
        </Button>
        <Result loading={dateTime.loading || timeSync.loading} error={dateTime.error ?? timeSync.error}>
          {dateTime.data ? <span className="muted">{dateTime.data.dt}</span> : null}
        </Result>
      </div>
    </Section>
  );
}
