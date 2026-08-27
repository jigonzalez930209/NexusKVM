import { useEffect, useMemo, useState } from 'react';
import { api, inTauri } from '../api';
import { formatLatency, formatUptime } from '../shared/format';
import { toast } from '../shared/toast';
import type { RuntimeSnapshot, Status } from '../types';

// Material Symbols subset ships without ligature tables; each icon is a PUA
// codepoint. Keep this map in sync with src/assets/fonts (pyftsubset list).
const MS = {
  widgets: '\ue1bd',
  dns: '\ue875',
  schedule: '\ue8b5',
  close: '\ue5cd',
  laptop_mac: '\ue320',
  desktop_windows: '\ue30c',
  content_copy: '\ue14d',
  check: '\ue5ca',
  swap_horiz: '\ue8d4',
  arrow_forward: '\ue5c8',
  link: '\ue157',
  fullscreen: '\ue5d0',
  tune: '\ue429',
  power_settings_new: '\ue8ac',
} as const;

type MsName = keyof typeof MS;

function MsIcon({
  name,
  className = '',
  filled = false,
}: {
  name: MsName;
  className?: string;
  filled?: boolean;
}) {
  return (
    <span
      aria-hidden
      className={`material-symbols-outlined ${className}`}
      style={filled ? { fontVariationSettings: "'FILL' 1" } : undefined}
    >
      {MS[name]}
    </span>
  );
}

