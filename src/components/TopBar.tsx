import {
  Bell,
  ShieldCheck,
  Wifi,
  RefreshCw,
  AlertCircle,
  CheckCircle2,
} from 'lucide-react';
import { api } from '../api';

export interface NotificationItem {
  id: string;
  type: 'warn' | 'info';
  title: string;
  desc: string;
}

interface TopBarProps {
  serviceOk: boolean;
  listen: string;
  notifications: NotificationItem[];
  onRefresh: () => void;
}

export function TopBar({
  serviceOk,
  listen,
  notifications,
  onRefresh,
}: TopBarProps) {
  function handleMouseDown(e: React.MouseEvent) {
    // Only initiate window drag if left button clicked and not on a button/input
    if (
      e.button === 0 &&
      !(e.target as HTMLElement).closest('button, input, a, textarea')
    ) {
      api.startDragging();
    }
  }

  return (
    <header
      className="custom-topbar"
      data-tauri-drag-region
      onMouseDown={handleMouseDown}
    >
      <div className="topbar-left" data-tauri-drag-region>
        <span className={`status-pill ${serviceOk ? 'active' : 'inactive'}`}>
          <span className="pulse-dot" />
          {serviceOk ? 'DAEMON ACTIVE' : 'DAEMON STOPPED'}
        </span>
        <span className="port-tag">
          <Wifi size={13} /> {listen}
        </span>
      </div>

      <div className="topbar-right">
        <div className="security-badge">
          <ShieldCheck size={12} />
          <span>Zero-Trust TLS 1.3</span>
        </div>

        <div className="topbar-divider" />

        {/* Notification dialog icon with hover popup */}
        <div className="notification-container">
          <button className="topbar-btn" title="System Notifications & Alerts">
            <Bell size={16} />
            {notifications.length > 0 && (
              <span
                className={`notif-badge-dot ${
                  notifications.some((n) => n.type === 'warn')
                    ? 'has-error'
                    : ''
                }`}
              />
            )}
          </button>

          <div className="notif-popup">
            <div className="notif-header">
              <span>System Diagnostics</span>
              <span className="tag-badge">{notifications.length} alerts</span>
            </div>
            <div className="notif-list">
              {notifications.length === 0 ? (
                <div
                  className="notif-item"
                  style={{ color: 'var(--text-muted)' }}
                >
                  No active warnings or alerts.
                </div>
              ) : (
                notifications.map((n) => (
                  <div key={n.id} className={`notif-item ${n.type}`}>
                    {n.type === 'warn' ? (
                      <AlertCircle
                        size={16}
                        style={{
                          color: 'var(--error)',
                          flexShrink: 0,
                          marginTop: '2px',
                        }}
                      />
                    ) : (
                      <CheckCircle2
                        size={16}
                        style={{
                          color: 'var(--success)',
                          flexShrink: 0,
                          marginTop: '2px',
                        }}
                      />
                    )}
                    <div className="notif-item-content">
                      <span className="notif-item-title">{n.title}</span>
                      <span className="notif-item-desc">{n.desc}</span>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>

        <button
          className="topbar-btn"
          onClick={onRefresh}
          title="Refresh Daemon Status"
        >
          <RefreshCw size={15} />
        </button>

        <div className="topbar-divider" />

        {/* Window controls matching stitch design */}
        <div className="window-controls-group">
          <button
            className="win-ctrl-btn"
            onClick={() => api.minimizeWindow()}
            title="Minimize"
          >
            <span className="win-icon-minus" />
          </button>
          <button
            className="win-ctrl-btn"
            onClick={() => api.toggleMaximize()}
            title="Maximize / Restore"
          >
            <span className="win-icon-square" />
          </button>
          <button
            className="win-ctrl-btn close"
            onClick={() => api.hideWindow()}
            title="Close (Hide to Tray)"
          >
            <span className="win-icon-close">✕</span>
          </button>
        </div>
      </div>
    </header>
  );
}
