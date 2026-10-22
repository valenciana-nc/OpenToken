import { describe, expect, it } from 'vitest';
import { age, amount, countdown, tone, remaining } from './format';
const now = Date.parse('2026-09-12T12:00:00Z');
describe('reset countdowns', () => {
  it('updates without making network calls', () => { expect(countdown('2026-09-12T15:13:00Z', now)).toBe('Resets in 3h 13m'); expect(countdown('2026-09-12T15:13:00Z', now + 60_000)).toBe('Resets in 3h 12m'); });
  it('handles reset due, bad dates and absent windows', () => { expect(countdown('2026-09-12T11:00:00Z', now)).toBe('Reset due'); expect(countdown('garbage', now)).toBeNull(); expect(countdown(null, now)).toBeNull(); });
  it('formats long windows', () => expect(countdown('2026-09-14T13:00:00Z', now)).toBe('Resets in 2d 1h'));
});
it('formats typed money and credits', () => { expect(amount({ id: 'credits', label: 'Credits', percent: null, value: 821, unit: 'credits', resetsAt: null })).toContain('$32.84'); expect(amount({ id: 'extra', label: 'Extra', percent: 25, value: 2.5, unit: 'USD', resetsAt: null })).toBe('$2.50'); });
it('shows actual observation age, not last failure time', () => { expect(age(null, now)).toBe('Waiting for first reading'); expect(age('2026-09-12T11:00:00Z', now)).toBe('Updated 1h ago'); });
it('converts used quota into remaining without changing the API values', () => { expect(remaining(40)).toBe(60); expect(remaining(0)).toBe(100); expect(remaining(100)).toBe(0); expect(remaining(120)).toBe(0); expect(remaining(null)).toBeNull(); expect(remaining(NaN)).toBeNull(); });
it('colors thresholds consistently', () => { expect(tone(89)).toBe('warning'); expect(tone(90)).toBe('danger'); expect(tone(0)).toBe('healthy'); });
