import { describe, expect, it, vi } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
vi.mock('../bridge', () => ({ drag: vi.fn(), hide: vi.fn() }));
import PanelHeader from './StatusBar';
import { defaultSettings } from '../types';
import type { Page } from '../ui';
function render(pinned: boolean, pinDisabled = false, page: Page = 'usage') {
  return renderToStaticMarkup(<PanelHeader page={page} navigate={() => {}} refreshing={false} refresh={() => {}} pinned={pinned} pinDisabled={pinDisabled} togglePin={() => {}}/>);
}
describe('dashboard header controls', () => {
  it('starts unpinned until the user explicitly pins the dashboard', () => {
    expect(defaultSettings.dashboardPinned).toBe(false);
    const html = render(defaultSettings.dashboardPinned);
    expect(html).toContain('aria-label="Pin dashboard"');
    expect(html).toContain('aria-pressed="false"');
  });
  it('places an accessible pin immediately after refresh and uses a collapse chevron, not X', () => {
    const html = render(true);
    expect(html.indexOf('Refresh usage')).toBeLessThan(html.indexOf('Unpin dashboard'));
    expect(html.indexOf('Unpin dashboard')).toBeLessThan(html.indexOf('aria-label="Usage"'));
    expect(html).toContain('aria-pressed="true"'); expect(html).toContain('dismiss when clicking another app'); expect(html).toContain('Collapse dashboard');
    expect(html).toContain('lucide-chevron-down'); expect(html).not.toContain('lucide-x');
  });
  it('has one settings destination for both configuration sections', () => {
    for (const page of ['accounts', 'settings'] as const) {
      const html = render(false, false, page);
      expect(html).toMatch(/aria-label="Settings"[^>]*aria-pressed="true"/);
      expect(html).not.toContain('aria-label="Accounts"');
    }
  });
  it('gives Usage a distinct accessible navigation control', () => {
    const html = render(false, false, 'spending');
    expect(html).toMatch(/aria-label="Usage"[^>]*aria-pressed="true"/);
    expect(html).toMatch(/aria-label="Settings"[^>]*aria-pressed="false"/);
    expect(html).toContain('Back to providers');
  });
  it('shows the unpinned state and disables repeat changes while saving', () => {
    const html = render(false, true);
    expect(html).toContain('aria-label="Pin dashboard"'); expect(html).toContain('stay open when switching apps'); expect(html).toContain('aria-pressed="false"');
    expect(html).toMatch(/aria-pressed="false"[^>]*disabled=""/);
  });
});
