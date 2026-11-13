import { test, expect, type Page } from '@playwright/test';
import { openAccountSetup } from './helpers/recovery';

async function fault(page: Page, provider: string, kind: string | null, cached = true) {
  await page.evaluate(detail => dispatchEvent(new CustomEvent('opentoken-demo-source-fault', { detail })), { provider, kind, cached });
}
for (const name of ['Copilot', 'Grok']) {
  const provider = name.toLowerCase();
  test(`${name} keeps its saved scope and labels cached usage during failures, then automatically recovers`, async ({ page }) => {
    await page.goto('/?demo');
    await page.getByRole('tab', { name, exact: true }).click();
    await page.getByRole('button', { name: `Use existing ${name}`, exact: true }).click();
    const before = await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'));
    const id = await page.locator('.account-card').getAttribute('id');
    const left = await page.getByRole('progressbar').getAttribute('aria-valuenow');
    for (const kind of ['offline', 'rate_limited', 'unreadable']) {
      await fault(page, provider, kind);
      await expect(page.getByRole('status', { name: `${name} monitoring status` })).toContainText('Monitoring saved · cached usage');
      await expect(page.getByRole('status', { name: `${name} monitoring status` })).toContainText('not live');
      await expect(page.locator('.account-card')).toHaveAttribute('id', id!);
      await expect(page.locator('.account-card')).toHaveClass(/stale/);
      await expect(page.getByRole('progressbar')).toHaveAttribute('aria-valuenow', left!);
      await expect(page.getByRole('button', { name: `Use existing ${name}`, exact: true })).toHaveCount(0);
      await expect(page.getByRole('tab', { name, exact: true })).toHaveAttribute('title', /Monitoring saved · cached usage/);
      // Cached quota is not painted as a verified current mini-stripe.
      await expect(page.getByRole('tab', { name, exact: true }).locator('.provider-usage-stripe')).toHaveCount(0);
      expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'))).toBe(before);
    }
    await fault(page, provider, null);
    await expect(page.locator('.account-card')).toHaveClass(/ready/);
    await expect(page.getByRole('status', { name: `${name} monitoring status` })).toHaveCount(0);
    await expect(page.getByRole('tab', { name, exact: true }).locator('.provider-usage-stripe')).toHaveCount(1);
    expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'))).toBe(before);
  });

  test(`${name} distinguishes cold read failure from rejected login without deleting the connection`, async ({ page }) => {
    await page.goto('/?demo');
    await page.getByRole('tab', { name, exact: true }).click();
    await page.getByRole('button', { name: `Use existing ${name}`, exact: true }).click();
    const before = await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'));
    await fault(page, provider, 'offline', false);
    await expect(page.getByRole('status', { name: `${name} monitoring status` })).toContainText('usage unavailable');
    await expect(page.getByRole('progressbar')).toHaveCount(0);
    await expect(page.getByRole('button', { name: `Use existing ${name}`, exact: true })).toHaveCount(0);
    await fault(page, provider, 'needs_relogin', false);
    await expect(page.getByRole('status', { name: `${name} monitoring status` })).toContainText('sign-in needs recovery');
    await expect(page.getByRole('button', { name: `Use existing ${name}`, exact: true })).toBeVisible();
    await expect(page.getByRole('progressbar')).toHaveCount(0);
    await expect(page.getByRole('button', { name: name === 'Grok' ? 'Recover Grok session' : 'Sign in with GitHub', exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'))).toBe(before);
    await openAccountSetup(page);
    await expect(page.getByRole('button', { name: name === 'Grok' ? 'Recover Grok session' : 'Sign in with GitHub', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Cancel sign-in', exact: true })).toHaveCount(0);
  });
}
