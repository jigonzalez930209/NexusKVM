import {
  AccessTokenCard,
  ActiveNodesCard,
  EngineMetricsCard,
} from './MetricsCards';
import { SpatialMatrix, SubsystemsPanel } from './SpatialAndSubsystems';
import type { Peer, RuntimeSnapshot, Status } from '../types';

interface DashboardViewProps {
  rt: RuntimeSnapshot;
  status: Status | null;
  peers: Peer[];
  busy: boolean;
  onCopyInvite: () => void;
  runAction: (fn: () => Promise<RuntimeSnapshot | Status | void>) => void;
  refresh: () => void;
}

export function DashboardView({
  rt,
  status,
  peers,
  busy,
  onCopyInvite,
  runAction,
  refresh,
}: DashboardViewProps) {
  const activeTarget = status?.active_target ?? 'local';
  const remotePeer = peers[0];

  return (
    <>
      {/* Top Metrics Row */}
      <div className="dashboard-grid">
        <EngineMetricsCard serviceOk={rt.service_ok} metrics={rt.metrics} />

        <AccessTokenCard
          role={rt.role}
          password={rt.password}
          remoteServer={rt.remote_server}
          onCopyInvite={onCopyInvite}
        />

        <ActiveNodesCard
          advertise={rt.advertise}
          listen={rt.listen}
          peers={peers}
          peerSide={rt.peer_side}
        />
      </div>

      {/* Lower Section: Spatial Boundary Layout & Subsystem Controllers */}
      <div className="lower-dashboard-section">
        <SpatialMatrix
          activeTarget={activeTarget}
          remotePeer={remotePeer}
          peerSide={rt.peer_side}
          busy={busy}
          runAction={runAction}
          refresh={refresh}
        />

        <SubsystemsPanel
          role={rt.role}
          serviceOk={rt.service_ok}
          portalAvailable={rt.portal_available}
          clipboardOk={rt.clipboard_ok}
          running={rt.running}
          socketOk={rt.socket_ok}
          activeTarget={activeTarget}
          remotePeer={remotePeer}
          busy={busy}
          runAction={runAction}
        />
      </div>
    </>
  );
}
