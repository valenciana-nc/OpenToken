import type { Snapshot, Spending } from '../types';
import type { Page, PreferencesProps } from '../ui';
import type { ProviderId } from '../providers';
import AccountsPage from './AccountsPage';
import SettingsPage from './SettingsPage';

/** Both configuration sections share the same draft and connection transactions. */
export default function PreferencesPage({ section, navigate, snapshot, refreshing, provider, ...preferences }: PreferencesProps & {
  section: 'accounts' | 'settings'; navigate: (page: Page) => void;
  snapshot: Snapshot; refreshing: boolean; provider: ProviderId; pricing: Spending | null;
}) {
  return <div className="preferences-page">
    <h1>Providers & settings</h1>
    <div className="period-control settings-sections" role="group" aria-label="Settings sections">
      <button aria-pressed={section === 'accounts'} aria-controls="settings-content" onClick={() => navigate('accounts')}>Providers</button>
      <button aria-pressed={section === 'settings'} aria-controls="settings-content" onClick={() => navigate('settings')}>App settings</button>
    </div>
    <div id="settings-content">
      {section === 'accounts'
        ? <AccountsPage {...preferences} snapshot={snapshot} refreshing={refreshing} initialProvider={provider}/>
        : <SettingsPage {...preferences}/>}
    </div>
  </div>;
}
