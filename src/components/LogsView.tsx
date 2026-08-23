import { Terminal, FolderOpen, RefreshCw } from 'lucide-react';
import { api } from '../api';
import { toast } from '../shared/toast';

interface LogsViewProps {
  logDir: string | null;
  serviceLog: string | null;
  onRefresh: () => void;
}

export function LogsView({ logDir, serviceLog, onRefresh }: LogsViewProps) {
  return (
    <div className="logs-view-container">
      <div className="logs-toolbar">
        <span className="card-title">
          <Terminal size={14} /> Service Logs & Trace
        </span>
        <div style={{ display: 'flex', gap: '0.5rem' }}>
          {logDir && (
            <button
              className="btn-secondary"
              style={{ fontSize: '0.72rem', padding: '0.3rem 0.6rem' }}
              onClick={() =>
                api
                  .openLogs()
                  .then((p) => toast.info('Logs folder opened', p))
                  .catch((e) => toast.error('Could not open logs', String(e)))
              }
            >
              <FolderOpen size={13} /> Open Folder
            </button>
          )}
          <button
            className="btn-secondary"
            style={{ fontSize: '0.72rem', padding: '0.3rem 0.6rem' }}
            onClick={onRefresh}
          >
            <RefreshCw size={13} /> Refresh
          </button>
        </div>
      </div>
      <pre className="logs-terminal-output">
        {serviceLog ||
          'No log output yet. Daemon is idle or writing to systemd journal.'}
      </pre>
    </div>
  );
}
