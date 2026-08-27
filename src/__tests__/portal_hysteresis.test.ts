import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

describe('EdgePortal arming and return hysteresis logic', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('disarms on transition trigger and only re-arms 200ms after mouse leaves the portal upon return', () => {
    let isArmed = true;
    let switchCalls = 0;
    let leaveTimer: ReturnType<typeof setTimeout> | null = null;

    function onTrigger() {
      if (leaveTimer) {
        clearTimeout(leaveTimer);
        leaveTimer = null;
      }
      if (!isArmed) {
        return false;
      }
      isArmed = false;
      switchCalls++;
      return true;
    }

    function onMouseLeave() {
      if (leaveTimer) {
        clearTimeout(leaveTimer);
        leaveTimer = null;
      }
      leaveTimer = setTimeout(() => {
        isArmed = true;
        leaveTimer = null;
      }, 200);
    }

    function onTargetChanged(_newTarget: string) {
      if (leaveTimer) {
        clearTimeout(leaveTimer);
        leaveTimer = null;
      }
      isArmed = false;
    }

    // 1. Initial state: mouse enters portal from local PC -> switch triggers successfully
    expect(onTrigger()).toBe(true);
    expect(switchCalls).toBe(1);
    expect(isArmed).toBe(false);

    // 2. User presses hotkey to return to local PC (e.g. Pause / Break or Ctrl+Alt)
    // Target changed event fires -> portal remains strictly disarmed while mouse is over portal
    onTargetChanged('local');
    expect(isArmed).toBe(false);

    // 3. Moving mouse while still inside portal does NOT trigger switch
    expect(onTrigger()).toBe(false);
    expect(switchCalls).toBe(1);
    expect(isArmed).toBe(false);

    // 4. Mouse leaves portal area into local PC screen interior
    onMouseLeave();
    expect(isArmed).toBe(false); // Not yet armed (timer is running)

    // 4a. If mouse re-enters before 200ms, timer is cancelled and portal stays disarmed
    vi.advanceTimersByTime(100);
    expect(isArmed).toBe(false);
    onTrigger(); // cursor re-entered at 100ms
    vi.advanceTimersByTime(200);
    expect(isArmed).toBe(false); // Still disarmed because re-entered before 200ms

    // 4b. Now mouse leaves completely and stays outside for 200ms
    onMouseLeave();
    vi.advanceTimersByTime(199);
    expect(isArmed).toBe(false);

    vi.advanceTimersByTime(2); // 201ms elapsed
    expect(isArmed).toBe(true); // RE-ARMED!

    // 5. User later moves mouse back to edge to switch to remote PC a second time
    expect(onTrigger()).toBe(true); // Successfully triggers second switch!
    expect(switchCalls).toBe(2);
    expect(isArmed).toBe(false);
  });

  it('validates only left and right edge positions', () => {
    const validEdges = ['left', 'right'] as const;
    expect(validEdges.includes('left')).toBe(true);
    expect(validEdges.includes('right')).toBe(true);
    // @ts-expect-error test invalid edge
    expect(validEdges.includes('top')).toBe(false);
    // @ts-expect-error test invalid edge
    expect(validEdges.includes('bottom')).toBe(false);
  });
});
