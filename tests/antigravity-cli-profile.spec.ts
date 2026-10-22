import{test,expect}from'@playwright/test';
test('official CLI profile becomes visible, persists and disconnects without a guessed account',async({page})=>{
 await page.goto('/?demo');await page.getByRole('tab',{name:'Antigravity',exact:true}).click();
 page.once('dialog',async dialog=>{expect(dialog.message()).toContain('not a verified human account');await dialog.accept();});
 await page.getByRole('button',{name:'Monitor with agy CLI',exact:true}).click();
 await expect(page.getByRole('progressbar',{name:'Gemini · weekly remaining',exact:true})).toHaveAttribute('aria-valuenow','100');
 await expect(page.getByRole('progressbar',{name:'Third-party models · weekly remaining',exact:true})).toHaveAttribute('aria-valuenow','100');
 await expect(page.getByRole('progressbar',{name:/5 hours/})).toHaveCount(0);await expect(page.locator('.account-card')).toContainText('Antigravity CLI · current profile');
 let saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')??'{}'));expect(saved.connections[0].mode).toBe('antigravity_cli_usage');expect(saved.launchAtStartup).toBe(false);expect(saved.dashboardPinned).toBe(false);expect(JSON.stringify(saved)).not.toContain('@');
 await page.reload();await page.getByRole('tab',{name:'Antigravity',exact:true}).click();await page.getByRole('button',{name:'Expand Antigravity details',exact:true}).click();await expect(page.locator('.account-details')).toContainText('account identity not provided');
 await page.getByRole('button',{name:'Manage Antigravity accounts',exact:true}).click();await expect(page.locator('.identity-row').filter({hasText:'Official agy /usage'})).toContainText('identity not provided');await page.getByRole('button',{name:'Disconnect Antigravity CLI · current profile',exact:true}).last().click();
 saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')??'{}'));expect(saved.connections).toEqual([]);
});
test('declining profile monitoring cannot grant owner-side background renewal',async({page})=>{
 await page.goto('/?demo');await page.getByRole('tab',{name:'Antigravity',exact:true}).click();page.once('dialog',d=>d.dismiss());await page.getByRole('button',{name:'Monitor with agy CLI',exact:true}).click();await expect(page.getByRole('status',{name:'Antigravity CLI profile status'})).toContainText('cancelled');expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBeNull();await expect(page.locator('.account-card')).toHaveCount(0);
});
test('profile checking remains cancellable and does not adopt late completion',async({page})=>{
 await page.goto('/?demo&antigravityCli=waiting');await page.getByRole('tab',{name:'Antigravity',exact:true}).click();page.once('dialog',d=>d.accept());await page.getByRole('button',{name:'Monitor with agy CLI',exact:true}).click();await page.getByRole('button',{name:'Cancel CLI-profile check',exact:true}).click();await page.evaluate(()=>dispatchEvent(new CustomEvent('opentoken-demo-existing-ready',{detail:'antigravity'})));await expect(page.locator('.account-card')).toHaveCount(0);expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBeNull();
});
