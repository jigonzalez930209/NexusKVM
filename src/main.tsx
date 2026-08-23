import React, { useEffect, useState } from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWindow } from '@tauri-apps/api/window';
import App from './App';
import { TrayControlCenter } from './components/TrayControlCenter';
import { inTauri } from './api';
import { Toaster } from './components/ui/Toaster';
import './styles.css';

function Root() {
  const [label, setLabel] = useState<string>(() => {
    if (window.location.hash.includes('tray')) return 'tray-panel';
    if (inTauri()) {
      try {
        return getCurrentWindow().label;
      } catch {
        return 'main';
      }
    }
    return 'main';
  });

  useEffect(() => {
    if (inTauri()) {
      try {
        const current = getCurrentWindow().label;
        if (current && current !== label) {
          setLabel(current);
        }
      } catch {
        // Fallback for browser dev mode
      }
    }
  }, [label]);

  return (
    <>
      <Toaster />
      {label === 'tray-panel' ? <TrayControlCenter /> : <App />}
    </>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
