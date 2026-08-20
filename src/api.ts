import { invoke } from '@tauri-apps/api/core';
import type { Invite, RuntimeSnapshot, Status } from './types';

export function inTauri() {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export const api = {
  runtime: () => invoke<RuntimeSnapshot>('runtime_status'),
  setupHost: () => invoke<RuntimeSnapshot>('setup_as_host'),
  setupClient: (invite: Invite) =>
    invoke<RuntimeSnapshot>('setup_as_client', { invite }),
  start: () => invoke<RuntimeSnapshot>('start_runtime'),
  stop: () => invoke<RuntimeSnapshot>('stop_runtime'),
  reset: () => invoke<RuntimeSnapshot>('reset_runtime'),
  invite: () => invoke<Invite>('pairing_invite'),
  status: () => invoke<Status>('daemon_status'),
  switchTo: (target: string) => invoke<Status>('switch_target', { target }),
  local: () => invoke<Status>('switch_local'),
  releaseAll: () => invoke<void>('release_all'),
  openLogs: () => invoke<string>('open_logs'),
  setPeerSide: (side: string) => invoke<string>('set_peer_side', { side }),
  getPeerSide: () => invoke<string>('get_peer_side'),
};
