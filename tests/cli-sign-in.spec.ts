import { test, expect } from '@playwright/test';
import { openAccountSetup } from './helpers/recovery';
for (const [provider, name, initial] of [['codex', 'Codex', 2], ['claude', 'Claude', 1]] as const) {
  test(`${name} offers one sign-in action, automatically connects and keeps existing accounts`, async ({ page }) => {
    await page.goto('/?demo'); await page.getByRole('tab', { name, exact: true }).click();
    await expect(page.locator('.account-card')).toHaveCount(initial);
    await page.getByRole('button', { name: `Manage ${name} accounts`, exact: true }).click();
    await expect(page.getByRole('combobox', { name: 'Provider', exact: true })).toHaveValue(provider);
    await expect(page.getByLabel('Login folder', { exact: true })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Browse folders', exact: true })).toHaveCount(0);
    await expect(page.getByRole('heading', { name: 'Tracked folders', exact: true })).toHaveCount(0);
    await expect(page.locator('.cli-sign-in input, .cli-sign-in select, .sign-in-caption, .sign-in-info')).toHaveCount(0);
    await openAccountSetup(page); await expect(page.getByRole('button', { name: `Sign in with ${name}`, exact: true })).toHaveAccessibleDescription(new RegExp(`official ${name} CLI sign-in`));
    await expect(page.getByText('Official sign-in · account setup handled for you', { exact: true })).toHaveCount(0);
    await page.getByRole('button', { name: `Sign in with ${name}`, exact: true }).click();
    await expect(page.getByRole('status', { name: `${name} sign-in status`, exact: true })).toHaveText(`${name} connected.`);
    await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
    await expect(page.locator('.account-card')).toHaveCount(initial + 1);
    await page.reload(); await page.getByRole('tab', { name, exact: true }).click();
    await expect(page.locator('.account-card')).toHaveCount(initial + 1);
  });
  test(`${name} completes after navigation without overwriting unsaved preferences`, async ({ page }) => {
    await page.goto(`/?demo&${provider}Login=waiting`);
    await page.getByRole('button', { name: 'Add provider', exact: true }).click();
    await page.getByRole('combobox', { name: 'Provider', exact: true }).selectOption(provider);
    await openAccountSetup(page); await page.getByRole('button', { name: `Sign in with ${name}`, exact: true }).click();
    await expect(page.getByRole('status', { name: `${name} sign-in status`, exact: true })).toContainText('automatically');
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
    await page.evaluate(provider => window.dispatchEvent(new CustomEvent('opentoken-demo-cli-ready', { detail: provider })), provider);
    await expect.poll(async () => page.evaluate(() => JSON.parse(localStorage.getItem('opentoken.demo.cli') ?? '[]').length)).toBe(1);
    await expect(page.getByRole('combobox', { name: 'Theme', exact: true })).toHaveValue('dark');
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await page.getByRole('button', { name: 'Save preferences', exact: true }).click();
    const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('opentoken.demo.settings')!));
    expect(saved[provider === 'codex' ? 'codexHomes' : 'claudeHomes']).toHaveLength(1);
    expect(saved.dashboardPinned).toBe(false);
  });
  test(`${name} cancellation ignores late readiness and disconnect leaves other identities`, async ({ page }) => {
    await page.goto(`/?demo&${provider}Login=waiting`); await page.getByRole('button', { name: 'Add provider', exact: true }).click();
    await page.getByRole('combobox', { name: 'Provider', exact: true }).selectOption(provider);
    await openAccountSetup(page); await page.getByRole('button', { name: `Sign in with ${name}`, exact: true }).click();
    await page.getByRole('button', { name: 'Cancel sign-in', exact: true }).click();
    await page.evaluate(provider => window.dispatchEvent(new CustomEvent('opentoken-demo-cli-ready', { detail: provider })), provider);
    await expect(page.getByRole('status', { name: `${name} sign-in status`, exact: true })).toHaveText('Sign-in cancelled.');
    expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.cli'))).toBeNull();
    await page.getByRole('button', { name: `Sign in with ${name}`, exact: true }).click();
    await page.evaluate(provider => window.dispatchEvent(new CustomEvent('opentoken-demo-cli-ready', { detail: provider })), provider);
    await expect(page.getByRole('status', { name: `${name} sign-in status`, exact: true })).toHaveText(`${name} connected.`);
    const label = provider === 'codex' ? 'demo-codex-extra-1@example.test' : 'Claude · sample account';
    await page.getByRole('button', { name: `Disconnect ${label}`, exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: 'Monitoring stopped' })).toBeVisible();
    await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
    await page.getByRole('tab', { name, exact: true }).click(); await expect(page.locator('.account-card')).toHaveCount(initial);
    await page.reload(); await page.getByRole('tab', { name, exact: true }).click(); await expect(page.locator('.account-card')).toHaveCount(initial);
  });
  test(`${name} missing CLI and verification failures never pretend to connect`, async ({ page }) => {
    for (const scenario of ['error', 'failure']) {
      await page.goto(`/?demo&${provider}Login=${scenario}`); await page.getByRole('button', { name: 'Add provider', exact: true }).click();
      await page.getByRole('combobox', { name: 'Provider', exact: true }).selectOption(provider);
      await openAccountSetup(page); await page.getByRole('button', { name: `Sign in with ${name}`, exact: true }).click();
      await expect(page.getByRole('status', { name: `${name} sign-in status`, exact: true })).toContainText(scenario === 'error' ? 'not found' : 'verification failed');
      if (scenario === 'error') await expect(page.getByRole('button', { name: `Get ${name} CLI`, exact: true })).toBeVisible();
      expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.cli'))).toBeNull();
    }
  });
}
test('a failed CLI metadata save does not remove an existing sample connection', async ({ page }) => {
  await page.goto('/?demo'); await page.getByRole('button', { name: 'Add provider', exact: true }).click();
  await openAccountSetup(page); await page.getByRole('button', { name: 'Sign in with Codex', exact: true }).click();
  await expect(page.getByRole('status', { name: 'Codex sign-in status', exact: true })).toHaveText('Codex connected.');
  const before = await page.evaluate(() => localStorage.getItem('opentoken.demo.cli'));
  await page.evaluate(() => { const original = Storage.prototype.setItem; Storage.prototype.setItem = function(key, value) { if (key === 'opentoken.demo.settings') throw new DOMException('Synthetic CLI save failure', 'QuotaExceededError'); original.call(this, key, value); }; });
  await openAccountSetup(page); await page.getByRole('button', { name: 'Sign in with Codex', exact: true }).click();
  await expect(page.getByRole('status', { name: 'Codex sign-in status', exact: true })).toContainText('Synthetic CLI save failure');
  expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.cli'))).toBe(before);
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click(); await expect(page.locator('.account-card')).toHaveCount(3);
});
for (const theme of ['light', 'dark']) test(`compact account setup fits the ${theme} popup without folder controls`, async ({ page }) => {
  await page.addInitScript(theme => localStorage.setItem('opentoken.demo.settings', JSON.stringify({ theme })), theme);
  await page.goto('/?demo'); await page.getByRole('button', { name: 'Add provider', exact: true }).click();
  await openAccountSetup(page); const action = page.getByRole('button', { name: 'Sign in with Codex', exact: true });
  await action.scrollIntoViewIfNeeded();
  await expect(action).toBeVisible();
  await expect(page.locator('.cli-sign-in')).toHaveCSS('border-top-width', '0px');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.locator('main').screenshot({ path: `artifacts/simple-accounts-${theme}.png` });
});
