import{test,expect}from'@playwright/test';
import{openAccountSetup}from'./helpers/recovery';
test('Antigravity owner recovery is explicit, cancellable, survives navigation and verifies before persistence',async({page})=>{
 await page.goto('/?demo&antigravityRecovery=waiting');await page.getByRole('tab',{name:'Antigravity',exact:true}).click();
 expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBeNull();
 if(await page.locator('details[open]',{hasText:'Sign in or recover session'}).count()===0)await page.getByText('Sign in or recover session',{exact:true}).click();await page.getByRole('button',{name:'Recover with Antigravity IDE',exact:true}).click();
 await expect(page.getByRole('status',{name:'Antigravity recovery status'})).toContainText('selected official client');await expect(page.getByRole('button',{name:'Recover with Antigravity IDE',exact:true})).toBeDisabled();
 await page.getByRole('button',{name:'Cancel recovery',exact:true}).click();await page.evaluate(()=>dispatchEvent(new Event('opentoken-demo-antigravity-ready')));await expect(page.locator('.account-card')).toHaveCount(0);
 await page.getByRole('button',{name:'Recover with Antigravity IDE',exact:true}).click();await page.getByRole('tab',{name:'Codex',exact:true}).click();await page.evaluate(()=>dispatchEvent(new Event('opentoken-demo-antigravity-ready')));await page.getByRole('tab',{name:'Antigravity',exact:true}).click();
 await expect(page.locator('.account-card')).toHaveCount(1);await expect(page.getByRole('progressbar',{name:'Gemini · 5 hours remaining'})).toHaveAttribute('aria-valuenow','75');
 const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')??'{}'));expect(saved.connections[0].provider).toBe('antigravity');expect(saved.launchAtStartup).toBe(false);expect(saved.dashboardPinned).toBe(false);expect(JSON.stringify(saved)).not.toContain('access_token');
});
test('Grok owner model check never silently falls through to new OAuth when it cannot restore access',async({page})=>{
 await page.goto('/?demo&grokRecovery=rejected');await page.getByRole('tab',{name:'Grok',exact:true}).click();await openAccountSetup(page);await page.getByRole('button',{name:'Recover Grok session',exact:true}).click();
 await expect(page.getByRole('status',{name:'Grok sign-in status'})).toContainText('No new login was launched');await expect(page.getByRole('button',{name:'Sign in with Grok',exact:true})).toBeEnabled();await expect(page.locator('.account-card')).toHaveCount(0);expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBeNull();
});
test('Grok explicit model-only recovery can cancel without adopting late credentials',async({page})=>{
 await page.goto('/?demo&grokRecovery=waiting');await page.getByRole('tab',{name:'Grok',exact:true}).click();await openAccountSetup(page);await page.getByRole('button',{name:'Recover Grok session',exact:true}).click();await expect(page.getByRole('status',{name:'Grok sign-in status'})).toContainText('models check');await page.getByRole('button',{name:'Cancel sign-in',exact:true}).click();await page.evaluate(()=>dispatchEvent(new Event('opentoken-demo-grok-ready')));await expect(page.locator('.account-card')).toHaveCount(0);expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBeNull();
});
