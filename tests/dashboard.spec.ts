import { test, expect } from '@playwright/test';

test('provider grid preserves existing choices and adds Z.ai, DeepSeek, Kimi and OpenCode', async ({ page }) => {
  await page.goto('/?demo');
  const names = ['Codex', 'Claude', 'Antigravity', 'Grok', 'Muse Code', 'Cursor', 'Copilot', 'Devin', 'Z.ai', 'DeepSeek', 'Kimi', 'OpenCode'];
  await expect(page.getByRole('tab')).toHaveCount(12);
  for (const name of ['Amp', 'ElevenLabs', 'Zed', 'Perplexity', 'Pi', 'TypeSafe']) await expect(page.getByRole('tab', { name, exact: true })).toHaveCount(0);
  await expect(page.getByRole('tab')).toHaveText(names);
  for (const name of names) await expect(page.getByRole('tab', { name, exact: true })).toBeVisible();
  await expect(page.getByRole('tab', { name: 'Overview', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Overview', exact: true })).toHaveCount(0);
  const marks = await page.locator('.provider-grid .provider-mark').evaluateAll(es => es.map(e => getComputedStyle(e).maskImage));
  expect(marks.every(m => m !== 'none')).toBeTruthy(); expect(new Set(marks).size).toBe(12);
  let previousRowY = -1;
  for (const row of [names.slice(0, 5), names.slice(5, 10), names.slice(10)]) {
    const boxes = await Promise.all(row.map(name => page.getByRole('tab', { name, exact: true }).boundingBox()));
    expect(boxes[0]!.y).toBeGreaterThan(previousRowY); previousRowY = boxes[0]!.y;
    for (const [i, box] of boxes.entries()) {
      expect(Math.abs(box!.y - boxes[0]!.y)).toBeLessThan(1);
      if (i) expect(box!.x).toBeGreaterThan(boxes[i - 1]!.x);
    }
  }
  const opencode = await page.getByRole('tab', { name: 'OpenCode', exact: true }).boundingBox();
  const add = await page.getByRole('button', { name: 'Add provider', exact: true }).boundingBox();
  expect(Math.abs(opencode!.y - add!.y)).toBeLessThan(1); expect(add!.x).toBeGreaterThan(opencode!.x);
  await page.locator('.provider-grid').screenshot({ path: 'artifacts/provider-grid-preview.png' });
  await page.getByRole('button', { name: 'Add provider', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Connect existing usage', exact: true })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Provider', exact: true })).toHaveValue('codex');
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'Codex', exact: true })).toHaveAttribute('aria-selected', 'true');
});

