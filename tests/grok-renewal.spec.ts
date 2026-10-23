import {test,expect,type Page} from '@playwright/test';
import {openAccountSetup} from './helpers/recovery';
async function saved(page:Page,query=''){
 await page.goto(`/?demo${query}`);await page.getByRole('tab',{name:'Grok',exact:true}).click();
 await page.getByRole('button',{name:'Use existing Grok',exact:true}).click();
 await expect(page.getByRole('progressbar')).toHaveAttribute('aria-valuenow','75');
 await page.getByRole('button',{name:'Manage Grok accounts',exact:true}).click();await openAccountSetup(page);
}
test('Grok device sign-in explains code direction; browser fallback requires the full URL, never a frontend code field',async({page})=>{
 await page.goto('/?demo&grokLogin=waiting');await page.getByRole('tab',{name:'Grok',exact:true}).click();await openAccountSetup(page);
 await page.getByRole('button',{name:'Sign in with Grok',exact:true}).click();
 await expect(page.getByRole('status',{name:'Grok sign-in status'})).toContainText('device code');
 await expect(page.getByRole('status',{name:'Grok sign-in status'})).toContainText('Enter that code on the official page');
 await expect(page.getByRole('button',{name:'Enable automatic Grok renewal…',exact:true})).toBeDisabled();
 await page.getByRole('button',{name:'Cancel sign-in',exact:true}).click();
 await page.getByText('Browser return-code help',{exact:true}).click();
 await expect(page.getByText('full return URL',{exact:true})).toBeVisible();
 await page.getByRole('button',{name:'Use browser sign-in instead',exact:true}).click();
 await expect(page.getByRole('status',{name:'Grok sign-in status'})).toContainText('FULL return URL');
 await expect(page.locator('.cursor-sign-in input')).toHaveCount(0);
 await page.getByRole('button',{name:'Cancel sign-in',exact:true}).click();
 expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBeNull();
});
test('Grok renewal decline leaves the exact existing connection and settings untouched',async({page})=>{
 await saved(page);const before=await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'));
 page.once('dialog',async d=>{expect(d.message()).toContain('Background grok models');await d.dismiss();});
 await page.getByRole('button',{name:'Enable automatic Grok renewal…',exact:true}).click();
 await expect(page.getByRole('alert')).toContainText('cancelled');
 expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBe(before);
});
test('approved renewal changes only the same connection mode, survives navigation and can be revoked by read-only reuse',async({page})=>{
 await saved(page);const before=await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')!));
 page.once('dialog',async d=>d.accept());await page.getByRole('button',{name:'Enable automatic Grok renewal…',exact:true}).click();
 await expect(page.getByRole('status',{name:'Grok renewal status'})).toContainText('enabled');
 const after=await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')!));
 expect(after).toEqual({...before,connections:before.connections.map((c:Record<string,unknown>)=>({...c,mode:'grok_cli_renewal'}))});
 await page.reload();await expect(page.locator('body')).not.toContainText('synthetic-grok-token');
 await page.getByRole('tab',{name:'Grok',exact:true}).click();await expect(page.getByRole('progressbar')).toHaveAttribute('aria-valuenow','75');
 await page.getByRole('button',{name:'Expand Grok details',exact:true}).click();await expect(page.locator('.account-details')).toContainText('official CLI renewal approved');
 await page.getByRole('button',{name:'Manage Grok accounts',exact:true}).click();await page.getByRole('button',{name:'Use existing Grok',exact:true}).click();
 const revoked=await page.evaluate(()=>JSON.parse(localStorage.getItem('opentoken.demo.settings')!));expect(revoked).toEqual(before);
});
test('cancelled renewal never stores a grant or replaces the old connection',async({page})=>{
 await saved(page,'&grokRenewal=waiting');const before=await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'));
 page.once('dialog',async d=>d.accept());await page.getByRole('button',{name:'Enable automatic Grok renewal…',exact:true}).click();
 await page.getByRole('button',{name:'Cancel renewal check',exact:true}).click();
 await expect(page.getByRole('alert')).toContainText('cancelled');
 expect(await page.evaluate(()=>localStorage.getItem('opentoken.demo.settings'))).toBe(before);
});
