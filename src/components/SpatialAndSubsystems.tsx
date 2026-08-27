import {
  Layers,
  MousePointer2,
  Power,
  ShieldCheck,
  ArrowLeftRight,
} from 'lucide-react';
import { api } from '../api';
import type { Peer, RuntimeSnapshot, Status } from '../types';

interface SpatialMatrixProps {
  activeTarget: string;
  remotePeer: Peer | undefined;
  peerSide: string | null;
  busy: boolean;
  runAction: (fn: () => Promise<RuntimeSnapshot | Status | void>) => void;
  refresh: () => void;
}

export function SpatialMatrix({
  activeTarget,
  remotePeer,
  peerSide,
  busy,
  runAction,
  refresh,
}: SpatialMatrixProps) {
  return (
    <div className="spatial-canvas">
      <div className="card-header">
        <span className="card-title">
          <Layers size={14} /> Spatial Boundary Matrix
        </span>
        <span className="tag-badge">Target: {activeTarget.toUpperCase()}</span>
      </div>

      <div className="spatial-visualizer">
        <div className="screens-arrangement">
          <div
            className={`screen-box ${activeTarget === 'local' ? 'active-host' : ''}`}
          >
            <span className="screen-label">Local Screen</span>
            <span className="screen-tag">
              {activeTarget === 'local' ? 'Active' : 'Standby'}
            </span>
          </div>

          <div
            className={`screen-box ${activeTarget !== 'local' ? 'active-host' : 'remote-peer'}`}
          >
            <span className="screen-label">
              {remotePeer ? remotePeer.name || 'Remote PC' : 'Peer'}
            </span>
            <span className="screen-tag">{peerSide?.toUpperCase()}</span>
          </div>
        </div>
      </div>

      {/* Edge Selector Bar */}
      <div className="edge-selector-bar">
        <span>Transition Edge:</span>
        <div className="edge-btn-group">
          {(['left', 'right'] as const).map((side) => (
            <button
              key={side}
              className={`edge-btn ${(peerSide ?? 'right') === side ? 'active' : ''}`}
              disabled={busy}
              onClick={() =>
                runAction(async () => {
                  await api.setPeerSide(side);
                  refresh();
                })
              }
            >
              {side.toUpperCase()}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

interface SubsystemsPanelProps {
  role: string | null;
  serviceOk: boolean;
  portalAvailable: boolean;
  clipboardOk: boolean;
  running: boolean;
  socketOk: boolean;
  activeTarget: string;
  remotePeer: Peer | undefined;
  busy: boolean;
  runAction: (fn: () => Promise<RuntimeSnapshot | Status | void>) => void;
}

export function SubsystemsPanel({
  role,
  serviceOk,
  portalAvailable,
  clipboardOk,
  running,
  socketOk,
  activeTarget,
  remotePeer,
  busy,
  runAction,
}: SubsystemsPanelProps) {
  return (
    <div className="subsystems-panel">
      <div className="card-header">
        <span className="card-title">
          <ShieldCheck size={14} /> Subsystem States
        </span>
      </div>

      <div className="subsystem-row">
        <div className="subsystem-left">
          <Power />
          <span className="subsystem-title">
            {role === 'host' ? 'Host Daemon' : 'Client Service'}
          </span>
        </div>
        <span className={`subsystem-state-badge ${serviceOk ? 'ok' : 'off'}`}>
          {serviceOk ? 'ACTIVE' : 'STOPPED'}
        </span>
      </div>

      <div className="subsystem-row">
        <div className="subsystem-left">
          <MousePointer2 />
          <span className="subsystem-title">Wayland InputCapture</span>
        </div>
        <span
          className={`subsystem-state-badge ${portalAvailable ? 'ok' : 'off'}`}
        >
          {portalAvailable ? 'ARMED' : 'PENDING'}
        </span>
      </div>

      <div className="subsystem-row">
        <div className="subsystem-left">
          <ArrowLeftRight />
          <span className="subsystem-title">Clipboard Bridge</span>
        </div>
        <span className={`subsystem-state-badge ${clipboardOk ? 'ok' : 'off'}`}>
          {clipboardOk ? 'SYNCED' : 'OFF'}
        </span>
      </div>

      <div
        style={{
          marginTop: 'auto',
          display: 'flex',
          gap: '0.5rem',
          paddingTop: '0.5rem',
        }}
      >
        {role === 'host' && remotePeer && (
          <button
            className="btn-primary"
            style={{ flex: 1 }}
            disabled={busy || !socketOk}
            onClick={() =>
              runAction(() =>
                activeTarget === 'local'
                  ? api.switchTo(remotePeer.id)
                  : api.local(),
              )
            }
          >
            <ArrowLeftRight size={14} />
            {activeTarget === 'local' ? 'Control Remote' : 'Return Local'}
          </button>
        )}

        {running ? (
          <button
            className="btn-secondary"
            style={{ color: 'var(--error)' }}
            disabled={busy}
            onClick={() => runAction(() => api.stop())}
          >
            <Power size={14} /> Stop
          </button>
        ) : (
          <button
            className="btn-primary"
            style={{ flex: 1 }}
            disabled={busy}
            onClick={() => runAction(() => api.start())}
          >
            <Power size={14} /> Start Service
          </button>
        )}
      </div>
    </div>
  );
}
