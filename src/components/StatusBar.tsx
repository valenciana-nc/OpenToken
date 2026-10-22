import { ArrowLeft, ChartPie, ChevronDown, Pin, RefreshCw, Settings2 } from 'lucide-react';
import type { Page } from '../ui';
import { drag, hide } from '../bridge';

/** App controls on the glass itself, not a simulated desktop/menu bar. */
export default function PanelHeader({ page, navigate, refreshing, refresh, pinned, pinDisabled, togglePin }: {
  page: Page; navigate: (page: Page) => void; refreshing: boolean; refresh: () => void;
  pinned: boolean; pinDisabled: boolean; togglePin: () => void;
}) {
  return <header className="panel-header" onMouseDown={e => { if (e.target === e.currentTarget) void drag(); }}>
    <div className="panel-heading" onMouseDown={e => { if (e.button === 0) void drag(); }}>
      <h1>OpenToken</h1>
    </div>
    <div className="panel-actions" aria-label="OpenToken controls">
      {page !== 'usage' && <button aria-label="Back to providers" title="Back to providers" onClick={() => navigate('usage')}><ArrowLeft size={16}/></button>}
      <button aria-label="Refresh usage" title="Refresh usage" disabled={refreshing} onClick={refresh}><RefreshCw size={16} className={refreshing ? 'spin' : ''}/></button>
      <button className={`pin-button${pinned ? ' active' : ''}`} aria-label={pinned ? 'Unpin dashboard' : 'Pin dashboard'} aria-pressed={pinned} title={pinned ? 'Unpin dashboard · dismiss when clicking another app' : 'Pin dashboard · stay open when switching apps'} disabled={pinDisabled} onClick={togglePin}><Pin size={16} fill={pinned ? 'currentColor' : 'none'} aria-hidden="true"/></button>
      <button aria-label="Usage" title="Usage · tokens & spending" aria-pressed={page === 'spending'} className={page === 'spending' ? 'active' : ''} onClick={() => navigate('spending')}><ChartPie size={17}/></button>
      <button aria-label="Settings" title="Providers & settings" aria-pressed={page === 'settings' || page === 'accounts'} className={page === 'settings' || page === 'accounts' ? 'active' : ''} onClick={() => navigate('settings')}><Settings2 size={17}/></button>
      <button aria-label="Collapse dashboard" title="Collapse dashboard" onClick={() => void hide()}><ChevronDown size={17} aria-hidden="true"/></button>
    </div>
  </header>;
}
