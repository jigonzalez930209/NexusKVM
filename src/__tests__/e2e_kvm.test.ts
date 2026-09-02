import { describe, it, expect } from 'vitest';

type Role = 'host' | 'client';
type Edge = 'left' | 'right' | 'top' | 'bottom';

function opposite(edge: Edge): Edge {
  switch (edge) {
    case 'left':
      return 'right';
    case 'right':
      return 'left';
    case 'top':
      return 'bottom';
    case 'bottom':
      return 'top';
  }
}

function entryFor(exitEdge: Edge, normalized: number) {
  return {
    edge: opposite(exitEdge),
    normalized_position: Math.max(0, Math.min(1, normalized)),
    inset_px: 6,
  };
}

function switchPlan(role: Role, peerSide: Edge, connectedPeer?: string) {
  if (role === 'client') {
    return { action: 'switch_local_to_host' as const, channel: ':5259' };
  }
  return {
    action: 'switch_to_peer' as const,
    target: connectedPeer ?? 'peer',
    entry: entryFor(peerSide, 0.5),
  };
}

describe('e2e KVM switch contracts', () => {
  it('host crossing the right edge enters the remote on the left', () => {
    const plan = switchPlan('host', 'right', '10.0.0.8');
    expect(plan.action).toBe('switch_to_peer');
    if (plan.action === 'switch_to_peer') {
      expect(plan.target).toBe('10.0.0.8');
      expect(plan.entry.edge).toBe('left');
    }
  });

  it('client edge uses host SwitchLocal, not a local daemon', () => {
    const plan = switchPlan('client', 'left');
    expect(plan.action).toBe('switch_local_to_host');
    expect(plan.channel).toBe(':5259');
  });

  it('clamps portal Y into a valid entry position', () => {
    expect(entryFor('right', 1.4).normalized_position).toBe(1);
    expect(entryFor('left', -2).normalized_position).toBe(0);
  });
});
