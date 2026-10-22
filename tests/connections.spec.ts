import { test, expect } from '@playwright/test';

const sampleToken = 'opentoken-demo-token';
test('Cursor connection verifies a selected sample member, survives reload and disconnects without touching Codex', async ({ page }) => {
  await page.setViewportSize({ width: 454, height: 1100 });
  await page.goto('/?demo');
  await page.getByRole('tab', { name: 'Cursor', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Use existing Cursor', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Enterprise setup', exact: true }).click();
  await expect(page.getByRole('tabpanel')).toContainText('personal Cursor plans are not supported');
  await page.getByLabel('Team member email', { exact: true }).fill('member@example.test');
  await page.locator('.provider-connection').screenshot({ path: 'artifacts/connection-cursor.png' });
  await page.getByLabel('Enterprise admin API key', { exact: true }).fill(sampleToken);
  await page.getByRole('button', { name: 'Verify & connect', exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'On-demand budget remaining', exact: true })).toHaveAttribute('aria-valuenow', '75');
  await expect(page.locator('.account-card')).toHaveCount(1);
  await expect(page.locator('.account-card')).toContainText('member@example.test');
  await expect(page.locator('.donut, .token-summary')).toHaveCount(0);
  const saved = await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'));
  expect(saved).not.toContain(sampleToken); expect(saved).not.toContain('"token"');
  await page.reload(); await page.getByRole('tab', { name: 'Cursor', exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'On-demand budget remaining', exact: true })).toHaveAttribute('aria-valuenow', '75');
  await page.getByRole('button', { name: 'Manage Cursor accounts', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Provider', exact: true })).toHaveValue('cursor');
  await page.getByRole('button', { name: 'Disconnect member@example.test', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Your provider login was not changed');
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Use existing Cursor', exact: true })).toBeVisible();
  await page.getByRole('tab', { name: 'Codex', exact: true }).click();
  await expect(page.locator('.account-card')).toHaveCount(2);
});

test('Copilot reports personal usage without inventing remaining quota or a local-token donut', async ({ page }) => {
  await page.goto('/?demo'); await page.getByRole('tab', { name: 'Copilot', exact: true }).click();
  await page.getByRole('button', { name: 'Advanced personal billing', exact: true }).click();
  await expect(page.getByRole('tabpanel')).toContainText('Plan: read');
  await expect(page.getByRole('tabpanel')).toContainText('Organization-paid usage');
  await expect(page.getByLabel('Team member email', { exact: true })).toHaveCount(0);
  await page.getByLabel('Fine-grained personal access token', { exact: true }).fill(sampleToken);
  await page.getByRole('button', { name: 'Verify & connect', exact: true }).click();
  await expect(page.locator('.account-card')).toContainText('42 requests');
  await expect(page.locator('.account-card')).not.toContainText('Claude');
  await expect(page.getByRole('progressbar')).toHaveCount(0);
  await expect(page.locator('.donut, .token-summary, #provider-tab-copilot .provider-usage-stripe')).toHaveCount(0);
  await page.locator('main').screenshot({ path: 'artifacts/connection-copilot.png' });
  await page.reload(); await page.getByRole('tab', { name: 'Copilot', exact: true }).click();
  await expect(page.locator('.account-card')).toContainText('42 requests');
  expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'))).not.toContain(sampleToken);
});

test('an API-only saved profile starts on its configured provider', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('opentoken.demo.settings', JSON.stringify({ codexEnabled: false, claudeEnabled: false, connections: [{ id: 'demo-copilot', provider: 'copilot', subject: 'sample-member', login: 'sample-user', scope: 'personal' }] })));
  await page.goto('/?demo');
  await expect(page.getByRole('tab', { name: 'Copilot', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('.account-card')).toContainText('42 requests');
});

test('failed connection clears the credential field and never pretends to connect', async ({ page }) => {
  await page.goto('/?demo'); await page.getByRole('tab', { name: 'Copilot', exact: true }).click();
  await page.getByRole('button', { name: 'Advanced personal billing', exact: true }).click();
  const token = page.getByLabel('Fine-grained personal access token', { exact: true });
  await token.fill('synthetic-invalid-token'); await page.getByRole('button', { name: 'Verify & connect', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Sample verification failed');
  await expect(token).toHaveValue(''); await expect(page.locator('.account-card')).toHaveCount(0);
  expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'))).toBeNull();
  await token.fill(sampleToken); await page.getByRole('button', { name: 'Verify & connect', exact: true }).click();
  await expect(page.locator('.account-card')).toContainText('42 requests');
});

test('switching provider-specific forms discards secrets and native-session providers request none', async ({ page }) => {
  await page.goto('/?demo'); await page.getByRole('button', { name: 'Add provider', exact: true }).click();
  const provider = page.getByRole('combobox', { name: 'Provider', exact: true });
  await provider.selectOption('copilot'); await page.getByRole('button', { name: 'Advanced personal billing', exact: true }).click(); await page.getByLabel('Fine-grained personal access token', { exact: true }).fill(sampleToken);
  await provider.selectOption('devin');
  await expect(page.getByRole('button',{name:'Use existing Devin',exact:true})).toBeVisible(); await expect(page.locator('input[type="password"]')).toHaveCount(0);
  await provider.selectOption('copilot'); await page.getByRole('button', { name: 'Advanced personal billing', exact: true }).click(); await expect(page.getByLabel('Fine-grained personal access token', { exact: true })).toHaveValue('');
});

test('targeted connection saves preserve unsaved preferences and the unpinned default', async ({ page }) => {
  await page.goto('/?demo'); await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
  await page.getByRole('button', { name: 'Providers', exact: true }).click();
  await page.getByRole('combobox', { name: 'Provider', exact: true }).selectOption('copilot');
  await page.getByRole('button', { name: 'Advanced personal billing', exact: true }).click();
  await page.getByLabel('Fine-grained personal access token', { exact: true }).fill(sampleToken);
  await page.getByRole('button', { name: 'Verify & connect', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Copilot connected');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Theme', exact: true })).toHaveValue('dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('opentoken.demo.settings') ?? '{}'));
  expect(saved.dashboardPinned).toBe(false); expect(saved.theme).toBe('light'); expect(saved.connections).toHaveLength(1);
});

test('failed secure-save preview rolls back connection metadata and clears the submitted secret', async ({ page }) => {
  await page.goto('/?demo'); await page.getByRole('tab', { name: 'Copilot', exact: true }).click();
  await page.getByRole('button', { name: 'Advanced personal billing', exact: true }).click();
  await page.evaluate(() => { const original = Storage.prototype.setItem; Storage.prototype.setItem = function(key, value) { if (key === 'opentoken.demo.settings') throw new DOMException('Synthetic save failure', 'QuotaExceededError'); original.call(this, key, value); }; });
  const token = page.getByLabel('Fine-grained personal access token', { exact: true });
  await token.fill(sampleToken); await page.getByRole('button', { name: 'Verify & connect', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Synthetic save failure'); await expect(token).toHaveValue('');
  await expect(page.locator('.account-card')).toHaveCount(0);
  expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'))).toBeNull();
});
