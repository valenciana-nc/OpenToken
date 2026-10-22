import type { Dispatch, SetStateAction } from 'react';
import type { Settings } from './types';
// Native navigation retains 'usage' for the quota flyout and the two settings sections.
export type Page = 'usage' | 'spending' | 'accounts' | 'settings';
export type Action = (run: () => Promise<unknown>, success?: string) => Promise<void>;
export interface PreferencesProps {
  settings: Settings;
  draft: Settings;
  setDraft: Dispatch<SetStateAction<Settings>>;
  save: (next?: Settings) => Promise<void>;
  action: Action;
  busy: boolean;
  connectionsChanged?: (next: Settings) => Promise<void>;
}
