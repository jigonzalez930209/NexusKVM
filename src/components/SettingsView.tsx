import { Settings } from 'lucide-react';
import { api } from '../api';
import type { Role, RuntimeSnapshot, Status } from '../types';

interface SettingsViewProps {
  role: Role | null;
  status: Status | null;
  runAction: (fn: () => Promise<RuntimeSnapshot | Status | void>) => void;
}

export function SettingsView({ role, status, runAction }: SettingsViewProps) {
  return (
    <div
      className="obsidian-card"
      style={{ flex: 1, minHeight: 0, overflowY: 'auto' }}
    >
      <div className="card-header">
        <span className="card-title">
          <Settings size={16} /> Runtime Configuration
        </span>
      </div>

      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '1rem',
          marginTop: '0.5rem',
        }}
      >
        <div className="subsystem-row">
          <div>
            <div style={{ fontWeight: 600, fontSize: '0.85rem' }}>
              Emergency Fail-Safe Hotkey
            </div>
            <div style={{ fontSize: '0.72rem', color: 'var(--text-muted)' }}>
              Restores input control immediately to this host.
            </div>
          </div>
          <kbd
            style={{
              fontFamily: 'var(--font-mono)',
              background: 'var(--bg-surface-high)',
              padding: '0.3rem 0.6rem',
              borderRadius: '4px',
              border: '1px solid var(--border-standard)',
              fontSize: '0.75rem',
            }}
          >
            {status?.emergency_shortcut ?? 'Left Alt + Left Ctrl'}
          </kbd>
        </div>

        <div className="subsystem-row">
          <div>
            <div style={{ fontWeight: 600, fontSize: '0.85rem' }}>
              Sticky Modifiers Clear
            </div>
            <div style={{ fontSize: '0.72rem', color: 'var(--text-muted)' }}>
              Releases all held modifier keys on screen switch.
            </div>
          </div>
          {role === 'host' && (
            <button className="btn-secondary" onClick={() => api.releaseAll()}>
              Release Now
            </button>
          )}
        </div>

        <div className="subsystem-row">
          <div>
            <div
              style={{
                fontWeight: 600,
                fontSize: '0.85rem',
                color: 'var(--error)',
              }}
            >
              Reset Role & Re-pair
            </div>
            <div style={{ fontSize: '0.72rem', color: 'var(--text-muted)' }}>
              Wipes local configuration and returns to the initial role
              selection.
            </div>
          </div>
          <button
            className="btn-secondary"
            style={{
              borderColor: 'var(--error-border)',
              color: 'var(--error)',
            }}
            onClick={() => runAction(() => api.reset())}
          >
            Reset Setup
          </button>
        </div>
      </div>
    </div>
  );
}
