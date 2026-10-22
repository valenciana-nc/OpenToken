import {test,expect} from '@playwright/test';
for(const [provider,name,mode,source] of [
 ['antigravity','Antigravity','antigravity_app','Antigravity local RPC'],
 ['devin','Devin','devin_cli','Devin CLI quota API'],
 ['muse','Muse Code','muse_cli','Muse last observation'],
] as const){
 test(`${name} existing native-source flow persists, explains scope, and disconnects monitoring only`,async({page})=>{
  await page.goto('/?demo');await page.getByRole('tab',{name,exact:true}).click();
  await expect(page.locator('input[type="password"]')).toHaveCount(0);await expect(page.getByRole('button',{name:/Sign in with/})).toHaveCount(0);
  await page.getByRole('button',{name:`Use existing ${name}`,exact:true}).click();await expect(page.locator('.account-card')).toHaveCount(1);await expect(page.locator('.provider-summary')).toContainText(source);await expect(page.getByRole('progressbar').first()).toHaveAttribute('aria-valuenow',provider==='devin'?'70':'75');
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')??'{}'));expect(saved.connections).toHaveLength(1);expect(saved.connections[0].mode).toBe(mode);expect(JSON.stringify(saved)).not.toContain('access_token');expect(saved.launchAtStartup).toBe(false);expect(saved.dashboardPinned).toBe(false);
  await page.reload();await page.getByRole('tab',{name,exact:true}).click();await expect(page.locator('.account-card')).toHaveCount(1);
  await page.getByRole('button',{name:`Expand ${name} details`,exact:true}).click();await expect(page.locator('.account-details')).toContainText(provider==='muse'?'last-observed':provider==='devin'?'credential scope':'owned local quota');
  await page.getByRole('button',{name:`Manage ${name} accounts`,exact:true}).click();await page.getByRole('button',{name:/^Disconnect /}).last().click();await expect(page.getByRole('status')).toContainText('provider login was not changed');
  expect((await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')??'{}'))).connections).toEqual([]);
 });
}
test('Devin hidden daily field never becomes a weekly percent-left or a progress bar',async({page})=>{
 await page.goto('/?demo&devinQuota=hidden');await page.getByRole('tab',{name:'Devin',exact:true}).click();await page.getByRole('button',{name:'Use existing Devin',exact:true}).click();await expect(page.locator('.value-row')).toContainText('window unverified');await expect(page.locator('.value-row')).toContainText('100 raw %');await expect(page.getByRole('progressbar')).toHaveCount(0);await expect(page.getByRole('tab',{name:'Devin',exact:true})).not.toHaveAttribute('title',/100% left/);
});
test('Muse cold start is unavailable, not a synthetic full subscription or a silent mint',async({page})=>{
 await page.goto('/?demo&museExisting=empty');await page.getByRole('tab',{name:'Muse Code',exact:true}).click();await page.getByRole('button',{name:'Use existing Muse Code',exact:true}).click();await expect(page.getByRole('alert')).toContainText('no subscription observation');await expect(page.getByRole('alert')).toContainText('No prompt, login or key-mint');await expect(page.locator('.account-card')).toHaveCount(0);await expect(page.getByRole('progressbar')).toHaveCount(0);expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBeNull();await expect(page.getByRole('button',{name:'Track local usage',exact:true})).toBeEnabled();
});
test('cancelling an existing source check blocks late completion without launching recovery',async({page})=>{
 await page.goto('/?demo&museExisting=waiting');await page.getByRole('tab',{name:'Muse Code',exact:true}).click();await page.getByRole('button',{name:'Use existing Muse Code',exact:true}).click();await page.getByRole('button',{name:'Cancel source check',exact:true}).click();await expect(page.getByRole('status',{name:'Muse Code source status',exact:true})).toContainText('Source check cancelled');await page.evaluate(()=>dispatchEvent(new CustomEvent('opentoken-demo-existing-ready',{detail:'muse'})));await expect(page.locator('.account-card')).toHaveCount(0);expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBeNull();await expect(page.getByRole('button',{name:/Sign in with/})).toHaveCount(0);
});
