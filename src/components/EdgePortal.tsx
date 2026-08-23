import { useEffect, useRef, useState } from 'react';
import { api, inTauri } from '../api';

export function EdgePortal() {
  const [isArmed, setIsArmed] = useState(true);
  const lastTriggerRef = useRef<number>(0);

  useEffect(() => {
    document.documentElement.classList.add('transparent-window');
    document.body.classList.add('transparent-window');
    document.documentElement.style.backgroundColor = 'transparent';
    document.body.style.backgroundColor = 'transparent';

    if (inTauri()) {
      api.positionEdgePortal().catch(() => {});
    }

    return () => {
      document.documentElement.classList.remove('transparent-window');
      document.body.classList.remove('transparent-window');
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

    // Determine normalized position along the dominant axis
    let normalized = 0.5;
    if (window.innerHeight > window.innerWidth) {
      // Vertical strip (Left/Right edge): normalized Y
      normalized = Math.max(
        0.0,
        Math.min(1.0, e.clientY / (window.innerHeight || 1)),
      );
    } else {
      // Horizontal strip (Top/Bottom edge): normalized X
      normalized = Math.max(
        0.0,
        Math.min(1.0, e.clientX / (window.innerWidth || 1)),
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
    // When the cursor leaves the 2px strip, re-arm the sensor
    setIsArmed(true);
  }

  return (
    <div
      className="w-full h-full bg-transparent select-none cursor-default overflow-hidden pointer-events-auto"
      style={{
        width: '100vw',
        height: '100vh',
        backgroundColor: 'transparent',
        userSelect: 'none',
      }}
      onMouseEnter={handleTrigger}
      onMouseMove={handleTrigger}
      onMouseLeave={handleMouseLeave}
    />
  );
}
