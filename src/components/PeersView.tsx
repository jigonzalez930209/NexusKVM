import { Users } from 'lucide-react';
import { api } from '../api';
import type { Peer, Role, RuntimeSnapshot, Status } from '../types';

interface PeersViewProps {
  role: Role | null;
  peers: Peer[];
  listen: string;
  advertise: string;
  socketOk: boolean;
  busy: boolean;
  runAction: (fn: () => Promise<RuntimeSnapshot | Status | void>) => void;
}

export function PeersView({
  role,
  peers,
  listen,
  advertise,
  socketOk,
  busy,
  runAction,
}: PeersViewProps) {
  return (
    <div
      className="obsidian-card"
      style={{ flex: 1, minHeight: 0, overflow: 'hidden' }}
    >
      <div className="card-header">
        <span className="card-title">
          <Users size={16} /> Managed Endpoints
        </span>
        <span className="tag-badge">{peers.length} Authorized</span>
      </div>

      <div style={{ flex: 1, overflowY: 'auto', paddingRight: '4px' }}>
        <ul
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '0.65rem',
            listStyle: 'none',
          }}
        >
          <li className="peer-row-item" style={{ padding: '0.875rem' }}>
            <div className="peer-info-left">
              <div className="peer-dot" />
              <div className="peer-details">
                <span className="peer-name" style={{ fontSize: '1rem' }}>
                  Primary Host (This Device)
                </span>
                <span className="peer-sub">
                  {advertise || listen} • Direct Hardware Ownership
                </span>
              </div>
            </div>
            <span className="tag-badge success">LOCAL NODE</span>
          </li>

          {peers.map((p) => (
            <li
              key={p.id}
              className="peer-row-item"
              style={{ padding: '0.875rem' }}
            >
              <div className="peer-info-left">
                <div className="peer-dot" />
                <div className="peer-details">
                  <span className="peer-name" style={{ fontSize: '1rem' }}>
                    {p.name || p.id}
                  </span>
                  <span className="peer-sub">
                    {p.address} • TLS 1.3 Handshake OK
                  </span>
                </div>
              </div>
              <div style={{ display: 'flex', gap: '0.5rem' }}>
                {role === 'host' && (
                  <button
                    className="btn-primary"
                    style={{ fontSize: '0.75rem', padding: '0.4rem 0.75rem' }}
                    disabled={busy || !socketOk}
                    onClick={() => runAction(() => api.switchTo(p.id))}
                  >
                    Switch Focus
                  </button>
                )}
              </div>
            </li>
          ))}

          {peers.length === 0 && (
            <div
              style={{
                padding: '2.5rem',
                textAlign: 'center',
                color: 'var(--text-muted)',
              }}
            >
              <p>No remote peers connected.</p>
              <p
                style={{
                  fontSize: '0.75rem',
                  marginTop: '0.5rem',
                  color: 'var(--text-dim)',
                }}
              >
                Copy the pairing code from the dashboard and paste it into
                NexusKVM on another machine.
              </p>
            </div>
          )}
        </ul>
      </div>
    </div>
  );
}
