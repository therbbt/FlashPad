import './app.css';
import { mount, type Component } from 'svelte';

const windowKind = new URLSearchParams(window.location.search).get('window');

const loadRoot = (): Promise<{ default: Component }> => {
  if (windowKind === 'settings') return import('./SettingsWindow.svelte');
  if (windowKind === 'database-manager') return import('./DatabaseWindow.svelte');
  return import('./App.svelte');
};

void loadRoot().then(({ default: Root }) => {
  mount(Root, {
    target: document.getElementById('app')!,
  });
});
