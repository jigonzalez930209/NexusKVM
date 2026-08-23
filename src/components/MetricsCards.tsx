import { Activity, Shield, Users, Copy } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { formatLatency } from '../shared/format';
import type { Peer, Role, ServiceMetrics } from '../types';

interface EngineMetricsCardProps {
  serviceOk: boolean;
  metrics: ServiceMetrics;
}

function SparkBars({ values, alt }: { values: number[]; alt?: boolean }) {
  return (
    <div className="sparkline-container">
      {values.map((v, i) => (
        <div
          key={i}
          className={`spark-bar${alt ? ' alt' : ''}`}
          style={{ height: `${Math.max(6, Math.min(100, v))}%` }}
        />
      ))}
    </div>
  );
}

export function EngineMetricsCard({
  serviceOk,
  metrics,
}: EngineMetricsCardProps) {
  const [cpuHist, setCpuHist] = useState<number[]>([4, 6, 8, 5]);
  const [memHist, setMemHist] = useState<number[]>([8, 12, 16, 14]);

  const cpu = serviceOk ? metrics.cpu_percent : 0;
  const memMb = serviceOk ? metrics.mem_mb : 0;

  useEffect(() => {
    if (!serviceOk) {
      setCpuHist([0, 0, 0, 0]);
      setMemHist([0, 0, 0, 0]);
      return;
    }
    setCpuHist((prev) => [...prev.slice(-6), Math.max(cpu, 4)]);
    setMemHist((prev) => [...prev.slice(-6), Math.max((memMb / 256) * 100, 6)]);
  }, [serviceOk, cpu, memMb]);

  return (
    <div className="obsidian-card" style={{ gridColumn: 'span 4' }}>
      <div className="card-header">
        <span className="card-title">
          <Activity size={14} /> Engine Status
        </span>
        <span className={`tag-badge ${serviceOk ? 'success' : ''}`}>
          {serviceOk ? 'HEALTHY' : 'OFFLINE'}
        </span>
      </div>

      <div className="metrics-row">
        <div className="metric-box">
          <div className="metric-label">
            <span>CPU</span>
            <span>
              {serviceOk ? `${metrics.cpu_percent.toFixed(1)}%` : '0%'}
            </span>
          </div>
          <div className="progress-bar-bg">
            <div
              className="progress-bar-fill"
              style={{
                width: `${Math.min(100, metrics.cpu_percent)}%`,
                background: 'var(--primary)',
              }}
            />
          </div>
          <SparkBars values={serviceOk ? cpuHist : []} />
        </div>

        <div className="metric-box">
          <div className="metric-label">
            <span>MEM</span>
            <span>{serviceOk ? `${memMb.toFixed(1)} MB` : '0 MB'}</span>
          </div>
          <div className="progress-bar-bg">
            <div
              className="progress-bar-fill"
              style={{
                width: `${Math.min(100, (memMb / 256) * 100)}%`,
                background: 'var(--success)',
              }}
            />
          </div>
          <SparkBars values={serviceOk ? memHist : []} alt />
        </div>
      </div>

      <div className="card-footer-info">
        <span>TLS 1.3 Active</span>
        <span>PID {metrics.pid ?? '—'}</span>
      </div>
    </div>
  );
}

interface AccessTokenCardProps {
  role: Role | null;
  password: string;
  remoteServer: string | null;
  onCopyInvite: () => void;
}

export function AccessTokenCard({
  role,
  password,
  remoteServer,
  onCopyInvite,
}: AccessTokenCardProps) {
  const [tokenRevealed, setTokenRevealed] = useState(false);

  return (
    <div className="obsidian-card" style={{ gridColumn: 'span 3' }}>
      <div className="card-header">
        <span className="card-title">
          <Shield size={14} /> {role === 'host' ? 'Access Token' : 'Host Link'}
        </span>
      </div>

      <div
        className="token-container"
        onClick={() => setTokenRevealed(!tokenRevealed)}
      >
        <div className={`token-code ${!tokenRevealed ? 'blurred' : ''}`}>
          {role === 'host'
            ? password
              ? password.slice(0, 8) + '...'
              : '••••••••'
            : remoteServer || '— no host linked —'}
        </div>
        {role === 'host' && (
          <button
            className="token-copy-btn"
            onClick={(e) => {
              e.stopPropagation();
              onCopyInvite();
            }}
            title="Copy full invite"
          >
            <Copy size={13} />
          </button>
        )}
        <span className="token-hint">
          {tokenRevealed ? 'Click to conceal' : 'Hover or click to reveal'}
        </span>
      </div>
    </div>
  );
}

interface ActiveNodesCardProps {
  advertise: string;
  listen: string;
  peers: Peer[];
  peerSide: string | null;
}

export function ActiveNodesCard({
  advertise,
  listen,
  peers,
  peerSide,
}: ActiveNodesCardProps) {
  return (
    <div className="obsidian-card" style={{ gridColumn: 'span 5' }}>
      <div className="card-header">
        <span className="card-title">
          <Users size={14} /> Active Nodes
        </span>
        <span className="tag-badge">{peers.length + 1} connected</span>
      </div>

      <ul className="peers-list-compact">
        <li className="peer-row-item">
          <div className="peer-info-left">
            <div className="peer-dot" />
            <div className="peer-details">
              <span className="peer-name">This Device (Local)</span>
              <span className="peer-sub">{advertise || listen} • Host</span>
            </div>
          </div>
          <div className="peer-metrics-right">
            <span className="peer-latency">0ms</span>
            <span className="tag-badge success" style={{ fontSize: '0.6rem' }}>
              ACTIVE
            </span>
          </div>
        </li>

        {peers.map((p) => (
          <li key={p.id} className="peer-row-item">
            <div className="peer-info-left">
              <div className="peer-dot" />
              <div className="peer-details">
                <span className="peer-name">{p.name || p.id}</span>
                <span className="peer-sub">{p.address} • Remote</span>
              </div>
            </div>
            <div className="peer-metrics-right">
              <span className="peer-latency">
                {formatLatency(p.latency_ms)}
              </span>
              <span className="tag-badge" style={{ fontSize: '0.6rem' }}>
                {peerSide?.toUpperCase()}
              </span>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
