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
  startDragging: () => invoke<void>('start_dragging'),
  minimizeWindow: () => invoke<void>('minimize_window'),
  toggleMaximize: () => invoke<void>('toggle_maximize'),
  hideWindow: () => invoke<void>('hide_window'),
  openMainWindow: () => invoke<void>('open_main_window'),
  showMainWindow: () => invoke<void>('open_main_window'),
  positionTrayPanel: () => invoke<void>('position_tray_panel'),
  toggleTrayPanel: () => invoke<void>('toggle_tray_panel'),
  toggleTrayWindow: () => invoke<void>('toggle_tray_panel'),
  hideTrayPanel: () => invoke<void>('hide_tray_panel'),
  hideTrayWindow: () => invoke<void>('hide_tray_panel'),
  switchEdge: (normalizedPosition: number) =>
    invoke<Status>('switch_edge', { normalizedPosition }),
  positionEdgePortal: (side?: string) =>
    invoke<void>('position_edge_portal_cmd', { side }),
  showEdgePortal: () => invoke<void>('show_edge_portal_cmd'),
  hideEdgePortal: () => invoke<void>('hide_edge_portal_cmd'),
  toggleEdgePortal: (enable: boolean) =>
    invoke<void>('toggle_edge_portal', { enable }),
  quitApp: () => invoke<void>('quit_app_cmd'),
};
