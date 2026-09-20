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
  /** True while the pointer is physically over the edge strip. */
  const pointerInsideRef = useRef<boolean>(false);

  useEffect(() => {
    document.documentElement.classList.add('edge-portal-root');
    document.body.classList.add('edge-portal-root');
    document.documentElement.style.backgroundColor = 'transparent';
    document.body.style.backgroundColor = 'transparent';

    let unlistenSide: (() => void) | undefined;
    let unlistenTarget: (() => void) | undefined;
    let cancelled = false;

    const track = (p: Promise<() => void>, assign: (u: () => void) => void) => {
      p.then((u) => {
        if (cancelled) u();
        else assign(u);
      }).catch(() => {});
    };

    if (inTauri()) {
      // Seed the authoritative ownership state before arming: if the app
      // starts (or reloads) while another machine owns control, the portal
      // must stay disarmed until `local` is reported again.
      api
        .status()
        .then((st) => {
          const target = st.active_target || 'local';
          activeTargetRef.current = target;
          const armed = target === 'local';
          isArmedRef.current = armed;
          setCanSwitch(armed);
        })
        .catch(() => {});

      // Sync layout edge position
      api
        .getPeerSide()
        .then((s) => {
          const valid: PeerSide[] = ['left', 'right', 'top', 'bottom'];
          const validSide: PeerSide = valid.includes(s as PeerSide)
            ? (s as PeerSide)
            : 'right';
          setSide(validSide);
          api.positionEdgePortal(validSide).catch(() => {});
        })
        .catch(() => {});

      track(
        api.onPeerSideChanged((newSide) => {
          const valid: PeerSide[] = ['left', 'right', 'top', 'bottom'];
          const validSide: PeerSide = valid.includes(newSide as PeerSide)
            ? (newSide as PeerSide)
            : 'right';
          setSide(validSide);
          api.positionEdgePortal(validSide).catch(() => {});
        }),
        (u) => {
          unlistenSide = u;
        },
      );

      track(
        api.onTargetChanged((target) => {
          activeTargetRef.current = target;
          if (leaveTimerRef.current) {
            clearTimeout(leaveTimerRef.current);
            leaveTimerRef.current = null;
          }

          isArmedRef.current = false;
          setCanSwitch(false);
          // Returning to local while the cursor is already off the strip: re-arm
          // only if the pointer is not sitting on the edge, otherwise wait for
          // mouseleave (the server pushes the local cursor inwards on return).
          if (target === 'local' && !pointerInsideRef.current) {
            leaveTimerRef.current = setTimeout(() => {
              if (
                activeTargetRef.current === 'local' &&
                !pointerInsideRef.current
              ) {
                isArmedRef.current = true;
                setCanSwitch(true);
              }
              leaveTimerRef.current = null;
            }, 200);
          }
        }),
        (u) => {
          unlistenTarget = u;
        },
      );
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
      cancelled = true;
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
    pointerInsideRef.current = false;
    if (activeTargetRef.current !== 'local') return;
    if (leaveTimerRef.current) {
      clearTimeout(leaveTimerRef.current);
    }
    leaveTimerRef.current = setTimeout(() => {
      // A re-entry during the delay cancels the re-arm: the portal must never
      // arm under a pointer that is already on the strip.
      if (!pointerInsideRef.current) {
        isArmedRef.current = true;
        setCanSwitch(true);
      }
      leaveTimerRef.current = null;
    }, 200);
  }

  async function handleTrigger(e: React.MouseEvent | React.PointerEvent) {
    const now = Date.now();
    pointerInsideRef.current = true;

    // Hard ownership gate: while another machine owns control, an edge event
    // must never be forwarded. Covers missed `target-changed` events (stale
    // ref) on top of the daemon-side containment window.
    if (activeTargetRef.current !== 'local') {
      return;
    }

    // Disarmed (e.g. control just returned and the cursor is still parked on
    // the strip): cancel any pending re-arm so it cannot fire under the
    // pointer, and stay disarmed until the pointer leaves and comes back.
    if (!isArmedRef.current) {
      if (leaveTimerRef.current) {
        clearTimeout(leaveTimerRef.current);
        leaveTimerRef.current = null;
      }
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
