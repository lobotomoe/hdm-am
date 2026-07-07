import { useState, type ReactNode } from 'react';
import { Activity, Clock, Info, RefreshCw } from 'lucide-react';
import { useDateTime, useHdmHealth, useHdmInfo, useTimeSync } from '@hdm-am/react';
import { OkBadge, Outcome } from '../ui.js';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';

export function DeviceSection(): ReactNode {
  const health = useHdmHealth();
  const info = useHdmInfo();

  // Device-touching calls are armed by a button so a full TCP round-trip never fires on mount.
  const [timeArmed, setTimeArmed] = useState(false);
  const dateTime = useDateTime({ enabled: timeArmed });
  const timeSync = useTimeSync();

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Activity className="size-5" />
          Device
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            size="sm"
            data-testid="btn-health"
            onClick={() => {
              health.refetch();
            }}
          >
            <RefreshCw className="size-4" />
            Health
          </Button>
          {health.data ? (
            <OkBadge>{health.data.status}</OkBadge>
          ) : health.error ? (
            <Badge variant="destructive">offline</Badge>
          ) : null}

          <Separator orientation="vertical" className="h-6" />

          <Button
            variant="outline"
            size="sm"
            data-testid="btn-info"
            onClick={() => {
              info.refetch();
            }}
          >
            <Info className="size-4" />
            Info
          </Button>
          {info.data ? (
            <span className="text-sm text-muted-foreground">
              {info.data.name} v{info.data.version} · spec {info.data.spec_version}
            </span>
          ) : null}
        </div>
        <Outcome loading={health.loading || info.loading} error={health.error ?? info.error} />

        <Separator />

        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="outline"
            size="sm"
            data-testid="btn-datetime"
            onClick={() => {
              setTimeArmed(true);
              dateTime.refetch();
            }}
          >
            <Clock className="size-4" />
            Date/time
          </Button>
          <Button
            variant="outline"
            size="sm"
            data-testid="btn-sync"
            disabled={timeSync.loading}
            onClick={() => {
              void timeSync.mutate();
            }}
          >
            Sync clock
          </Button>
          {timeSync.data ? <OkBadge>synced</OkBadge> : null}
        </div>
        <Outcome
          loading={dateTime.loading || timeSync.loading}
          error={dateTime.error ?? timeSync.error}
        >
          {dateTime.data ? (
            <span className="text-sm text-muted-foreground">{dateTime.data.dt}</span>
          ) : null}
        </Outcome>
      </CardContent>
    </Card>
  );
}
