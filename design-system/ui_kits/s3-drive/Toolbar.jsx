// Unified toolbar (lives in the transparent titlebar) + search filter bar.
function Toolbar({ view, nav, crumbs, onCrumb, query, onQuery, filtersOpen, filterCount, onToggleFilters, mode, onMode, narrow, inspectorOn, onToggleInspector, onNewFolder, onUpload, bucket, onBucket, onRefresh }) {
  const { IconButton, Button, Breadcrumbs, Input, SegmentedControl, Select } = window.DS;
  const Sep = () => <span style={{ width: 0.5, height: 18, background: 'var(--border)', margin: '0 2px' }}></span>;
  if (view === 'dashboard') return (
    <>
      <div style={{ fontSize: 13, fontWeight: 600 }}>ストレージとコスト</div>
      <div data-tauri-drag-region="" style={{ flex: 1, alignSelf: 'stretch' }}></div>
      <Select size="sm" value={bucket} onChange={(e) => onBucket(e.target.value)} options={window.S3DATA.buckets.map((b) => b.name)} style={{ width: 180 }} />
      <IconButton icon="refresh-cw" label="更新" onClick={onRefresh} />
    </>
  );
  return (
    <>
      <div style={{ display: 'flex', gap: 2 }}>
        <IconButton icon="chevron-left" label="戻る" disabled={!nav.canBack} onClick={nav.back} />
        <IconButton icon="chevron-right" label="進む" disabled={!nav.canFwd} onClick={nav.fwd} />
      </div>
      {view === 'recent'
        ? <div style={{ fontSize: 13, fontWeight: 600, paddingLeft: 4 }}>最近の項目</div>
        : <Breadcrumbs items={crumbs} onNavigate={onCrumb} style={{ overflow: 'hidden', flex: '0 1 auto', minWidth: 60 }} />}
      <div data-tauri-drag-region="" style={{ flex: 1, alignSelf: 'stretch', minWidth: 12 }}></div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, flex: '0 1 auto', minWidth: 0 }}>
      <Input icon="search" placeholder="検索" value={query} onChange={(e) => onQuery(e.target.value)} style={{ width: 180, minWidth: 110, flex: '0 1 180px' }} />
      <IconButton icon="sliders-horizontal" label={'フィルタ' + (filterCount ? '（' + filterCount + '）' : '')} active={filtersOpen || filterCount > 0} onClick={onToggleFilters} />
      <SegmentedControl value={mode} onChange={onMode} options={[{ value: 'list', label: 'リスト', icon: 'list', iconOnly: true }, { value: 'grid', label: 'アイコン', icon: 'layout-grid', iconOnly: true }]} />
      <Sep />
      <IconButton icon="folder-plus" label="新規フォルダ ⇧⌘N" onClick={onNewFolder} disabled={view === 'recent'} />
      {narrow ? <IconButton icon="upload" label="アップロード ⌘U" variant="default" onClick={onUpload} disabled={view === 'recent'} />
        : <Button icon="upload" onClick={onUpload} disabled={view === 'recent'} style={{ flexShrink: 0 }}>アップロード</Button>}
      <IconButton icon="panel-right" label={inspectorOn ? 'インスペクタを隠す' : 'インスペクタを表示'} active={inspectorOn} onClick={onToggleInspector} />
      </div>
    </>
  );
}
function FilterBar({ filters, onChange, onClear, count }) {
  const { Select, Input, Button, STORAGE_CLASSES } = window.DS;
  const set = (k) => (e) => onChange({ ...filters, [k]: e.target.value });
  const kinds = Object.entries(window.S3DATA.KIND_LABEL).filter(([k]) => k !== 'folder').map(([value, label]) => ({ value, label }));
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 14px', borderBottom: '0.5px solid var(--border)', background: 'var(--muted)', flexWrap: 'wrap' }}>
      <span style={{ fontSize: 12, fontWeight: 500, color: 'var(--muted-foreground)' }}>絞り込み</span>
      <Select size="sm" value={filters.kind} onChange={set('kind')} options={[{ value: 'all', label: 'すべての種類' }, ...kinds]} style={{ width: 130 }} />
      <Input size="sm" placeholder="拡張子（例: pdf）" value={filters.ext} onChange={set('ext')} style={{ width: 130 }} />
      <Select size="sm" value={filters.size} onChange={set('size')} options={[{ value: 'all', label: 'すべてのサイズ' }, { value: 'lt1', label: '1 MB 未満' }, { value: '1to100', label: '1〜100 MB' }, { value: 'gt100', label: '100 MB 以上' }]} style={{ width: 130 }} />
      <Select size="sm" value={filters.date} onChange={set('date')} options={[{ value: 'all', label: 'すべての期間' }, { value: '7d', label: '過去 7 日間' }, { value: '30d', label: '過去 30 日間' }, { value: 'year', label: '今年' }]} style={{ width: 120 }} />
      <Select size="sm" value={filters.sc} onChange={set('sc')} options={[{ value: 'all', label: 'すべてのクラス' }, ...Object.entries(STORAGE_CLASSES).map(([value, c]) => ({ value, label: c.short }))]} style={{ width: 130 }} />
      <Button variant="link" size="sm" onClick={onClear}>クリア</Button>
      <span style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--muted-foreground)' }} className="s3-num">{count} 件</span>
    </div>
  );
}
Object.assign(window, { Toolbar, FilterBar });
