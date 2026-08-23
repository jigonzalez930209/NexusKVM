export function formatUptime(secs: number): string {
  if (secs === undefined || secs === null || secs < 0 || isNaN(secs))
    return '—';
  if (secs < 60) return `${Math.floor(secs)}s`;
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function formatLatency(ms?: number | null): string {
  return ms != null && ms >= 0 ? `${ms}ms` : '—';
}
