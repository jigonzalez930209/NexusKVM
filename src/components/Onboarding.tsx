import { Monitor, Server } from 'lucide-react';
import { useState } from 'react';
import { api } from '../api';
import { toast } from '../shared/toast';
import type { RuntimeSnapshot } from '../types';

interface OnboardingProps {
  busy: boolean;
  runAction: (fn: () => Promise<RuntimeSnapshot | void>) => void;
}

export function Onboarding({ busy, runAction }: OnboardingProps) {
  const [paste, setPaste] = useState('');

  return (
    <div className="onboarding-screen">
      <div className="onboarding-hero">
        <div className="brand-icon" style={{ margin: '0 auto 1.5rem auto' }}>
          N
        </div>
        <h1>Choose Machine Role</h1>
        <p>
          Configure NexusKVM for this device. Will this machine share its
          peripherals or be remotely controlled?
        </p>
      </div>

      <div className="role-cards-grid">
        <div
          className="role-select-card"
          onClick={() => !busy && runAction(() => api.setupHost())}
        >
          <div className="role-icon-box">
            <Monitor size={24} />
          </div>
          <h3>This is the Host</h3>
          <p>
            This machine has the physical keyboard and mouse. Other devices
            connect here over the local network.
          </p>
          <button className="btn-primary" disabled={busy}>
            Initialize as Host
          </button>
        </div>

        <div className="role-select-card" style={{ cursor: 'default' }}>
          <div className="role-icon-box">
            <Server size={24} />
          </div>
          <h3>Connect to Another</h3>
          <p>Paste the pairing invite code generated on the host machine.</p>
          <div className="client-input-area">
            <textarea
              className="client-textarea"
              value={paste}
              onChange={(e) => setPaste(e.target.value)}
              placeholder='{"server":"...","password":"...","certificate":"..."}'
            />
            <button
              className="btn-primary"
              disabled={busy || !paste.trim()}
              onClick={() => {
                try {
                  const inv = JSON.parse(paste);
                  runAction(() => api.setupClient(inv));
                } catch {
                  toast.error(
                    'Invalid pairing code',
                    'Please paste a valid JSON invite code',
                  );
                }
              }}
            >
              Connect to Host
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
