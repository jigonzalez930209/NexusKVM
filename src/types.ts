export type PeerStatus = 'Connected' | 'Disconnected' | 'Degraded';
export interface Peer {
  id: string;
  name: string;
  address: string;
  status: PeerStatus;
  latency_ms?: number;
}
export interface Status {
  state: { kind: string; peer?: string };
  active_target: string;
  peers: Record<string, Peer>;
  agent_connected: boolean;
  portal_available: boolean;
  emergency_shortcut: string;
}
export type Role = 'host' | 'client';
export interface Invite {
  server: string;
  password: string;
  certificate: string;
}
export interface RuntimeSnapshot {
  role: Role | null;
  running: boolean;
  socket_ok: boolean;
  service_ok: boolean;
  listen: string;
  advertise: string;
  remote_server: string | null;
  password: string;
  error: string | null;
  needs_logout: boolean;
  log_dir: string | null;
  service_log: string | null;
  daemon: Status | null;
  binary_host: string | null;
  binary_client: string | null;
  peer_side: string | null;
  portal_available: boolean;
  portal_error: string | null;
  clipboard_ok: boolean;
}

export type PeerSide = 'left' | 'right' | 'top' | 'bottom';
