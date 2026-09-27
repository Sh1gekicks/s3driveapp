// Exposes the design-system namespace as window.DS.
// 1) Uses the compiled bundle (_ds_bundle.js) when present.
// 2) Otherwise falls back to compiling components/*.jsx in-browser (needs React + Babel loaded first).
(function () {
  if (window.DS) return;
  for (const k of Object.keys(window)) {
    try { const v = window[k]; if (v && typeof v === 'object' && typeof v.Button === 'function' && typeof v.StorageClassBadge === 'function') { window.DS = v; return; } } catch (e) {}
  }
  if (!window.Babel || !window.React) return;
  const base = new URL('.', document.currentScript.src).href;
  const files = ['core/Icon', 'core/Button', 'core/IconButton', 'forms/Input', 'forms/Select', 'forms/Checkbox', 'forms/Switch', 'forms/SegmentedControl',
    'feedback/Badge', 'feedback/StorageClassBadge', 'feedback/Progress', 'feedback/Toast', 'feedback/Tooltip', 'feedback/Dialog',
    'navigation/Menu', 'navigation/SidebarItem', 'navigation/Breadcrumbs', 'data/FileIcon', 'shell/AppWindow'];
  const cache = {};
  function load(url) {
    if (cache[url]) return cache[url].exports;
    const xhr = new XMLHttpRequest(); xhr.open('GET', url, false); xhr.send();
    const code = Babel.transform(xhr.responseText, { presets: ['react', ['env', { modules: 'commonjs' }]], filename: url }).code;
    const mod = { exports: {} }; cache[url] = mod;
    const req = (p) => p === 'react' ? window.React : load(new URL(p, url).href);
    new Function('require', 'module', 'exports', 'React', code)(req, mod, mod.exports, window.React);
    return mod.exports;
  }
  const ns = {};
  files.forEach((f) => Object.assign(ns, load(base + 'components/' + f + '.jsx')));
  window.DS = ns;
})();
