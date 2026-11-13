import { test, expect } from '@playwright/test';

// CSS contract only. scripts/native-glass.mjs separately checks real DWM/composited pixels.
for (const theme of ['light', 'dark'] as const) {
  test(`native glass keeps one taskbar-weight tint across every page in ${theme}`, async ({ page }) => {
    await page.addInitScript(theme => localStorage.setItem('opentoken.demo.settings', JSON.stringify({ theme })), theme);
    await page.goto('/?demo');
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    // Exercise desktop styles without pretending a browser can apply Windows acrylic.
    await page.evaluate(() => { document.documentElement.dataset.desktop = 'true'; });
    async function expectGlass() {
      await expect(page.locator('.surface')).toHaveCSS('background-color', theme === 'light' ? 'rgba(243, 243, 243, 0.8)' : 'rgba(32, 32, 32, 0.84)');
      await expect(page.locator('.surface')).toHaveCSS('opacity', '1');
      await expect(page.locator('.surface')).toHaveCSS('backdrop-filter', 'none');
      for (const region of await page.locator('html, body, #root, .app, .panel-header, .provider-grid, main, footer, .save-bar').all()) {
        await expect(region).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
        await expect(region).toHaveCSS('opacity', '1');
        await expect(region).toHaveCSS('backdrop-filter', 'none');
      }
      expect(await page.locator('.surface').boundingBox()).toEqual({ x: 0, y: 0, width: 454, height: 760 });
      await expect(page.locator('.surface')).toHaveCSS('color', theme === 'light' ? 'rgb(27, 27, 27)' : 'rgb(244, 244, 244)');
    }
    await expectGlass();
    await page.getByRole('button', { name: 'Usage', exact: true }).click();
    await expect(page.locator('.usage-page')).toBeVisible();
    await expectGlass();
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Save preferences', exact: true })).toBeVisible();
    await expectGlass();
    await page.getByRole('button', { name: 'Providers', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Connect existing usage', exact: true })).toBeVisible();
    await expectGlass();
    await page.emulateMedia({ forcedColors: 'active' });
    const canvas = await page.evaluate(() => {
      const probe = document.createElement('div'); probe.style.background = 'Canvas';
      document.body.append(probe); const color = getComputedStyle(probe).backgroundColor; probe.remove();
      return color;
    });
    await expect(page.locator('.surface')).toHaveCSS('background-color', canvas);
    expect(canvas).toMatch(/^rgb\(/);
  });

  test(`scrollbars retain a visible thumb without a solid track in ${theme}`, async ({ page }) => {
    await page.addInitScript(theme => localStorage.setItem('opentoken.demo.settings', JSON.stringify({ theme })), theme);
    await page.goto('/?demo');
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    await expect(page.locator('.account-card')).toHaveCount(2);
    await page.evaluate(() => { document.documentElement.dataset.desktop = 'true'; });
    const main = page.locator('main');
    await expect.poll(() => main.evaluate(element => element.scrollHeight > element.clientHeight)).toBe(true);
    const thumb = theme === 'light' ? 'rgba(0, 0, 0, 0.36)' : 'rgba(255, 255, 255, 0.38)';
    await expect(main).toHaveCSS('scrollbar-color', `${thumb} rgba(0, 0, 0, 0)`);
    await expect(main).toHaveCSS('scrollbar-width', 'thin');
    for (const pseudo of ['::-webkit-scrollbar', '::-webkit-scrollbar-track', '::-webkit-scrollbar-track-piece', '::-webkit-scrollbar-corner']) {
      expect(await main.evaluate((element, pseudo) => getComputedStyle(element, pseudo).backgroundColor, pseudo)).toBe('rgba(0, 0, 0, 0)');
    }
    await main.hover(); await page.mouse.wheel(0, 500);
    await expect.poll(() => main.evaluate(element => element.scrollTop)).toBeGreaterThan(0);
    await page.emulateMedia({ forcedColors: 'active' });
    await expect(main).toHaveCSS('scrollbar-color', 'auto');
  });

  test(`browser preview keeps a readable ${theme} background without native DWM`, async ({ page }) => {
    await page.addInitScript(theme => localStorage.setItem('opentoken.demo.settings', JSON.stringify({ theme })), theme);
    await page.goto('/?demo');
    await expect(page.locator('html')).toHaveAttribute('data-desktop', 'false');
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    await expect(page.locator('html')).not.toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
    await expect(page.locator('.surface')).toHaveCSS('background-color', theme === 'light' ? 'rgba(245, 245, 247, 0.88)' : 'rgba(29, 29, 32, 0.86)');
  });
}
