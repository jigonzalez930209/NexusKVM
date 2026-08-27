import { useEffect, useMemo, useState } from 'react';
import { api, inTauri } from './api';
import { DashboardView } from './components/DashboardView';
import { LogsView } from './components/LogsView';
import { Onboarding } from './components/Onboarding';
import { PeersView } from './components/PeersView';
import { SettingsView } from './components/SettingsView';
import { ActiveTab, Sidebar } from './components/Sidebar';
import { NotificationItem, TopBar } from './components/TopBar';
import { toast } from './shared/toast';
import type { RuntimeSnapshot, Status } from './types';

const emptyRuntime: RuntimeSnapshot = {
  role: null,
  running: false,
  socket_ok: false,
  service_ok: false,
  listen: '0.0.0.0:5258',
  advertise: '',
  remote_server: null,
  password: '',
  error: null,
  needs_logout: false,
  log_dir: null,
  service_log: null,
  daemon: null,
  binary_host: null,
  binary_client: null,
  peer_side: 'right',
  portal_available: false,
  portal_error: null,
  clipboard_ok: false,
  metrics: {
    pid: null,
    service: null,
    cpu_percent: 0,
    mem_mb: 0,
    uptime_secs: 0,
  },
};

export default function App() {
  const [rt, setRt] = useState<RuntimeSnapshot>(emptyRuntime);
  const [activeTab, setActiveTab] = useState<ActiveTab>('dashboard');
  const [busy, setBusy] = useState(false);
  const [copiedInvite, setCopiedInvite] = useState(false);

  const status: Status | null = rt.daemon;
  const peers = useMemo(
    () => (status ? Object.values(status.peers) : []),
    [status],
  );

  async function refresh() {
    if (!inTauri()) return;
    try {
      setRt(await api.runtime());
    } catch (e) {
      toast.error('Connection error', String(e));
    }
  }

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 2000);

    let unlistenTarget: (() => void) | undefined;
    let unlistenSide: (() => void) | undefined;
    let unlistenStatus: (() => void) | undefined;

    if (inTauri()) {
      api
        .onTargetChanged(() => {
          refresh();
        })
        .then((u) => {
          unlistenTarget = u;
        })
        .catch(() => {});

      api
        .onPeerSideChanged(() => {
          refresh();
        })
        .then((u) => {
          unlistenSide = u;
        })
        .catch(() => {});

      api
        .onStatusChanged(() => {
          refresh();
        })
        .then((u) => {
          unlistenStatus = u;
        })
        .catch(() => {});
    }

    return () => {
      clearInterval(interval);
      if (unlistenTarget) unlistenTarget();
      if (unlistenSide) unlistenSide();
      if (unlistenStatus) unlistenStatus();
    };
  }, []);

  useEffect(() => {
    if (!inTauri()) return;
    api.showEdgePortal().catch(() => {});
  }, [rt.peer_side]);

  async function runAction(fn: () => Promise<RuntimeSnapshot | Status | void>) {
    setBusy(true);
    try {
      const out = await fn();
      if (out && typeof out === 'object' && 'role' in out) {
        setRt(out as RuntimeSnapshot);
      } else {
        await refresh();
      }
    } catch (e) {
      toast.error('Action failed', String(e));
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  async function copyInvite() {
    try {
      const inv = await api.invite();
      await navigator.clipboard.writeText(JSON.stringify(inv));
      setCopiedInvite(true);
      toast.success(
        'Pairing code copied to clipboard',
        'Share it with your client PC',
      );
      setTimeout(() => setCopiedInvite(false), 3000);
    } catch (e) {
      toast.error('Failed to copy invite', String(e));
    }
  }

  const notifications = useMemo<NotificationItem[]>(() => {
    const list: NotificationItem[] = [];
    if (rt.needs_logout) {
      list.push({
        id: 'logout',
        type: 'warn',
        title: 'Permission Required',
        desc: 'Log out and back in to grant /dev/uinput access.',
      });
    }
    if (rt.portal_error) {
      list.push({
        id: 'portal',
        type: 'warn',
        title: 'Wayland Portal Issue',
        desc: rt.portal_error,
      });
    }
    if (rt.error) {
      list.push({
        id: 'error',
        type: 'warn',
        title: 'Runtime Error',
        desc: rt.error,
      });
    }
    if (rt.service_ok) {
      list.push({
        id: 'service',
        type: 'info',
        title: 'Service Operational',
        desc: `TLS 1.3 listener on ${rt.listen}. Engine healthy.`,
      });
    }
    if (rt.clipboard_ok) {
      list.push({
        id: 'clip',
        type: 'info',
        title: 'Clipboard Sync',
        desc: 'Plaintext clipboard bridge is active.',
      });
    }
    return list;
  }, [rt]);

  if (!rt.role) {
    return <Onboarding busy={busy} runAction={runAction} />;
  }

  return (
    <div className="app-container">
      <Sidebar
        role={rt.role}
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        serviceOk={rt.service_ok}
        peerCount={peers.length}
        copiedInvite={copiedInvite}
        busy={busy}
        onCopyInvite={copyInvite}
      />

      <div className="main-wrapper">
        <TopBar
          serviceOk={rt.service_ok}
          listen={rt.listen}
          notifications={notifications}
          onRefresh={refresh}
        />

        <main className="content-canvas">
          {activeTab === 'dashboard' && (
            <DashboardView
              rt={rt}
              status={status}
              peers={peers}
              busy={busy}
              onCopyInvite={copyInvite}
              runAction={runAction}
              refresh={refresh}
            />
          )}

          {activeTab === 'peers' && (
            <PeersView
              role={rt.role}
              peers={peers}
              listen={rt.listen}
              advertise={rt.advertise}
              socketOk={rt.socket_ok}
              busy={busy}
              runAction={runAction}
            />
          )}

          {activeTab === 'logs' && (
            <LogsView
              logDir={rt.log_dir}
              serviceLog={rt.service_log}
              onRefresh={refresh}
            />
          )}

          {activeTab === 'settings' && (
            <SettingsView
              role={rt.role}
              status={status}
              runAction={runAction}
            />
          )}
        </main>
      </div>
    </div>
  );
}
