import { writable } from 'svelte/store';
import { settingsOpen } from './stores';

export type SettingsTab = 'general' | 'appearance' | 'ai' | 'agent-access' | 'privacy' | 'plugins';

export const settingsTab = writable<SettingsTab>('general');

export function openSettings(tab: SettingsTab = 'general'): void {
    settingsTab.set(tab);
    settingsOpen.set(true);
}
