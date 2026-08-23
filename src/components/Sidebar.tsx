import { Activity, Users, Terminal, Settings, Copy } from 'lucide-react';
import type { RuntimeSnapshot } from '../types';

export type ActiveTab = 'dashboard' | 'peers' | 'logs' | 'settings';

interface SidebarProps {
  role: 'host' | 'client';
  activeTab: ActiveTab;
  setActiveTab: (tab: ActiveTab) => void;
  serviceOk: boolean;
  peerCount: number;
  copiedInvite: boolean;
  busy: boolean;
  onCopyInvite: () => void;
}

export function Sidebar({
  role,
  activeTab,
  setActiveTab,
  serviceOk,
  peerCount,
  copiedInvite,
  busy,
  onCopyInvite,
}: SidebarProps) {
  return (
    <nav className="sidebar">
      <div className="sidebar-header">
        <div className="brand-icon">N</div>
        <div className="brand-titles">
          <h1>NexusKVM</h1>
          <span className="node-role">
            {role === 'host' ? 'Host Node' : 'Client Node'}
          </span>
        </div>
      </div>

      <ul className="nav-links">
        <li>
          <button
            className={`nav-item-btn ${activeTab === 'dashboard' ? 'active' : ''}`}
            onClick={() => setActiveTab('dashboard')}
          >
            <Activity />
            <span>Control Panel</span>
          </button>
        </li>
        <li>
          <button
            className={`nav-item-btn ${activeTab === 'peers' ? 'active' : ''}`}
            onClick={() => setActiveTab('peers')}
          >
            <Users />
            <span>Endpoints ({peerCount})</span>
          </button>
        </li>
        <li>
          <button
            className={`nav-item-btn ${activeTab === 'logs' ? 'active' : ''}`}
            onClick={() => setActiveTab('logs')}
          >
            <Terminal />
            <span>Daemon Logs</span>
          </button>
        </li>
        <li>
          <button
            className={`nav-item-btn ${activeTab === 'settings' ? 'active' : ''}`}
            onClick={() => setActiveTab('settings')}
          >
            <Settings />
            <span>Configuration</span>
          </button>
        </li>
      </ul>

      <div className="sidebar-footer">
        <div className="version-status">
          <span>v0.1.0-stable</span>
          <span className="status-online">
            {serviceOk ? 'Online' : 'Stopped'}
          </span>
        </div>
        {role === 'host' && (
          <button
            className="btn-primary"
            style={{ width: '100%', fontSize: '0.78rem' }}
            onClick={onCopyInvite}
            disabled={busy}
          >
            <Copy size={14} />
            {copiedInvite ? 'Copied!' : 'Authorize Peer'}
          </button>
        )}
      </div>
    </nav>
  );
}
