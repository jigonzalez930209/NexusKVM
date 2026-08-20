import { useEffect, useMemo, useState } from 'react';
import {
  Activity,
  ArrowLeftRight,
  Copy,
  Monitor,
  MousePointer2,
  Plug,
  Power,
  RefreshCcw,
  ShieldCheck,
  Wifi,
} from 'lucide-react';
import { api, inTauri } from './api';
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
};

export default function App() {
  const [rt, setRt] = useState<RuntimeSnapshot>(emptyRuntime);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState('');
  const [paste, setPaste] = useState('');
  const status: Status | null = rt.daemon;
  const peers = useMemo(
    () => (status ? Object.values(status.peers) : []),
    [status],
  );

  async function refresh() {
    if (!inTauri()) {
      return;
    }
    try {
      setRt(await api.runtime());
    } catch (e) {
      setMsg(String(e));
    }
  }

  useEffect(() => {
    refresh();
    const i = setInterval(refresh, 2000);
    return () => clearInterval(i);
  }, []);

  async function run(fn: () => Promise<RuntimeSnapshot | Status | void>) {
    setBusy(true);
    setMsg('');
    try {
      const out = await fn();
      if (out && typeof out === 'object' && 'role' in out) {
        setRt(out as RuntimeSnapshot);
      } else {
        await refresh();
      }
    } catch (e) {
      setMsg(String(e));
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  async function copyInvite() {
    const inv = await api.invite();
    await navigator.clipboard.writeText(JSON.stringify(inv));
    setMsg('Pairing code copied. Paste it on the other machine.');
  }

  if (!rt.role) {
    return (
      <main className="setup">
        <header>
          <div className="brand">
            <div className="mark">
              <MousePointer2 />
            </div>
            <div>
              <h1>NexusKVM</h1>
              <span>One keyboard, two machines</span>
            </div>
          </div>
        </header>
        <section className="hero">
          <div>
            <p className="eyebrow">FIRST LAUNCH</p>
            <h2>
              Choose the role of
              <br />
              <em>this machine.</em>
            </h2>
            <p>
              The app generates certificates, saves configuration, and starts
              the service. No terminal required.
            </p>
          </div>
        </section>
        <section className="setupGrid">
          <button
            className="setupCard"
            disabled={busy}
            onClick={() => run(() => api.setupHost())}
          >
            <Monitor />
            <b>This is the host</b>
            <span>
              Has the keyboard and mouse. The other machine connects here.
            </span>
          </button>
          <div className="setupCard clientCard">
            <Plug />
            <b>Connect to another</b>
            <span>Paste the code you copied from the host machine.</span>
            <textarea
              value={paste}
              onChange={(e) => setPaste(e.target.value)}
              placeholder='{"server":"...","password":"...","certificate":"..."}'
            />
            <button
              className="primary"
              disabled={busy || !paste.trim()}
              onClick={() => {
                try {
                  const inv = JSON.parse(paste) as {
                    server: string;
                    password: string;
                    certificate: string;
                  };
                  run(() => api.setupClient(inv));
                } catch {
                  setMsg('The code is not valid JSON');
                }
              }}
            >
              Connect
            </button>
          </div>
        </section>
        {msg && <p className="banner">{msg}</p>}
        {rt.needs_logout && (
          <p className="banner">
            Log out and back in after installing NexusKVM (/dev/uinput
            permission).
          </p>
        )}
      </main>
    );
  }

  const active = status?.active_target ?? 'local';
  const remote = peers[0];

  return (
    <main>
      <header>
        <div className="brand">
          <div className="mark">
            <MousePointer2 />
          </div>
          <div>
            <h1>NexusKVM</h1>
            <span>{rt.role === 'host' ? 'Host machine' : 'Client machine'}</span>
          </div>
        </div>
        <div className={'health ' + (rt.service_ok ? 'on' : 'off')}>
          <i /> {rt.service_ok ? 'Service running' : 'Stopped'}
        </div>
      </header>
      {(rt.error || msg) && <p className="banner">{rt.error || msg}</p>}
      {rt.needs_logout && (
        <p className="banner">
          NexusKVM is installed but this session cannot open /dev/uinput. Log
          out and back in (or reboot), then press Start.
        </p>
      )}
      <section className="hero">
        <div>
          <p className="eyebrow">
            {rt.role === 'host' ? 'SHARE THIS MACHINE' : 'REMOTE CONTROL'}
          </p>
          <h2>
            {rt.role === 'host' ? (
              <>
                Ready to
                <br />
                <em>accept the other PC.</em>
              </>
            ) : (
              <>
                Connected
                <br />
                <em>to the host.</em>
              </>
            )}
          </h2>
          <p>
            {rt.role === 'host'
              ? 'Copy the pairing code and paste it into NexusKVM on the second machine.'
              : 'The host keyboard and mouse can control this machine.'}
          </p>
        </div>
        <div className="heroActions">
          {rt.role === 'host' && (
            <button className="primary" onClick={copyInvite} disabled={busy}>
              <Copy /> Copy pairing code
            </button>
          )}
          {rt.role === 'host' && remote && (
            <button
              className="primary"
              disabled={busy || !rt.socket_ok}
              onClick={() =>
                run(() =>
                  active === 'local' ? api.switchTo(remote.id) : api.local(),
                )
              }
            >
              <ArrowLeftRight />{' '}
              {active === 'local' ? 'Control other machine' : 'Return here'}
            </button>
          )}
        </div>
      </section>
      <section className="grid">
        <article className="card pad">
          <div className="cardTitle">
            <span>
              <Wifi /> Connection
            </span>
            <button onClick={refresh}>
              <RefreshCcw size={16} />
            </button>
          </div>
          <dl className="meta">
            <div>
              <dt>{rt.role === 'client' ? 'Host' : 'Address'}</dt>
              <dd>
                {rt.role === 'client'
                  ? rt.remote_server || '—'
                  : rt.advertise || rt.listen}
              </dd>
            </div>
            {rt.role === 'host' && (
              <div>
                <dt>Password</dt>
                <dd>
                  <kbd>{rt.password || '—'}</kbd>
                </dd>
              </div>
            )}
            <div>
              <dt>{rt.role === 'client' ? 'rkvm client' : 'IPC control'}</dt>
              <dd>
                {rt.role === 'client'
                  ? rt.service_ok
                    ? 'Connected'
                    : 'Stopped'
                  : rt.socket_ok
                    ? 'OK'
                    : 'no daemon'}
              </dd>
            </div>
            {rt.log_dir && (
              <div>
                <dt>Logs</dt>
                <dd>
                  <button
                    type="button"
                    className="linkish"
                    onClick={() =>
                      api.openLogs().then((p) => setMsg(`Logs: ${p}`))
                    }
                  >
                    Open folder
                  </button>
                </dd>
              </div>
            )}
          </dl>
        </article>
        <aside className="stack">
          <article className="card">
            <div className="cardTitle">
              <span>
                <Activity /> Status
              </span>
              <span className={'badge ' + (rt.service_ok ? '' : 'warn')}>
                {rt.service_ok ? 'LIVE' : 'OFF'}
              </span>
            </div>
            <ul className="checks">
              <li>
                <ShieldCheck />
                {rt.role === 'client' ? 'Client' : 'Daemon'}{' '}
                <b>{rt.service_ok ? 'Active' : 'No'}</b>
              </li>
              <li>
                <Plug />
                Binary{' '}
                <b>
                  {rt.role === 'host'
                    ? rt.binary_host
                      ? 'Ready'
                      : 'Missing'
                    : rt.binary_client
                      ? 'Ready'
                      : 'Missing'}
                </b>
              </li>
              <li>
                <MousePointer2 />
                Portal{' '}
                <b>
                  {rt.portal_error
                    ? 'Error'
                    : rt.portal_available || status?.portal_available
                      ? 'Available'
                      : 'Pending'}
                </b>
              </li>
              <li>
                <ArrowLeftRight />
                Clipboard <b>{rt.clipboard_ok ? 'Active' : 'Off'}</b>
              </li>
            </ul>
            {rt.portal_error && <p className="hint">{rt.portal_error}</p>}
            <div className="peerSide">
              <span>The other PC is on the</span>
              <div className="sideRow">
                {(
                  [
                    ['left', 'Left'],
                    ['right', 'Right'],
                    ['top', 'Top'],
                    ['bottom', 'Bottom'],
                  ] as const
                ).map(([id, label]) => (
                  <button
                    key={id}
                    type="button"
                    className={
                      (rt.peer_side ?? 'right') === id ? 'side active' : 'side'
                    }
                    disabled={busy}
                    onClick={() =>
                      run(async () => {
                        await api.setPeerSide(id);
                        await refresh();
                      })
                    }
                  >
                    {label}
                  </button>
                ))}
              </div>
              <p className="hint">
                Fallback hotkey:{' '}
                <kbd>{status?.emergency_shortcut ?? 'Left Alt + Left Ctrl'}</kbd>
              </p>
            </div>
          </article>
          <article className="card emergency">
            <Power />
            <div>
              <span>Service</span>
              <kbd>{status?.emergency_shortcut ?? 'Left Alt + Left Ctrl'}</kbd>
            </div>
            {rt.running ? (
              <button onClick={() => run(() => api.stop())}>Stop</button>
            ) : (
              <button onClick={() => run(() => api.start())}>Start</button>
            )}
          </article>
        </aside>
      </section>
      {rt.service_log && (
        <section className="logPanel">
          <div className="sectionTitle">
            <h3>Service log</h3>
            <span>{rt.log_dir}</span>
          </div>
          <pre>{rt.service_log}</pre>
        </section>
      )}
      <section className="devices">
        <div className="sectionTitle">
          <h3>Machines</h3>
          <span>{peers.length + 1} nodes</span>
        </div>
        <div className="deviceRow">
          <div className="device active">
            <Monitor />
            <div>
              <b>This machine</b>
              <span>{active === 'local' ? 'Control here' : 'Waiting'}</span>
            </div>
            <mark>LOCAL</mark>
          </div>
          {peers.map((p) => (
            <div className="device" key={p.id}>
              <Monitor />
              <div>
                <b>{p.name}</b>
                <span>
                  {p.address} · {p.status}
                </span>
              </div>
              {rt.role === 'host' && (
                <button
                  onClick={() => run(() => api.switchTo(p.id))}
                  disabled={!rt.socket_ok}
                >
                  Control
                </button>
              )}
            </div>
          ))}
          {peers.length === 0 && (
            <div className="device">
              <Monitor />
              <div>
                <b>No peer yet</b>
                <span>
                  {rt.role === 'host'
                    ? 'Open NexusKVM on the other PC and paste the code.'
                    : rt.service_ok
                      ? 'The peer shows up on the host PC, not here.'
                      : 'Start the service; check the log if it fails.'}
                </span>
              </div>
            </div>
          )}
        </div>
      </section>
      <footer>
        <span>NexusKVM 0.1.0</span>
        <span>
          {rt.role === 'host' && (
            <button onClick={() => api.releaseAll()}>Release keys</button>
          )}
          <button onClick={() => run(() => api.reset())}>Change role</button>
        </span>
      </footer>
    </main>
  );
}
