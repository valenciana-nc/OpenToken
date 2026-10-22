import {test,expect} from '@playwright/test';
for(const c of [
 {name:'Devin',query:'devinSource=app',mode:'devin_app',source:'Devin app quota API',detail:'Devin app · unofficial quota RPC'},
 {name:'Antigravity',query:'antigravitySource=cli',mode:'antigravity_cli',source:'Antigravity CLI quota',detail:'Antigravity CLI · daily Cloud Code quota'},
 {name:'Antigravity',query:'antigravitySource=stored',mode:'antigravity_stored',source:'Antigravity IDE stored session',detail:'Antigravity IDE · stored session · daily Cloud Code quota'},
])test(`${c.name} ${c.mode} keeps actual source provenance across persistence and monitoring-only disconnect`,async({page})=>{
 await page.goto(`/?demo&${c.query}`);await page.getByRole('tab',{name:c.name,exact:true}).click();
 await expect(page.locator('input[type="password"]')).toHaveCount(0);
 await page.getByRole('button',{name:`Use existing ${c.name}`,exact:true}).click();
 await expect(page.locator('.provider-summary')).toContainText(c.source);
 let saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')??'{}'));
 expect(saved.connections[0].mode).toBe(c.mode);expect(JSON.stringify(saved)).not.toContain('access_token');expect(saved.dashboardPinned).toBe(false);
 await page.reload();await page.getByRole('tab',{name:c.name,exact:true}).click();await expect(page.locator('.provider-summary')).toContainText(c.source);
 await page.getByRole('button',{name:`Expand ${c.name} details`,exact:true}).click();await expect(page.locator('.account-details')).toContainText(c.detail);
 await page.getByRole('button',{name:`Manage ${c.name} accounts`,exact:true}).click();await page.getByRole('button',{name:/^Disconnect /}).last().click();
 await expect(page.getByRole('status')).toContainText('provider login was not changed');
 saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')??'{}'));expect(saved.connections).toEqual([]);
});
