// S3 Drive — main app state & wiring (mock; no network).
function App() {
  const { AppWindow, Toast, Menu, SegmentedControl, fileKind } = window.DS;
  const D = window.S3DATA;
  const [signedIn, setSignedIn] = React.useState(true);
  const [items, setItems] = React.useState(D.items);
  const [bucketName, setBucketName] = React.useState('acme-media-tokyo');
  const [view, setView] = React.useState('files');
  const [path, setPath] = React.useState('projects/2026/');
  const [hist, setHist] = React.useState({ back: ['projects/', ''], fwd: [] });
  const [sel, setSel] = React.useState(['projects/2026/report-q3.pdf']);
  const [mode, setMode] = React.useState('list');
  const [query, setQuery] = React.useState('');
  const [filtersOpen, setFiltersOpen] = React.useState(false);
  const noFilters = { kind: 'all', ext: '', size: 'all', date: 'all', sc: 'all' };
  const [filters, setFilters] = React.useState(noFilters);
  const [sort, setSort] = React.useState({ key: 'name', dir: 1 });
  const [tab, setTab] = React.useState('info');
  const [dialog, setDialog] = React.useState(null);
  const [menu, setMenu] = React.useState(null);
  const [toasts, setToasts] = React.useState([]);
  const [theme, setTheme] = React.useState('auto');
  const wrapRef = React.useRef(); const fileRef = React.useRef();
  const [w, setW] = React.useState(1300);
  const [inspectorOn, setInspectorOn] = React.useState(true);
  React.useEffect(() => { const ro = new ResizeObserver(([e]) => setW(e.contentRect.width)); ro.observe(wrapRef.current); return () => ro.disconnect(); }, [signedIn]);
  const narrow = w < 1100, compact = w < 900;

  React.useEffect(() => { if (theme === 'auto') document.documentElement.removeAttribute('data-theme'); else document.documentElement.setAttribute('data-theme', theme); }, [theme]);
  const bucket = D.buckets.find((b) => b.name === bucketName);
  const bItems = items.filter((i) => i.bucket === bucketName);
  const filterCount = Object.keys(noFilters).filter((k) => filters[k] !== noFilters[k]).length;
  const searching = view === 'files' && (query.trim() || filterCount > 0);

  const match = (it) => {
    const q = query.trim().toLowerCase();
    if (q && !it.name.toLowerCase().includes(q)) return false;
    if (it.folder) return !filterCount && !!q;
    const f = filters, mb = it.size / 1e6, days = (D.NOW - it.modified) / 864e5;
    if (f.kind !== 'all' && fileKind(it.name) !== f.kind) return false;
    if (f.ext && !it.name.toLowerCase().endsWith('.' + f.ext.replace(/^\./, '').toLowerCase())) return false;
    if (f.size === 'lt1' && mb >= 1) return false;
    if (f.size === '1to100' && (mb < 1 || mb > 100)) return false;
    if (f.size === 'gt100' && mb <= 100) return false;
    if (f.date === '7d' && days > 7) return false;
    if (f.date === '30d' && days > 30) return false;
    if (f.date === 'year' && it.modified.getFullYear() !== D.NOW.getFullYear()) return false;
    if (f.sc !== 'all' && it.sc !== f.sc) return false;
    return true;
  };
  let list = view === 'recent' ? items.filter((i) => !i.folder).sort((a, b) => b.modified - a.modified).slice(0, 12)
    : searching ? bItems.filter(match) : bItems.filter((i) => i.parent === path);
  if (view !== 'recent') {
    const k = sort.key;
    const val = (i) => k === 'name' ? i.name.toLowerCase() : k === 'size' ? (i.size || 0) : k === 'sc' ? (i.sc || '') : +i.modified;
    list = [...list].sort((a, b) => (b.folder - a.folder) || (val(a) > val(b) ? 1 : val(a) < val(b) ? -1 : 0) * sort.dir);
  }
  const selItems = items.filter((i) => sel.includes(i.key) && (view === 'recent' || i.bucket === bucketName));
  const descendants = (key) => bItems.filter((i) => i.key.startsWith(key) && !i.folder);
  const folderInfo = (() => {
    const it = selItems.length === 1 && selItems[0].folder ? selItems[0] : null;
    const key = it ? it.key : path;
    const d = descendants(key);
    return { key, name: it ? it.name : (path ? path.split('/').slice(-2)[0] : bucketName), count: bItems.filter((i) => i.parent === key).length, size: d.reduce((a, i) => a + i.size, 0) };
  })();

  const toast = (t) => { const id = Math.random(); setToasts((ts) => [...ts, { id, ...t }]); return id; };
  const patchToast = (id, p) => setToasts((ts) => ts.map((t) => (t.id === id ? { ...t, ...p } : t)));
  const dropToast = (id) => setToasts((ts) => ts.filter((t) => t.id !== id));
  const later = (id, ms = 3200) => setTimeout(() => dropToast(id), ms);
  const transfer = (title, desc, done, doneDesc, onDone) => {
    const id = toast({ icon: title.includes('アップ') ? 'upload' : 'download', title, description: desc, progress: 0 });
    let p = 0; const t = setInterval(() => { p += 9 + Math.random() * 14;
      if (p >= 100) { clearInterval(t); patchToast(id, { progress: undefined, icon: 'circle-check', tone: 'success', title: done, description: doneDesc }); onDone && onDone(); later(id); }
      else patchToast(id, { progress: p }); }, 180);
  };

  const navigate = (p) => { setHist((h) => ({ back: [...h.back, path], fwd: [] })); setPath(p); setSel([]); setQuery(''); setView('files'); };
  const nav = {
    canBack: hist.back.length > 0, canFwd: hist.fwd.length > 0,
    back: () => { const p = hist.back[hist.back.length - 1]; setHist({ back: hist.back.slice(0, -1), fwd: [path, ...hist.fwd] }); setPath(p); setSel([]); },
    fwd: () => { const p = hist.fwd[0]; setHist({ back: [...hist.back, path], fwd: hist.fwd.slice(1) }); setPath(p); setSel([]); },
  };
  const crumbs = [{ label: bucketName, icon: 'database' }, ...path.split('/').filter(Boolean).map((l) => ({ label: l }))];
  const onCrumb = (i) => { const p = i === 0 ? '' : path.split('/').filter(Boolean).slice(0, i).join('/') + '/'; if (p !== path) navigate(p); };

  const onSelect = (it, e = {}) => {
    setMenu(null);
    if (!it) return setSel([]);
    if (e.metaKey || e.ctrlKey) return setSel((s) => (s.includes(it.key) ? s.filter((k) => k !== it.key) : [...s, it.key]));
    if (e.shiftKey && sel.length) {
      const a = list.findIndex((i) => i.key === sel[sel.length - 1]), b = list.findIndex((i) => i.key === it.key);
      return setSel(list.slice(Math.min(a, b), Math.max(a, b) + 1).map((i) => i.key));
    }
    setSel([it.key]);
  };
  const onOpen = (it) => {
    if (it.folder) { if (view === 'recent') setBucketName(it.bucket); navigate(it.key); }
    else if (view === 'recent') { setBucketName(it.bucket); setView('files'); setPath(it.parent); setSel([it.key]); }
    else doAction('download');
  };
  const onContext = (e, it) => {
    const r = wrapRef.current.getBoundingClientRect();
    setMenu({ x: Math.min(e.clientX - r.left, r.width - 240), y: Math.min(e.clientY - r.top, r.height - 220), it });
  };
  const upload = (files) => {
    if (!files.length) return;
    const now = new Date();
    const added = files.map((f) => ({ bucket: bucketName, parent: path, name: f.name, key: path + f.name, size: f.size || 1000, modified: now, created: now, sc: 'STANDARD',
      contentType: f.type || 'application/octet-stream', etag: D.vid().toLowerCase().replace(/[^a-z0-9]/g, '').padEnd(32, '0').slice(0, 32), versions: [{ id: D.vid(), date: now, size: f.size || 1000, latest: true }] }));
    const total = files.reduce((a, f) => a + f.size, 0);
    transfer(files.length + ' 件をアップロード中', files[0].name + (files.length > 1 ? ' ほか' : '') + ' · ' + D.fmtSize(total), 'アップロードが完了しました', files.length + ' 件 → ' + bucketName + '/' + path,
      () => { setItems((its) => [...its.filter((i) => !added.some((a) => a.key === i.key && a.bucket === i.bucket)), ...added]); setSel(added.map((a) => a.key)); });
  };
  const doAction = (id, targets) => {
    setMenu(null);
    const t = targets || selItems;
    if (id === 'upload') return fileRef.current.click();
    if (id === 'newfolder') return setDialog({ type: 'newfolder' });
    if (!t.length) return;
    if (id === 'open') return onOpen(t[0]);
    if (id === 'versions') { setTab('versions'); return; }
    if (id === 'download') {
      const size = t.reduce((a, i) => a + (i.folder ? descendants(i.key).reduce((s, x) => s + x.size, 0) : i.size), 0);
      return transfer(t.length === 1 ? '「' + t[0].name + '」をダウンロード中' : t.length + ' 件をダウンロード中', D.fmtSize(size), 'ダウンロードが完了しました', '~/Downloads に保存しました');
    }
    setDialog({ type: id, items: t });
  };
  const apply = {
    newfolder: (name) => { setItems((its) => [...its, { bucket: bucketName, parent: path, name, key: path + name + '/', folder: true, modified: new Date() }]); setSel([path + name + '/']); setDialog(null); later(toast({ icon: 'folder-plus', tone: 'success', title: '「' + name + '」を作成しました' })); },
    delete: (all) => { const ks = dialog.items.map((i) => i.key); setItems((its) => its.filter((i) => !(i.bucket === bucketName && ks.some((k) => i.key === k || (k.endsWith('/') && i.key.startsWith(k)))))); setSel([]); setDialog(null);
      later(toast({ icon: 'trash-2', title: dialog.items.length + ' 項目を削除しました', description: all ? '全バージョンを削除' : '削除マーカーを作成' })); },
    move: (dest) => { const roots = dialog.items;
      setItems((its) => its.map((i) => { if (i.bucket !== bucketName) return i; const r = roots.find((m) => i.key === m.key || (m.folder && i.key.startsWith(m.key))); if (!r) return i;
        return { ...i, key: dest + i.key.slice(r.parent.length), parent: dest + i.parent.slice(r.parent.length) }; }));
      setSel([]); setDialog(null); later(toast({ icon: 'folder-input', tone: 'success', title: roots.length + ' 項目を移動しました', description: '→ ' + (dest || '/') })); },
    class: (sc) => { const ks = dialog.items.map((i) => i.key); setItems((its) => its.map((i) => (!i.folder && i.bucket === bucketName && ks.some((k) => i.key === k || (k.endsWith('/') && i.key.startsWith(k))) ? { ...i, sc } : i)));
      setDialog(null); later(toast({ icon: 'layers', tone: 'success', title: 'ストレージクラスを変更しました', description: window.DS.STORAGE_CLASSES[sc].label })); },
  };
  const versionOp = (item, fn) => setItems((its) => its.map((i) => (i === item ? fn(i) : i)));
  const restore = (v) => { const it = selItems[0]; const now = new Date();
    versionOp(it, (i) => ({ ...i, size: v.size, modified: now, versions: [{ ...v, id: D.vid(), date: now, latest: true }, ...i.versions.map((x) => ({ ...x, latest: false }))] }));
    later(toast({ icon: 'rotate-ccw', tone: 'success', title: 'バージョンを復元しました', description: D.fmtDate(v.date) + ' の内容が最新になりました' })); };
  const delVersion = (v) => { const it = selItems[0];
    versionOp(it, (i) => { const vs = i.versions.filter((x) => x.id !== v.id); if (v.latest) vs[0] = { ...vs[0], latest: true }; return { ...i, versions: vs, size: vs[0].size, modified: vs[0].date }; });
    later(toast({ icon: 'trash-2', title: 'バージョンを完全に削除しました' })); };

  React.useEffect(() => {
    const onKey = (e) => {
      if (/INPUT|TEXTAREA|SELECT/.test(e.target.tagName) || dialog) return;
      if (e.metaKey && (e.key === 'Backspace' || e.key === 'Delete') && sel.length) { e.preventDefault(); doAction('delete'); }
      if (e.metaKey && e.key === 'a') { e.preventDefault(); setSel(list.map((i) => i.key)); }
      if (e.key === 'Escape') setMenu(null);
    };
    const onDown = () => setMenu(null);
    window.addEventListener('keydown', onKey); window.addEventListener('mousedown', onDown);
    return () => { window.removeEventListener('keydown', onKey); window.removeEventListener('mousedown', onDown); };
  });

  const menuItems = menu && (menu.it ? [
    ...(menu.it.folder ? [{ id: 'open', label: '開く', icon: 'folder-open' }] : []),
    { id: 'download', label: 'ダウンロード', icon: 'download', shortcut: '⌘D' },
    { id: 'move', label: '移動…', icon: 'folder-input' },
    { id: 'class', label: 'ストレージクラスを変更…', icon: 'layers' },
    ...(!menu.it.folder && selItems.length === 1 ? [{ id: 'versions', label: 'バージョン履歴', icon: 'history' }] : []),
    { separator: true },
    { id: 'delete', label: selItems.length > 1 ? selItems.length + ' 項目を削除' : '削除', icon: 'trash-2', destructive: true, shortcut: '⌘⌫' },
  ] : menu.account ? [
    { id: 'settings', label: '設定…', icon: 'settings', shortcut: '⌘,' },
    { id: 'creds', label: '認証情報を更新…', icon: 'key-round' },
    { separator: true },
    { id: 'signout', label: 'サインアウト', icon: 'log-out' },
  ] : [
    { id: 'newfolder', label: '新規フォルダ', icon: 'folder-plus', shortcut: '⇧⌘N' },
    { id: 'upload', label: 'アップロード…', icon: 'upload', shortcut: '⌘U' },
  ]);
  const onMenu = (m) => { if (m.id === 'signout') { setMenu(null); setSignedIn(false); return; } if (m.id === 'settings' || m.id === 'creds') return setMenu(null); doAction(m.id); };

  const selSize = selItems.filter((i) => !i.folder).reduce((a, i) => a + i.size, 0);
  const status = list.length + ' 項目' + (sel.length ? ' · ' + sel.length + ' 項目を選択' + (selSize ? '（' + D.fmtSize(selSize) + '）' : '') : '') + (view === 'files' && !searching ? ' · ' + bucket.regionShort : '');
  const folders = bItems.filter((i) => i.folder).sort((a, b) => (a.key > b.key ? 1 : -1));
  const showFiles = view !== 'dashboard';
  const showInspector = showFiles && inspectorOn && (!narrow || sel.length > 0);
  const overlayInspector = showInspector && compact;

  const inspectorEl = <Inspector item={selItems.length === 1 ? selItems[0] : null} count={selItems.length} folderInfo={folderInfo} bucket={bucket} tab={tab} onTab={setTab}
    onAction={(a) => doAction(a)} onRestore={restore} onDeleteVersion={delVersion} onDownloadVersion={(v) => transfer('「' + selItems[0].name + '」をダウンロード中', 'バージョン ' + D.fmtDate(v.date), 'ダウンロードが完了しました', '~/Downloads に保存しました')} />;
  const win = !signedIn ? <SignIn onDone={() => setSignedIn(true)} /> : (
    <AppWindow width="100%" height="100%"
      sidebar={<Sidebar view={view} bucket={bucketName} onView={(v) => { setView(v); setSel([]); }} onBucket={(b) => { setBucketName(b); setView('files'); setPath(''); setSel([]); setHist({ back: [], fwd: [] }); }}
        onAccount={(e) => { e.stopPropagation(); const r = wrapRef.current.getBoundingClientRect(); const br = e.currentTarget.getBoundingClientRect(); setMenu({ account: true, x: br.left - r.left, y: br.top - r.top - 130 }); }} />}
      toolbar={<Toolbar view={view} nav={nav} crumbs={crumbs} onCrumb={onCrumb} query={query} onQuery={setQuery} filtersOpen={filtersOpen} filterCount={filterCount} onToggleFilters={() => setFiltersOpen(!filtersOpen)}
        mode={mode} onMode={setMode} narrow={narrow} inspectorOn={showInspector} onToggleInspector={() => setInspectorOn(!inspectorOn)} onNewFolder={() => doAction('newfolder')} onUpload={() => doAction('upload')} bucket={bucketName} onBucket={setBucketName}
        onRefresh={() => later(toast({ icon: 'refresh-cw', title: 'メトリクスを更新しました', description: 'CloudWatch · Cost Explorer' }))} />}
      inspector={showInspector && !overlayInspector ? inspectorEl : null}>
      {showFiles ? (
        <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
          {filtersOpen && view === 'files' ? <FilterBar filters={filters} onChange={setFilters} onClear={() => { setFilters(noFilters); setQuery(''); }} count={list.length} /> : null}
          {searching && !filtersOpen ? <div style={{ padding: '6px 16px', fontSize: 12, color: 'var(--muted-foreground)', borderBottom: '0.5px solid var(--border)' }}>{bucketName} 全体を検索中 · {list.length} 件</div> : null}
          <FileList items={list} mode={mode} selected={sel} onSelect={onSelect} onOpen={onOpen} onContext={onContext} sort={sort}
            onSort={(k) => setSort((s) => ({ key: k, dir: s.key === k ? -s.dir : 1 }))} showPath={searching || view === 'recent'} narrow={narrow}
            onDropFiles={view === 'files' ? upload : null} dropLabel={'ドロップして ' + bucketName + '/' + path + ' にアップロード'} status={status}
            empty={searching ? { icon: 'search-x', text: '一致する項目はありません' } : { icon: 'cloud-upload', text: 'ファイルをドロップしてアップロード' }} />
        </div>
      ) : <Dashboard bucket={bucket} />}
      {overlayInspector ? <div onMouseDown={(e) => e.stopPropagation()} style={{ position: 'absolute', top: 0, right: 0, bottom: 0, width: 'min(300px, 85%)', overflow: 'auto', background: 'var(--background)', boxShadow: 'var(--shadow-md)', borderLeft: '0.5px solid var(--border)', zIndex: 5 }}>{inspectorEl}</div> : null}
    </AppWindow>
  );

  return (
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column', padding: '16px 24px 24px', gap: 12 }}>
      <div style={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', gap: 8, fontSize: 12, color: 'var(--muted-foreground)' }}>
        外観<SegmentedControl value={theme} onChange={setTheme} options={[{ value: 'auto', label: '自動' }, { value: 'light', label: 'ライト' }, { value: 'dark', label: 'ダーク' }]} />
      </div>
      <div ref={wrapRef} style={{ flex: 1, minHeight: 0, position: 'relative', maxWidth: 1320, width: '100%', margin: '0 auto' }}>
        {win}
        {dialog ? (
          <div style={{ position: 'absolute', inset: 0, borderRadius: 12, overflow: 'hidden', zIndex: 40 }}>
            {dialog.type === 'newfolder' ? <NewFolderDialog existing={bItems.filter((i) => i.parent === path).map((i) => i.name)} onCancel={() => setDialog(null)} onCreate={apply.newfolder} /> : null}
            {dialog.type === 'delete' ? <DeleteDialog items={dialog.items} versioning={bucket.versioning} onCancel={() => setDialog(null)} onDelete={apply.delete} /> : null}
            {dialog.type === 'move' ? <MoveDialog folders={folders} moving={dialog.items} onCancel={() => setDialog(null)} onMove={apply.move} /> : null}
            {dialog.type === 'class' ? <StorageClassDialog items={dialog.items} onCancel={() => setDialog(null)} onApply={apply.class} /> : null}
          </div>
        ) : null}
        {menu ? <div onMouseDown={(e) => e.stopPropagation()} style={{ position: 'absolute', left: menu.x, top: menu.y, zIndex: 50 }}><Menu items={menuItems} onSelect={onMenu} /></div> : null}
        <div style={{ position: 'absolute', right: 16, bottom: 16, display: 'flex', flexDirection: 'column', gap: 8, zIndex: 45 }}>
          {toasts.map((t) => <Toast key={t.id} {...t} onClose={() => dropToast(t.id)} />)}
        </div>
      </div>
      <input ref={fileRef} type="file" multiple style={{ display: 'none' }} onChange={(e) => { upload(Array.from(e.target.files)); e.target.value = ''; }} />
    </div>
  );
}
window.App = App;
