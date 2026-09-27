// Translucent source list: drive views, buckets, usage meter, account.
function Sidebar({ view, bucket, onView, onBucket, onAccount }) {
  const { SidebarSection, SidebarItem, Progress, Icon, STORAGE_CLASSES } = window.DS;
  const { buckets, user } = window.S3DATA;
  const b = buckets.find((x) => x.name === bucket);
  const total = Object.values(b.usage).reduce((a, v) => a + v, 0);
  const segs = Object.entries(b.usage).map(([k, v]) => ({ value: v, color: 'var(--sc-' + STORAGE_CLASSES[k].token + ')' }));
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div style={{ flex: 1, overflow: 'auto', paddingBottom: 8 }}>
        <SidebarSection title="バケット">
          {buckets.map((x) => (
            <SidebarItem key={x.name} icon="database" label={x.name} active={bucket === x.name && view === 'files'} onClick={() => onBucket(x.name)} />
          ))}
          <SidebarItem icon="plus" label="バケットを追加" iconColor="var(--muted-foreground)" />
        </SidebarSection>
        <SidebarSection title="管理">
          <SidebarItem icon="chart-pie" label="ストレージとコスト" active={view === 'dashboard'} onClick={() => onView('dashboard')} />
        </SidebarSection>
      </div>
      <div style={{ padding: '10px 18px 12px', display: 'flex', flexDirection: 'column', gap: 6 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', fontSize: 11 }}>
          <span style={{ fontWeight: 600 }} className="s3-num">{total.toFixed(1)} GB 使用中</span>
          <span style={{ color: 'var(--muted-foreground)' }}>{b.regionShort}</span>
        </div>
        <Progress max={total} segments={segs} height={5} />
        <div style={{ fontSize: 11, color: 'var(--muted-foreground)' }} className="s3-num">{b.objects.toLocaleString()} オブジェクト</div>
      </div>
      <button className="s3-side-item" onClick={onAccount} style={{ margin: '0 10px 10px', width: 'auto', height: 40, gap: 10, borderTop: 0 }}>
        <span style={{ width: 24, height: 24, borderRadius: 12, background: 'var(--azure-500)', color: '#fff', display: 'grid', placeItems: 'center', fontSize: 11, fontWeight: 600, flexShrink: 0 }}>{user.initial}</span>
        <span style={{ flex: 1, minWidth: 0, textAlign: 'left', display: 'flex', flexDirection: 'column', gap: 2 }}>
          <span style={{ fontSize: 12, fontWeight: 500 }}>{user.name}</span>
          <span style={{ fontSize: 11, color: 'var(--muted-foreground)', overflow: 'hidden', textOverflow: 'ellipsis' }}>{user.email}</span>
        </span>
        <Icon name="chevrons-up-down" size={12} color="var(--muted-foreground)" />
      </button>
    </div>
  );
}
window.Sidebar = Sidebar;
