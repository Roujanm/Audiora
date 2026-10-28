import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';

// In the installed app the UI must feel like an app, not a web page: no browser context menu
// ("Save image", "Inspect"…) except in text fields, and no reload / print / find / view-source
// shortcuts. The dev server keeps them for debugging.
if (import.meta.env.PROD) {
  document.addEventListener('contextmenu', (e) => {
    const t = e.target as HTMLElement | null;
    if (!t?.closest('input, textarea, [contenteditable="true"]')) e.preventDefault();
  });
  document.addEventListener(
    'keydown',
    (e) => {
      const k = e.key.toLowerCase();
      const ctrl = e.ctrlKey || e.metaKey;
      const blocked =
        k === 'f5' ||
        k === 'f7' ||
        k === 'f12' ||
        (e.altKey && (k === 'arrowleft' || k === 'arrowright')) ||
        (ctrl && ['r', 'p', 'f', 'g', 'u', 'j', 's', 'h', '+', '-', '=', '0'].includes(k)) ||
        (ctrl && e.shiftKey && ['i', 'c', 'j'].includes(k));
      if (blocked) e.preventDefault();
    },
    { capture: true },
  );
  // dragging a picture or link out of the window would hand it to the browser
  document.addEventListener('dragstart', (e) => e.preventDefault());
  // ctrl + mouse wheel would zoom the whole window like a browser page
  document.addEventListener('wheel', (e) => e.ctrlKey && e.preventDefault(), { passive: false });
}

const app = mount(App, { target: document.getElementById('app')! });

export default app;
