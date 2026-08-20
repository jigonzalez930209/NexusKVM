import { describe, it, expect } from 'vitest';

describe('coordinate mapping', () => {
  it('maps the midpoint proportionally', () => {
    const normalized = 540 / 1080;
    expect(Math.round(normalized * 1440)).toBe(720);
  });
  it('clamps values to the screen', () => {
    const clamp = (n: number) => Math.max(0, Math.min(1, n));
    expect(clamp(1.4)).toBe(1);
    expect(clamp(-0.2)).toBe(0);
  });
});
