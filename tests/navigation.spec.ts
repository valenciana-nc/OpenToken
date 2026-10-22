import { test, expect } from '@playwright/test';

for (const theme of ['light', 'dark'] as const) {
  test(`unified settings and the separate Usage page remain unboxed and keyboard accessible in ${theme}`, async ({ page }) => {
    await page.addInitScript(theme => localStorage.setItem('opentoken.demo.settings', JSON.stringify({ theme })), theme);
    await page.goto('/?demo');
    await expect(page.getByRole('button', { name: 'Accounts', exact: true })).toHaveCount(0);
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Providers & settings', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Providers', exact: true }).focus();
    await page.keyboard.press('Enter');
    await expect(page.getByRole('combobox', { name: 'Provider', exact: true })).toHaveValue('codex');
    await expect(page.getByRole('button', { name: 'Settings', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await page.screenshot({ path: `artifacts/providers-settings-${theme}.png` });
    await page.getByRole('button', { name: 'Usage', exact: true }).focus();
    await page.keyboard.press('Enter');
    await expect(page.getByRole('button', { name: 'Usage', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByRole('combobox', { name: 'Usage provider', exact: true }).locator('option')).toHaveCount(13);
    await expect(page.locator('.account-card, .provider-grid')).toHaveCount(0);
    await expect(page.locator('.spend-surface')).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
    await expect(page.locator('.spend-surface')).toHaveCSS('box-shadow', 'none');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
    await page.screenshot({ path: `artifacts/usage-${theme}.png` });
  });
}

test('Usage pricing link opens app settings without saving or losing a draft', async ({ page }) => {
  await page.goto('/?demo');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  await page.getByRole('tab', { name: 'Muse Code', exact: true }).click();
  await page.getByRole('button', { name: 'Track local usage', exact: true }).click();
  const saved = await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'));
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await page.getByRole('combobox', { name: 'Usage provider', exact: true }).selectOption('muse');
  await page.getByRole('button', { name: 'Set missing model rates', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Providers & settings', exact: true })).toBeAttached();
  await expect(page.getByRole('heading', { name: 'Local usage & pricing', exact: true })).toBeInViewport();
  await expect(page.getByRole('combobox', { name: 'Theme', exact: true })).toHaveValue('dark');
  expect(await page.evaluate(() => localStorage.getItem('opentoken.demo.settings'))).toBe(saved);
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Usage provider', exact: true })).toHaveValue('muse');
  await expect(page.locator('.token-stat strong').first()).toHaveText('125 tokens');
});