for (const theme of ['light', 'dark'] as const) {
  test(`selected provider uses a neutral gray square in ${theme} mode without recoloring quota bars`, async ({ page }) => {
    await page.addInitScript(theme => localStorage.setItem('opentoken.demo.settings', JSON.stringify({ theme })), theme);
    await page.goto('/?demo');
    const codex = page.getByRole('tab', { name: 'Codex', exact: true });
    const claude = page.getByRole('tab', { name: 'Claude', exact: true });
    const gray = theme === 'light' ? 'rgb(222, 222, 222)' : 'rgb(69, 69, 69)';
    const text = theme === 'light' ? 'rgb(27, 27, 27)' : 'rgb(244, 244, 244)';
    const blue = theme === 'light' ? 'rgb(0, 120, 212)' : 'rgb(96, 205, 255)';
    await expect(codex).toHaveCSS('background-color', gray);
    await expect(codex).toHaveCSS('color', text);
    await expect(codex.locator('.provider-mark')).toHaveCSS('background-color', text);
    await expect(codex.locator('.provider-usage-stripe')).toHaveCSS('background-color', theme === 'light' ? 'rgba(0, 0, 0, 0.12)' : 'rgba(255, 255, 255, 0.14)');
    await expect(claude).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
    for (const bar of await page.locator('.track > div').all()) await expect(bar).toHaveCSS('background-color', blue);
    await codex.screenshot({ path: `artifacts/provider-selected-gray-${theme}.png` });
    await codex.focus(); await page.keyboard.press('ArrowRight');
    await expect(claude).toBeFocused();
    await expect(claude).toHaveAttribute('aria-selected', 'true');
    await expect(claude).toHaveCSS('background-color', gray);
    await expect(codex).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
    await expect(page.getByRole('progressbar')).toHaveCount(3);
  });
  test(`Add uses the same bare monochrome icon style in ${theme} mode`, async ({ page }) => {
    await page.addInitScript(theme => localStorage.setItem('opentoken.demo.settings', JSON.stringify({ theme })), theme);
    await page.goto('/?demo');
    const add = page.getByRole('button', { name: 'Add provider', exact: true });
    const mark = add.locator('.add-provider-mark');
    const reference = page.getByRole('tab', { name: 'OpenCode', exact: true });
    await expect(mark).toHaveAttribute('aria-hidden', 'true');
    await expect(mark).toHaveCSS('width', '31px'); await expect(mark).toHaveCSS('height', '31px');
    await expect(mark).toHaveCSS('border-top-width', '0px'); await expect(mark).toHaveCSS('border-radius', '0px');
    await expect(mark).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)'); await expect(mark).toHaveCSS('box-shadow', 'none');
    await expect(mark).toHaveCSS('color', await reference.evaluate(e => getComputedStyle(e).color));
    const refMark = await reference.locator('.provider-mark').boundingBox();
    const addMark = await mark.boundingBox();
    expect(addMark!.width).toBe(refMark!.width); expect(addMark!.height).toBe(refMark!.height);
    const refLabel = await reference.locator('.provider-tab-label').boundingBox();
    const addLabel = await add.locator('.provider-tab-label').boundingBox();
    expect(Math.abs(refLabel!.y - addLabel!.y)).toBeLessThan(1);
    await page.locator('.provider-grid').screenshot({ path: `artifacts/provider-grid-${theme}.png` });
    await add.hover();
    const hover = await add.evaluate(e => ({ color: getComputedStyle(e).color, background: getComputedStyle(e).backgroundColor }));
    await reference.hover();
    expect(await reference.evaluate(e => ({ color: getComputedStyle(e).color, background: getComputedStyle(e).backgroundColor }))).toEqual(hover);
    await add.focus(); await page.keyboard.press('Enter');
    await expect(page.getByRole('heading', { name: 'Connect existing usage', exact: true })).toBeVisible();
  });
}

