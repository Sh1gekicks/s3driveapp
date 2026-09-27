Mockup of the Tauri window (titleBarStyle "Overlay", hiddenTitle, sidebar vibrancy). The toolbar IS the titlebar. In production the OS draws the traffic lights — this component only mocks them.
```jsx
<AppWindow sidebar={<Sidebar/>} toolbar={<Toolbar/>} inspector={<Inspector/>}>…</AppWindow>
```
