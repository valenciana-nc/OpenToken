import { test, expect, type Page } from '@playwright/test';

async function museUsage(page: Page) {
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await page.getByRole('combobox', { name: 'Usage provider', exact: true }).selectOption('muse');
}

test('source discovery has all twelve explicit capabilities without pretending they are twelve logged-in accounts', async ({ page }) => {
  await page.goto('/?demo');
  await page.getByRole('button', { name: 'Add provider', exact: true }).click();
  const panel = page.locator('.panel').filter({ has: page.getByRole('heading', { name: 'Usage sources', exact: true }) });
  await panel.getByText('Detected source capabilities', { exact: true }).click();
  await expect(panel.getByRole('button', { name: /^View .* sources$/ })).toHaveCount(12);
  await expect(panel).toContainText('Found data is not a verified login');
  await expect(panel).toContainText('Potential source found · not verified');
  await expect(page.getByRole('button', { name: 'Sign in with Codex', exact: true })).toHaveCount(0);
  await panel.getByRole('button', { name: 'View Kimi sources', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Provider', exact: true })).toHaveValue('kimi');
});

test('unified settings preserve drafts/startup/pin across existing Codex connection and Usage navigation', async ({ page }) => {
  await page.goto('/?demo');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Providers & settings', exact: true })).toBeVisible();
  await page.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
  await page.getByRole('button', { name: 'Providers', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Providers', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Use existing Codex', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('2 existing Codex accounts verified');
  expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.cli'))).toBeNull();
  const s = await page.evaluate(() => JSON.parse(localStorage.getItem('opentoken.demo.settings') ?? '{}'));
  expect(s.codexHomes).toEqual([]); expect(s.dashboardPinned).toBe(false);
  expect(s.launchAtStartup).toBe(false); expect(s.connections).toEqual([]);
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Theme', exact: true })).toHaveValue('dark');
  await expect(page.getByRole('button', { name: 'App settings', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
});

test('an unavailable existing session does not silently open recovery or claim connection', async ({ page }) => {
  await page.goto('/?demo&cursorExisting=missing');
  await page.getByRole('tab', { name: 'Cursor', exact: true }).click();
  await page.getByRole('button', { name: 'Use existing Cursor', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('No login was started');
  await expect(page.getByRole('button', { name: 'Sign in with Cursor', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Cancel sign-in', exact: true })).toHaveCount(0);
  await expect(page.locator('.account-card')).toHaveCount(0);
  expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'))).toBeNull();
});

test('Muse local tokens need no login or quota, and stopping tracking retains imported history on Usage', async ({ page }) => {
  await page.goto('/?demo');
  await page.getByRole('tab', { name: 'Muse Code', exact: true }).click();
  await expect(page.locator('input[type="password"]')).toHaveCount(0);
  await expect(page.getByRole('button', { name: /Sign in/ })).toHaveCount(0);
  await page.getByRole('button', { name: 'Track local usage', exact: true }).click();
  await expect(page.locator('.token-summary')).toHaveCount(0);
  await expect(page.getByRole('progressbar')).toHaveCount(0);
  await expect(page.locator('.account-card')).toHaveCount(0);
  await museUsage(page);
  await expect(page.locator('.token-stat strong').first()).toHaveText('125 tokens');
  await expect(page.locator('.donut-total strong')).toHaveText('125');
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  await page.getByRole('button', { name: 'Manage Muse Code accounts', exact: true }).click();
  await page.getByText('Detected source capabilities', { exact: true }).click();
  await page.getByRole('button', { name: 'Stop Muse Code local tracking', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Imported totals were retained');
  await museUsage(page);
  await expect(page.locator('.token-stat strong').first()).toHaveText('125 tokens');
  await page.reload();
  await museUsage(page);
  await expect(page.locator('.token-stat strong').first()).toHaveText('125 tokens');
});

test('Kimi local tracking is independent of API-key quota access', async ({ page }) => {
  await page.goto('/?demo');
  await page.getByRole('tab', { name: 'Kimi', exact: true }).click();
  await page.getByRole('button', { name: 'Track local usage', exact: true }).click();
  const s = await page.evaluate(() => JSON.parse(localStorage.getItem('opentoken.demo.settings') ?? '{}'));
  expect(s.historySources.kimi).toBe(true); expect(s.connections).toEqual([]);
  await expect(page.getByRole('progressbar')).toHaveCount(0);
  await expect(page.locator('.account-card')).toHaveCount(0);
  await expect(page.getByLabel('Kimi Code API key', { exact: true })).toHaveValue('');
});

test('failed local-source saves roll back instead of claiming recording is active', async ({ page }) => {
  await page.goto('/?demo');
  await page.getByRole('tab', { name: 'Muse Code', exact: true }).click();
  await page.evaluate(() => {
    const old = Storage.prototype.setItem;
    Storage.prototype.setItem = function(k, v) {
      if (k === 'opentoken.demo.settings') throw new DOMException('Synthetic source save failure', 'QuotaExceededError');
      old.call(this, k, v);
    };
  });
  await page.getByRole('button', { name: 'Track local usage', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Synthetic source save failure');
  expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'))).toBeNull();
  await museUsage(page);
  await expect(page.locator('.token-stat strong').first()).toHaveText('— tokens');
});