test('account data, themes and preferences work without boxes or horizontal overflow', async ({ page }) => {
  await page.goto('/?demo');
  await expect(page.locator('.demo-banner')).toBeVisible();
  await expect(page.locator('.account-card')).toHaveCount(2);
  await expect(page.getByText('alex@example.com', { exact: true })).toBeVisible();
  await expect(page.getByRole('progressbar')).toHaveCount(4);
  await expect(page.getByRole('progressbar', { name: 'Session remaining', exact: true }).first()).toHaveAttribute('aria-valuenow', '53');
  await expect(page.getByRole('progressbar', { name: 'Weekly remaining', exact: true }).first()).toHaveAttribute('aria-valuenow', '88');
  await expect(page.locator('.spend-overview, .token-summary')).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'OpenToken', exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
  await expect(page.locator('.statusbar, .status-accounts')).toHaveCount(0);
  expect(await page.locator('.app').evaluate(e => getComputedStyle(e).padding)).toBe('0px');
  async function expectFlatSurfaces() {
    for (const surface of await page.locator('.metrics-surface, .spend-surface, .provider-logo').all()) {
      await expect(surface).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
      await expect(surface).toHaveCSS('border-top-width', '0px');
      await expect(surface).toHaveCSS('box-shadow', 'none');
    }
    await expect(page.locator('.account-heading .provider-logo')).toHaveCount(2);
    for (const logo of await page.locator('.account-heading .provider-logo').all()) await expect(logo.locator('.provider-mark')).not.toHaveCSS('mask-image', 'none');
  }
  await expectFlatSurfaces();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
  await expect(page.getByRole('combobox', { name: 'Tray icon style', exact: true })).toHaveValue('badge');
  await page.getByRole('combobox', { name: 'Tray icon style', exact: true }).selectOption('provider_pair');
  await expect(page.getByRole('combobox', { name: 'Dashboard position', exact: true })).toHaveValue('bottom_right');
  await page.getByRole('combobox', { name: 'Dashboard position', exact: true }).selectOption('bottom_left');
  await page.getByRole('button', { name: 'Save preferences', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText(/Preferences saved/);
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  await expectFlatSurfaces();
});

test('Usage is separate from quotas, filters providers, and retains its scope across navigation', async ({ page }) => {
  await page.goto('/?demo');
  await expect(page.locator('.token-summary')).toHaveCount(0);
  await page.getByRole('tab', { name: 'Claude', exact: true }).click();
  await expect(page.locator('.account-card')).toHaveCount(1);
  await expect(page.getByRole('progressbar')).toHaveCount(3);
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Usage', exact: true })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Usage provider', exact: true })).toHaveValue('all');
  await expect(page.locator('.provider-grid, .account-card, [role="progressbar"]')).toHaveCount(0);
  await page.getByRole('combobox', { name: 'Usage provider', exact: true }).selectOption('codex');
  await expect(page.locator('.token-summary')).toContainText('1.2M tokens');
  await expect(page.locator('.token-summary')).toContainText('95.8M tokens');
  await page.getByRole('combobox', { name: 'Usage provider', exact: true }).selectOption('claude');
  await expect(page.locator('.token-summary')).toContainText('35.8M tokens');
  await expect(page.locator('.token-summary')).toContainText('4.3B tokens');
  await expect(page.locator('.token-summary')).toContainText('Last 30 days');
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  await expect(page.getByRole('tab', { name: 'Claude', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('.token-summary')).toHaveCount(0);
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Usage provider', exact: true })).toHaveValue('claude');
  await page.getByRole('combobox', { name: 'Usage provider', exact: true }).selectOption('antigravity');
  await expect(page.locator('.donut-total strong')).toHaveText('—');
  await expect(page.locator('.spend-overview')).not.toContainText('$0');
});

test('disconnected native provider tiles never reuse another provider’s real quota or spend', async ({ page }) => {
  await page.goto('/?demo');
  for (const name of ['Antigravity', 'Devin']) {
    await page.getByRole('tab', { name, exact: true }).click();
    await expect(page.getByRole('tab', { name, exact: true })).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('button',{name:`Use existing ${name}`,exact:true})).toBeVisible();
    await expect(page.getByRole('progressbar')).toHaveCount(0);
    await expect(page.locator('.donut, .token-summary')).toHaveCount(0);
    await expect(page.getByRole('tabpanel')).not.toContainText('$');
    await expect(page.getByRole('tab', { name, exact: true })).toHaveAttribute('title', `${name} · Not connected`);
    await expect(page.getByRole('tab', { name, exact: true }).locator('.provider-usage-stripe')).toHaveCount(0);
  }
});

test('provider grid is keyboard navigable with a single selected tab stop', async ({ page }) => {
  await page.goto('/?demo');
  await page.getByRole('tab', { name: 'Codex', exact: true }).focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('tab', { name: 'Claude', exact: true })).toBeFocused();
  for (const name of ['Antigravity', 'Grok', 'Muse Code']) {
    await page.keyboard.press('ArrowRight');
    await expect(page.getByRole('tab', { name, exact: true })).toBeFocused();
    await expect(page.getByRole('tab', { name, exact: true })).toHaveAttribute('aria-selected', 'true');
  }
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('tab', { name: 'DeepSeek', exact: true })).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(page.getByRole('tab', { name: 'Muse Code', exact: true })).toBeFocused();
  await page.keyboard.press('Home'); await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('tab', { name: 'Cursor', exact: true })).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(page.getByRole('tab', { name: 'Kimi', exact: true })).toBeFocused();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('tab', { name: 'OpenCode', exact: true })).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(page.getByRole('tab', { name: 'Copilot', exact: true })).toBeFocused();
  await page.keyboard.press('End');
  await expect(page.getByRole('tab', { name: 'OpenCode', exact: true })).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Add provider', exact: true })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Providers & settings', exact: true })).toBeVisible();
});

