import type {Page} from '@playwright/test';
/** Tests of authentication deliberately enter recovery; ordinary monitoring must not. */
export async function openAccountSetup(page:Page){const button=page.getByRole('button',{name:'Account setup (only if needed)',exact:true});if(await button.isVisible().catch(()=>false))await button.click();}