export function TrayControlCenter() {
  const [rt, setRt] = useState<RuntimeSnapshot | null>(null);
  const [copied, setCopied] = useState(false);

  async function refresh() {
    if (!inTauri()) return;
    try {
      setRt(await api.runtime());
    } catch {
      /* ignore */
    }
  }

  useEffect(() => {
    if (inTauri()) {
      api.positionTrayPanel().catch(() => {});
    }
    refresh();
    const interval = setInterval(refresh, 1500);

    let unlistenTarget: (() => void) | undefined;
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
      if (unlistenStatus) unlistenStatus();
    };
  }, []);

  const status: Status | null = rt?.daemon ?? null;
  const peers = useMemo(
    () => (status ? Object.values(status.peers) : []),
    [status],
  );

  async function copyInvite() {
    try {
      const inv = await api.invite();
      await navigator.clipboard.writeText(JSON.stringify(inv));
      setCopied(true);
      toast.success('Pairing code copied to clipboard');
      setTimeout(() => setCopied(false), 2500);
    } catch (e) {
      toast.error('Failed to copy invite', String(e));
    }
  }

  async function toggleService() {
    if (!rt) return;
    try {
      if (rt.running) {
        await api.stop();
        toast.info('Service stopped');
      } else {
        await api.start();
        toast.success('Service started');
      }
    } catch (e) {
      toast.error('Service action failed', String(e));
    }
    await refresh();
  }

  function handleMouseDown(e: React.MouseEvent) {
    if (
      e.button === 0 &&
      !(e.target as HTMLElement).closest('button, input, a, textarea')
    ) {
      api.startDragging();
    }
  }

  const isClient = rt?.role === 'client';

  return (
    <div className="w-full h-full bg-surface border border-outline-variant flex flex-col font-body overflow-hidden select-none">
      {/* Header — native drag region. Left click & drag moves the window. */}
      <header
        data-tauri-drag-region
        onMouseDown={handleMouseDown}
        className="flex items-center justify-between px-4 py-3 border-b border-outline-variant bg-surface-container-low cursor-move shrink-0"
      >
        <div data-tauri-drag-region className="flex items-center gap-2.5">
          <MsIcon name="widgets" className="text-primary text-xl" filled />
          <h1
            data-tauri-drag-region
            className="font-headline font-bold text-on-surface tracking-tight text-base"
          >
            NexusKVM
          </h1>
        </div>
        <div className="flex items-center gap-2.5">
          <div
            data-tauri-drag-region
            className="flex items-center gap-2 px-2.5 py-1 rounded-full bg-surface-container-lowest border border-outline-variant"
          >
            <span
              className={`w-2 h-2 rounded-full shadow-[0_0_8px_rgba(52,211,153,0.6)] ${rt?.service_ok ? 'bg-tertiary animate-pulse' : 'bg-secondary'}`}
            ></span>
            <span className="text-[10px] text-on-surface-variant font-medium tracking-wide uppercase">
              {rt?.service_ok
                ? isClient
                  ? 'Client Active'
                  : 'Daemon Active'
                : 'Stopped'}
            </span>
          </div>
          <button
            onClick={() => api.hideTrayPanel()}
            aria-label="Close Panel"
            className="w-7 h-7 rounded-md text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high flex items-center justify-center transition-colors cursor-pointer"
          >
            <MsIcon name="close" className="text-base" />
          </button>
        </div>
      </header>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col gap-6 bg-surface px-4 py-5 overflow-y-auto min-h-0">
        {/* Status Card */}
        <div className="bg-surface-container rounded-xl p-5 border border-outline-variant flex flex-col gap-5 shrink-0">
          <div className="flex justify-between items-start">
            <div>
              <h2 className="text-[11px] text-on-surface-variant mb-2 font-medium uppercase tracking-wider">
                {isClient ? 'Ready to establish link' : 'System Status'}
              </h2>
              <div className="text-2xl font-bold text-on-surface tracking-tight leading-none">
                {isClient ? 'Client Mode' : 'Host Mode'}
              </div>
            </div>
            {/* Toggle Switch */}
            <label className="relative inline-flex items-center cursor-pointer mt-1">
              <input
                checked={!!rt?.running}
                onChange={toggleService}
                className="sr-only peer"
                type="checkbox"
              />
              <div className="w-9 h-5 bg-secondary-container peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-primary rounded-full peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-on-surface after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-primary"></div>
            </label>
          </div>
          <div className="flex items-center justify-between pt-4 border-t border-outline-variant">
            <div className="flex items-center gap-2.5 min-w-0">
              <MsIcon
                name={rt?.running ? 'schedule' : 'dns'}
                className="text-secondary text-base shrink-0"
              />
              <span className="text-xs text-on-surface-variant font-medium truncate">
                {rt?.running
                  ? `Uptime: ${formatUptime(rt?.metrics?.uptime_secs ?? 0)}`
                  : isClient
                    ? rt?.remote_server || 'Not linked'
                    : `Listening: ${rt?.listen || '—'}`}
              </span>
            </div>
            <button
              onClick={() => api.openMainWindow()}
              className="text-xs text-primary hover:text-primary-fixed transition-colors font-semibold flex items-center gap-1.5 group shrink-0 pl-2 cursor-pointer"
            >
              <span>Details</span>
              <MsIcon
                name="arrow_forward"
                className="text-sm group-hover:translate-x-0.5 transition-transform"
              />
            </button>
          </div>
        </div>

        {/* Pairing Code Section */}
        <div className="flex flex-col gap-3 shrink-0">
          <label className="text-[11px] font-medium text-on-surface-variant uppercase tracking-wider">
            {isClient ? 'Access Token' : 'Pairing Code'}
          </label>
          <div className="flex items-center gap-2.5">
            <div className="flex-1 min-w-0 bg-surface-container-lowest border border-outline-variant rounded-lg px-4 py-3 flex items-center justify-between font-mono text-on-surface text-sm tracking-widest relative overflow-hidden group">
              <span className="select-all truncate">
                {rt?.role === 'host'
                  ? rt?.password || '— not configured —'
                  : rt?.remote_server || '— no host linked —'}
              </span>
              <div className="absolute inset-0 bg-primary/5 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none"></div>
            </div>
            {rt?.role === 'host' && (
              <button
                onClick={copyInvite}
                aria-label="Copy Pairing Code"
                className="shrink-0 w-11 h-11 bg-secondary-container hover:bg-surface-container-high text-on-surface border border-outline-variant rounded-lg transition-colors active:scale-95 group focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-surface flex items-center justify-center cursor-pointer"
              >
                <MsIcon
                  name={copied ? 'check' : 'content_copy'}
                  className="text-base group-hover:text-primary transition-colors"
                />
              </button>
            )}
          </div>
        </div>

        {/* Peers List */}
        <div className="flex flex-col gap-3">
          <h3 className="text-[11px] font-medium text-on-surface-variant uppercase tracking-wider">
            {isClient ? 'Remote Host' : `Active Peers (${peers.length})`}
          </h3>
          <div className="flex flex-col gap-2.5">
            {peers.map((p) => (
              <div
                key={p.id}
                className="bg-surface-container-low border border-outline-variant rounded-md px-3 py-3 flex items-center justify-between hover:bg-surface-container-high transition-colors cursor-default group"
              >
                <div className="flex items-center gap-3 min-w-0">
                  <div className="w-9 h-9 shrink-0 rounded-md bg-secondary-container flex items-center justify-center">
                    <MsIcon
                      name="laptop_mac"
                      className="text-on-surface text-lg"
                    />
                  </div>
                  <div className="flex flex-col min-w-0">
                    <span className="text-sm font-medium text-on-surface leading-tight mb-1 truncate">
                      {p.name || p.id}
                    </span>
                    <span
                      className={`text-[10px] flex items-center gap-1.5 font-mono ${p.status === 'Connected' ? 'text-tertiary' : 'text-on-surface-variant'}`}
                    >
                      <span
                        className={`w-1.5 h-1.5 rounded-full inline-block shrink-0 ${p.status === 'Connected' ? 'bg-tertiary' : 'bg-secondary'}`}
                      ></span>
                      {p.status}
                    </span>
                  </div>
                </div>
                <div className="flex items-center gap-3 shrink-0 ml-3">
                  <span className="text-[10px] bg-surface-container-lowest border border-outline-variant text-on-surface-variant px-2 py-1 rounded font-mono">
                    {formatLatency(p.latency_ms)}
                  </span>
                  {!isClient && (
                    <button
                      onClick={() => api.switchTo(p.id)}
                      aria-label={`Switch to ${p.name || p.id}`}
                      className="w-7 h-7 rounded-md flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity text-on-surface-variant hover:text-primary hover:bg-surface-container-high cursor-pointer"
                    >
                      <MsIcon name="swap_horiz" className="text-base" />
                    </button>
                  )}
                </div>
              </div>
            ))}
            {peers.length === 0 && !isClient && (
              <div className="bg-surface-container-low border border-outline-variant rounded-lg px-4 py-6 flex items-center justify-center">
                <span className="text-xs text-on-surface-variant text-center leading-relaxed">
                  No external machines connected yet.
                </span>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Footer Actions */}
      <footer className="px-4 py-3.5 bg-surface-container-low border-t border-outline-variant flex gap-2.5 shrink-0">
        <button
          onClick={() => api.openMainWindow()}
          className="flex-1 bg-primary hover:bg-primary-fixed text-on-primary font-semibold text-xs py-2.5 px-4 rounded-lg transition-colors flex items-center justify-center gap-2 active:scale-[0.98] focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-surface cursor-pointer"
        >
          <MsIcon
            name={isClient ? 'link' : 'fullscreen'}
            className="text-base"
          />
          {isClient ? 'Connect to Host' : 'Maximize Interface'}
        </button>
        <button
          onClick={() => api.quitApp()}
          aria-label="Terminate Processes"
          className="w-11 bg-surface-container border border-outline-variant hover:border-error hover:text-error text-on-surface-variant rounded-lg transition-colors active:scale-[0.98] group focus:outline-none focus:ring-2 focus:ring-error focus:ring-offset-2 focus:ring-offset-surface flex items-center justify-center cursor-pointer"
        >
          <MsIcon
            name="power_settings_new"
            className="text-base group-hover:animate-pulse"
          />
        </button>
      </footer>
    </div>
  );
}