test('browser launch is honest about requiring the desktop host', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('alert')).toContainText('desktop app');
  await expect(page.locator('.account-card')).toHaveCount(0);
});

test('spend periods and collapsible account details remain functional', async ({ page }) => {
  await page.goto('/?demo');
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await page.getByRole('combobox', { name: 'Usage provider', exact: true }).selectOption('codex');
  await expect(page.getByRole('button', { name: '30 Days', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('.donut-total')).toContainText('$134.46');
  await page.getByRole('button', { name: 'Today', exact: true }).click();
  await expect(page.locator('.donut-total')).toContainText('$1.74');
  await page.getByRole('combobox', { name: 'Usage provider', exact: true }).selectOption('claude');
  await expect(page.locator('.donut-total')).toContainText('$49.85');
  await page.getByRole('button', { name: 'Back to providers', exact: true }).click();
  await page.getByRole('tab', { name: 'Claude', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Expand Claude details', exact: true })).toHaveAttribute('aria-expanded', 'false');
  await page.getByRole('button', { name: 'Expand Claude details', exact: true }).click();
  await expect(page.getByText('$49.85 · 35.8M tokens', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Collapse Claude details', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Expand Claude details', exact: true })).toHaveAttribute('aria-expanded', 'false');
});

test('the full Usage page remains available without invented billing data', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await expect(page.locator('.donut-total strong')).toHaveText('—');
  await expect(page.getByRole('group', { name: 'Spending period' })).toBeVisible();
  await expect(page.locator('.period-control button')).toHaveCount(3);
  await page.getByRole('button', { name: 'Yesterday', exact: true }).click();
  await expect(page.locator('.spend-unavailable')).toContainText('yesterday');
  await expect(page.locator('.spend-overview')).not.toContainText('$0');
  await expect(page.locator('.spend-legend dd')).toHaveCount(0);
  await page.getByLabel('About spending data', { exact: true }).click();
  await expect(page.locator('.spend-help p')).toContainText('never as zero');
});

test('credit balances and reset allowances appear only inside expanded usage details', async ({ page }) => {
  await page.goto('/?demo');
  const card = page.locator('#demo-personal');
  const credits = card.getByText('Credits', { exact: true }); const resets = card.getByText('Rate limit resets', { exact: true });
  await expect(credits).toBeHidden(); await expect(resets).toBeHidden();
  await expect(card.locator('.quota-list').getByText('Credits', { exact: true })).toHaveCount(0);
  const toggle = card.getByRole('button', { name: 'Expand Codex details', exact: true });
  const detailsId = await toggle.getAttribute('aria-controls');
  await toggle.click();
  await expect(card.locator('.account-details')).toHaveAttribute('id', detailsId!);
  await expect(credits).toBeVisible(); await expect(resets).toBeVisible();
  await expect(card.locator('.account-details')).toContainText('821 credits · $32.84');
  await expect(card.locator('.account-details')).toContainText('2 available');
  await expect(card.getByRole('progressbar')).toHaveCount(2);
  await card.getByRole('button', { name: 'Collapse Codex details', exact: true }).focus();
  await page.keyboard.press('Enter'); await expect(credits).toBeHidden(); await expect(resets).toBeHidden();
});

test('account handles support keyboard reordering, cancellation and persistence', async ({ page }) => {
  await page.goto('/?demo'); const cards = page.locator('.account-card');
  await expect(cards.first()).toHaveAttribute('id', 'demo-codex-work');
  const handle = page.getByRole('button', { name: 'Reorder Codex Work', exact: true });
  await handle.focus(); await page.keyboard.press('Space');
  await expect(handle).toHaveAttribute('aria-pressed', 'true');
  // dnd-kit deliberately attaches its keyboard sensor on the next macrotask.
  // Wait for that attachment, not just React's synchronous pickup announcement.
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 0)));
  await expect(page.locator('[role="status"][aria-live="assertive"]')).toContainText('position 1 of 2');
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('[role="status"][aria-live="assertive"]')).toContainText('position 2 of 2');
  await page.keyboard.press('Escape');
  await expect(page.locator('[role="status"][aria-live="assertive"]')).toContainText('Move cancelled');
  await expect(cards.first()).toHaveAttribute('id', 'demo-codex-work');
  await handle.click({ trial: true }); await handle.focus(); await page.keyboard.press('Space');
  await expect(handle).toHaveAttribute('aria-pressed', 'true');
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 0)));
  await expect(page.locator('[role="status"][aria-live="assertive"]')).toContainText('position 1 of 2');
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('[role="status"][aria-live="assertive"]')).toContainText('position 2 of 2');
  await page.keyboard.press('Space');
  await expect(cards.first()).toHaveAttribute('id', 'demo-personal');
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem('opentoken.demo.settings') ?? '{}').accountOrder)).toEqual(['demo-personal', 'demo-codex-work']);
  await page.reload(); await expect(cards.first()).toHaveAttribute('id', 'demo-personal');
  await expect(cards.last()).toHaveAttribute('id', 'demo-codex-work');
  await expect(cards.first().getByRole('button', { name: 'Expand Codex details', exact: true })).toHaveAttribute('aria-expanded', 'false');
});

