import { test, expect } from '@playwright/test';
import {openAccountSetup}from './helpers/recovery';
for (const [name, field] of [['Z.ai','Z.ai API key'],['DeepSeek','DeepSeek API key'],['Kimi','Kimi Code API key'],['OpenCode','OpenCode Go API key']] as const) {
  test(`${name} connects actual capability samples, persists and removes without borrowing local history`, async ({ page }) => {
    await page.goto('/?demo'); await page.getByRole('tab',{name,exact:true}).click();
    await expect(page.getByLabel(field,{exact:true})).toHaveAttribute('type','password');
    await page.getByLabel(field,{exact:true}).fill('opentoken-demo-token');await page.getByRole('button',{name:'Verify & connect',exact:true}).click();
    await expect(page.locator('.account-card')).toHaveCount(1);await expect(page.locator('.token-summary,.donut')).toHaveCount(0);
    if(name==='DeepSeek'){await expect(page.getByRole('progressbar')).toHaveCount(0);await expect(page.locator('.account-card')).toContainText('12.50');}else{await expect(page.getByRole('progressbar')).toHaveAttribute('aria-valuenow','80');}
    expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).not.toContain('opentoken-demo-token');
    await page.reload();await page.getByRole('tab',{name,exact:true}).click();await expect(page.locator('.account-card')).toHaveCount(1);
    await page.getByRole('button',{name:`Manage ${name} accounts`,exact:true}).click();
    const provider={ 'Z.ai':'zai',DeepSeek:'deepseek',Kimi:'kimi',OpenCode:'opencode'}[name];
    await page.getByRole('button',{name:`Disconnect ${provider} · sample API key`,exact:true}).click();
    await page.getByRole('button',{name:'Back to providers',exact:true}).click();await expect(page.locator('.account-card')).toHaveCount(0);
  });
  test(`${name} rejects invalid credentials without pretending to connect`,async({page})=>{
    await page.goto('/?demo');await page.getByRole('tab',{name,exact:true}).click();
    await page.getByLabel(field,{exact:true}).fill('invalid-synthetic-token');await page.getByRole('button',{name:'Verify & connect',exact:true}).click();
    await expect(page.getByRole('alert')).toContainText('Sample verification failed');await expect(page.getByLabel(field,{exact:true})).toHaveValue('');await expect(page.locator('.account-card')).toHaveCount(0);
  });
}
test('Copilot defaults to GitHub CLI sign-in and reports supplied quota rather than personal billing',async({page})=>{
  await page.goto('/?demo');await page.getByRole('tab',{name:'Copilot',exact:true}).click();
  await expect(page.locator('input[type="password"]')).toHaveCount(0);await page.getByRole('button',{name:'Use existing Copilot',exact:true}).click();
  await expect(page.getByRole('progressbar',{name:'Premium requests remaining',exact:true})).toHaveAttribute('aria-valuenow','72');
  await expect(page.locator('.provider-summary')).toContainText('Unofficial Copilot quota');await page.reload();await page.getByRole('tab',{name:'Copilot',exact:true}).click();await expect(page.locator('.account-card')).toContainText('github-sample');
  expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).not.toContain('accessToken');
});
test('GitHub sign-in waits across navigation and cancel ignores late completion',async({page})=>{
  await page.goto('/?demo&githubLogin=waiting');await page.getByRole('tab',{name:'Copilot',exact:true}).click();await openAccountSetup(page);await page.getByRole('button',{name:'Sign in with GitHub',exact:true}).click();
  await page.getByRole('button',{name:'Cancel sign-in',exact:true}).click();await page.evaluate(()=>window.dispatchEvent(new Event('opentoken-demo-github-ready')));await expect(page.locator('.account-card')).toHaveCount(0);
  await page.getByRole('button',{name:'Sign in with GitHub',exact:true}).click();await page.getByRole('tab',{name:'Codex',exact:true}).click();await page.evaluate(()=>window.dispatchEvent(new Event('opentoken-demo-github-ready')));
  await page.getByRole('tab',{name:'Copilot',exact:true}).click();await expect(page.getByRole('progressbar')).toHaveAttribute('aria-valuenow','72');
});
test('region changes discard unsaved keys and keep the selected region on the sample connection',async({page})=>{
  await page.goto('/?demo');await page.getByRole('tab',{name:'Kimi',exact:true}).click();await page.getByLabel('Kimi Code API key',{exact:true}).fill('do-not-retain-this-value');
  await page.getByRole('combobox',{name:'Region',exact:true}).selectOption('international');await expect(page.getByLabel('Kimi Code API key',{exact:true})).toHaveValue('');
  await page.getByLabel('Kimi Code API key',{exact:true}).fill('opentoken-demo-token');await page.getByRole('button',{name:'Verify & connect',exact:true}).click();
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')!).connections[0].scope)).toContain('international');
});
