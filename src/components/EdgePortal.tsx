import { useEffect, useRef, useState } from 'react';
import { api, inTauri } from '../api';
import type { PeerSide } from '../types';

export function EdgePortal() {
  const [canSwitch, setCanSwitch] = useState(true);
  const [side, setSide] = useState<PeerSide>('right');

  const isArmedRef = useRef<boolean>(true);
  const activeTargetRef = useRef<string>('local');
  const lastTriggerRef = useRef<number>(0);
  const leaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    document.documentElement.classList.add('edge-portal-root');
    document.body.classList.add('edge-portal-root');
    document.documentElement.style.backgroundColor = 'transparent';
    document.body.style.backgroundColor = 'transparent';

    let unlistenSide: (() => void) | undefined;
    let unlistenTarget: (() => void) | undefined;

    if (inTauri()) {
      // Sync layout edge position
      api
        .getPeerSide()
        .then((s) => {
          const validSide: PeerSide = s === 'left' ? 'left' : 'right';
          setSide(validSide);
          api.positionEdgePortal(validSide).catch(() => {});
        })
        .catch(() => {});

      api
        .onPeerSideChanged((newSide) => {
          const validSide: PeerSide = newSide === 'left' ? 'left' : 'right';
          setSide(validSide);
          api.positionEdgePortal(validSide).catch(() => {});
        })
        .then((unlisten) => {
          unlistenSide = unlisten;
        })
        .catch(() => {});

      api
        .onTargetChanged((target) => {
          activeTargetRef.current = target;
          if (leaveTimerRef.current) {
            clearTimeout(leaveTimerRef.current);
            leaveTimerRef.current = null;
          }

          if (target !== 'local') {
            // While on remote PC: keep portal disarmed
            isArmedRef.current = false;
            setCanSwitch(false);
          } else {
            // Returned to local PC via key combination:
            // Mouse is resting over this portal window on PC1.
            // Disarm portal and schedule re-arm 200ms after mouse leaves the edge.
            isArmedRef.current = false;
            setCanSwitch(false);
            leaveTimerRef.current = setTimeout(() => {
              isArmedRef.current = true;
              setCanSwitch(true);
              leaveTimerRef.current = null;
            }, 200);
          }
        })
        .then((unlisten) => {
          unlistenTarget = unlisten;
        })
        .catch(() => {});
    }

    const onDocLeave = () => {
      scheduleRearm();
    };

    const onDocEnterOrMove = (e: MouseEvent | PointerEvent) => {
      handleTrigger(e as unknown as React.MouseEvent);
    };

    document.addEventListener('mouseleave', onDocLeave);
    document.addEventListener('pointerleave', onDocLeave);
    window.addEventListener('mouseleave', onDocLeave);
    window.addEventListener('pointerleave', onDocLeave);

    document.addEventListener('mouseenter', onDocEnterOrMove);
    document.addEventListener('pointerenter', onDocEnterOrMove);
    document.addEventListener('mousemove', onDocEnterOrMove);
    document.addEventListener('pointermove', onDocEnterOrMove);

    return () => {
      document.documentElement.classList.remove('edge-portal-root');
      document.body.classList.remove('edge-portal-root');
      document.removeEventListener('mouseleave', onDocLeave);
      document.removeEventListener('pointerleave', onDocLeave);
      window.removeEventListener('mouseleave', onDocLeave);
      window.removeEventListener('pointerleave', onDocLeave);
      document.removeEventListener('mouseenter', onDocEnterOrMove);
      document.removeEventListener('pointerenter', onDocEnterOrMove);
      document.removeEventListener('mousemove', onDocEnterOrMove);
      document.removeEventListener('pointermove', onDocEnterOrMove);
      if (unlistenSide) unlistenSide();
      if (unlistenTarget) unlistenTarget();
      if (leaveTimerRef.current) {
        clearTimeout(leaveTimerRef.current);
        leaveTimerRef.current = null;
      }
    };
  }, []);

  function scheduleRearm() {
    if (activeTargetRef.current !== 'local') return;
    if (leaveTimerRef.current) {
      clearTimeout(leaveTimerRef.current);
    }
    leaveTimerRef.current = setTimeout(() => {
      isArmedRef.current = true;
      setCanSwitch(true);
      leaveTimerRef.current = null;
    }, 200);
  }

  async function handleTrigger(e: React.MouseEvent | React.PointerEvent) {
    const now = Date.now();

    // If portal is disarmed (e.g. mouse just returned to local PC and is still over the portal),
    // do NOT switch to remote! Instead, keep resetting the 200ms timer so it only re-arms
    // 200ms after the mouse stops moving at the edge or leaves into the desktop.
    if (!isArmedRef.current) {
      scheduleRearm();
      return;
    }

    if (now - lastTriggerRef.current < 300) {
      return;
    }

    if (leaveTimerRef.current) {
      clearTimeout(leaveTimerRef.current);
      leaveTimerRef.current = null;
    }

    lastTriggerRef.current = now;
    // Immediately deactivate portal before switching
    isArmedRef.current = false;
    setCanSwitch(false);

    // Calculate normalized vertical edge position and dispatch transition
    const clientY = e.clientY;
    const height = window.innerHeight || 1;
    const normalized = Math.max(0.0, Math.min(1.0, clientY / height));

    if (inTauri()) {
      try {
        await api.switchEdge(normalized);
      } catch (err) {
        console.warn('[NexusKVM] Edge switch failed:', err);
        // Recovery: if switch failed, re-arm so user isn't stuck
        isArmedRef.current = true;
        setCanSwitch(true);
      }
    }
  }

  function handleMouseLeave() {
    scheduleRearm();
  }

  // Indicator border strip pinned to screen edge with vibrant blue styling
  const lineStyle: React.CSSProperties = {
    position: 'absolute',
    top: 0,
    bottom: 0,
    left: 0,
    right: 0,
    width: '100%',
    height: '100%',
    backgroundColor: '#3b82f6',
    pointerEvents: 'none',
    zIndex: 99999,
  };

  return (
    <div
      className="relative w-full h-full select-none cursor-default overflow-hidden pointer-events-auto"
      style={{
        width: '100vw',
        height: '100vh',
        backgroundColor: 'transparent',
        userSelect: 'none',
      }}
      onMouseEnter={handleTrigger}
      onMouseMove={handleTrigger}
      onPointerEnter={handleTrigger}
      onPointerMove={handleTrigger}
      onMouseLeave={handleMouseLeave}
      onPointerLeave={handleMouseLeave}
    >
      <div style={lineStyle} />
    </div>
  );
}