test('six-dot handles reorder cards with the mouse without making quota bars draggable', async ({ page }) => {
  await page.setViewportSize({ width: 454, height: 1450 }); await page.goto('/?demo');
  const first = page.locator('.account-card').first(), last = page.locator('.account-card').last();
  const a = await first.boundingBox(), b = await last.boundingBox(), grip = await first.locator('.account-drag-handle').boundingBox();
  expect(a && b && grip).toBeTruthy();
  await page.mouse.move(grip!.x + grip!.width / 2, grip!.y + grip!.height / 2); await page.mouse.down();
  await page.mouse.move(grip!.x + grip!.width / 2, grip!.y + grip!.height / 2 + 10, { steps: 3 });
  await page.mouse.move(grip!.x + grip!.width / 2, grip!.y + grip!.height / 2 + b!.y + b!.height / 2 - a!.y - a!.height / 2, { steps: 20 });
  await page.mouse.up();
  await expect(page.locator('.account-card').first()).toHaveAttribute('id', 'demo-personal');
  await expect(page.getByRole('progressbar')).toHaveCount(4);
});

test('custom pricing requires explicit rates and persists without changing card order', async ({ page }) => {
  await page.goto('/?demo'); await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByLabel('Model rate override', { exact: true }).fill('custom-model');
  for (const [name, value] of [['Input', '2'], ['Cached input', '0.1'], ['Output', '10'], ['Cache write · 5 min', '2.5'], ['Cache write · 1 hour', '4']]) await page.getByRole('spinbutton', { name, exact: true }).fill(value);
  await page.getByRole('button', { name: 'Use these rates', exact: true }).click();
  await page.getByRole('button', { name: 'Save preferences', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Preferences saved');
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('opentoken.demo.settings') ?? '{}').modelPrices['custom-model'])).toEqual({ input: 2, cached: 0.1, output: 10, cacheWrite: 2.5, cacheWriteHour: 4 });
  await page.reload(); await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.locator('.reference-rates')).toContainText('custom-model');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
});

