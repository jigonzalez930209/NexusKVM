import { useEffect, useRef, useState } from 'react';
import { api, inTauri } from '../api';

export function EdgePortal() {
  const [isArmed, setIsArmed] = useState(true);
  const [side, setSide] = useState<string>('right');
  const lastTriggerRef = useRef<number>(0);

  useEffect(() => {
    document.documentElement.classList.add('edge-portal-root');
    document.body.classList.add('edge-portal-root');
    document.documentElement.style.backgroundColor = 'transparent';
    document.body.style.backgroundColor = 'transparent';

    if (inTauri()) {
      api
        .getPeerSide()
        .then((s) => setSide(s))
        .catch(() => {});
      api.positionEdgePortal().catch(() => {});
    }

    return () => {
      document.documentElement.classList.remove('edge-portal-root');
      document.body.classList.remove('edge-portal-root');
    };
  }, []);

  async function handleTrigger(e: React.MouseEvent) {
    const now = Date.now();
    // Guard against rapid re-triggers (minimum 600ms cooldown)
    if (!isArmed || now - lastTriggerRef.current < 600) {
      return;
    }

    lastTriggerRef.current = now;
    setIsArmed(false);

    // Determine normalized position along the active axis
    let normalized = 0.5;
    if (side === 'top' || side === 'bottom') {
      normalized = Math.max(
        0.0,
        Math.min(1.0, e.clientX / (window.innerWidth || 1)),
      );
    } else {
      normalized = Math.max(
        0.0,
        Math.min(1.0, e.clientY / (window.innerHeight || 1)),
      );
    }

    if (inTauri()) {
      try {
        await api.switchEdge(normalized);
      } catch (err) {
        console.warn('Edge switch failed:', err);
      }
    }
  }

  function handleMouseLeave() {
    // When the cursor leaves the strip, re-arm the sensor
    setIsArmed(true);
  }

  // Exact 1-pixel red indicator strip pinned to the screen border
  const lineStyle: React.CSSProperties = {
    position: 'absolute',
    backgroundColor: 'blue',
    pointerEvents: 'none',
    zIndex: 99999,
    ...(side === 'left'
      ? { left: 0, top: 0, width: '1px', height: '100%' }
      : side === 'top'
        ? { left: 0, top: 0, width: '100%', height: '1px' }
        : side === 'bottom'
          ? { left: 0, bottom: 0, width: '100%', height: '1px' }
          : { right: 0, top: 0, width: '1px', height: '100%' }),
  };

  return (
    <div
      className="relative w-full h-full select-none cursor-default overflow-hidden pointer-events-auto"
      style={{
        width: '100vw',
        height: '100vh',
        backgroundColor: 'red',
        userSelect: 'none',
      }}
      onMouseEnter={handleTrigger}
      onMouseMove={handleTrigger}
      onMouseLeave={handleMouseLeave}
    >
      <div style={lineStyle} />
    </div>
  );
}