test('header starts unpinned, sits beside refresh, preserves drafts, and persists an explicit choice', async ({ page }) => {
  await page.goto('/?demo');
  const controls = page.locator('.panel-actions button');
  await expect(controls).toHaveCount(5);
  await expect(controls.nth(0)).toHaveAttribute('aria-label', 'Refresh usage');
  await expect(controls.nth(1)).toHaveAttribute('aria-label', 'Pin dashboard');
  await expect(page.getByRole('button', { name: 'Pin dashboard', exact: true })).toHaveAttribute('aria-pressed', 'false');
  await expect(page.getByRole('button', { name: 'Collapse dashboard', exact: true }).locator('.lucide-chevron-down')).toHaveCount(1);
  await expect(page.locator('.panel-actions .lucide-x')).toHaveCount(0);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('combobox', { name: 'Theme', exact: true }).selectOption('dark');
  await page.getByRole('button', { name: 'Pin dashboard', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Unpin dashboard', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('combobox', { name: 'Theme', exact: true })).toHaveValue('dark');
  await page.getByRole('button', { name: 'Save preferences', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Preferences saved');
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('opentoken.demo.settings') ?? '{}').dashboardPinned)).toBe(true);
  await page.reload();
  await expect(page.getByRole('button', { name: 'Unpin dashboard', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Unpin dashboard', exact: true }).focus(); await page.keyboard.press('Space');
  await expect(page.getByRole('button', { name: 'Pin dashboard', exact: true })).toHaveAttribute('aria-pressed', 'false');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBeTruthy();
});

test('failed pin writes roll back the pressed state instead of pretending it was saved', async ({ page }) => {
  await page.goto('/?demo');
  await expect(page.getByRole('button', { name: 'Pin dashboard', exact: true })).toBeEnabled();
  await page.evaluate(() => { const original = Storage.prototype.setItem; Storage.prototype.setItem = function(key, value) { if (key === 'opentoken.demo.settings') throw new DOMException('Synthetic full storage', 'QuotaExceededError'); original.call(this, key, value); }; });
  await page.getByRole('button', { name: 'Pin dashboard', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Dashboard pin could not be saved');
  await expect(page.getByRole('button', { name: 'Pin dashboard', exact: true })).toHaveAttribute('aria-pressed', 'false');
  await page.reload();
  await expect(page.getByRole('button', { name: 'Pin dashboard', exact: true })).toHaveAttribute('aria-pressed', 'false');
});

test('a previously saved explicit pin is respected despite the unpinned default', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('opentoken.demo.settings', JSON.stringify({ dashboardPinned: true, theme: 'dark' })));
  await page.goto('/?demo');
  await expect(page.getByRole('button', { name: 'Unpin dashboard', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('live tracking defaults on and its idle fallback preference persists', async ({ page }) => {
  await page.goto('/?demo'); await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const live = page.getByRole('switch', { name: /^Live tracking/ });
  await expect(live).toBeChecked(); await live.focus(); await page.keyboard.press('Space'); await expect(live).not.toBeChecked();
  await page.getByRole('combobox', { name: 'Idle quota refresh', exact: true }).selectOption('15');
  await page.getByRole('button', { name: 'Save preferences', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Preferences saved');
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('opentoken.demo.settings') ?? '{}').liveTracking)).toBe(false);
  await page.reload(); await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('switch', { name: /^Live tracking/ })).not.toBeChecked();
  await expect(page.getByRole('combobox', { name: 'Idle quota refresh', exact: true })).toHaveValue('15');
});

test('reference-style light overview visual snapshots', async ({ page }) => {
  await page.setViewportSize({ width: 454, height: 1200 }); await page.goto('/?demo');
  await page.getByRole('tab', { name: 'Claude', exact: true }).click();
  await expect(page.locator('.account-card')).toHaveCount(1);
  await page.screenshot({ path: 'artifacts/dashboard-light.png', fullPage: true });
  await page.getByRole('button', { name: 'Usage', exact: true }).click();
  await page.getByRole('combobox', { name: 'Usage provider', exact: true }).selectOption('claude');
  await page.screenshot({ path: 'artifacts/dashboard-spend-light.png', fullPage: true });
});
