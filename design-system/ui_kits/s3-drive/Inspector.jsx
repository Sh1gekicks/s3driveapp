// Right-hand inspector: metadata (詳細) and object versions (バージョン).
function Inspector({ item, count, folderInfo, bucket, tab, onTab, onAction, onRestore, onDeleteVersion, onDownloadVersion }) {
  const { FileIcon, StorageClassBadge, SegmentedControl, Button, IconButton, Badge, Icon, fileKind } = window.DS;
  const { fmtDate, fmtSize, KIND_LABEL } = window.S3DATA;
  const Row = ({ k, children, mono }) => (
    <>
      <div style={{ color: 'var(--muted-foreground)' }}>{k}</div>
      <div className={mono ? 's3-mono s3-selectable' : 's3-selectable'} style={{ minWidth: 0, wordBreak: 'break-all', fontSize: mono ? 11 : 12 }}>{children}</div>
    </>
  );
  const grid = { display: 'grid', gridTemplateColumns: '88px 1fr', gap: '8px 10px', fontSize: 12, alignItems: 'baseline' };
  const Header = ({ name, folder, sub }) => (
    <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
      <FileIcon name={name} folder={folder} size={36} />
      <div style={{ minWidth: 0 }}>
        <div className="s3-selectable" style={{ fontSize: 13, fontWeight: 600, wordBreak: 'break-all' }}>{name}</div>
        <div style={{ fontSize: 12, color: 'var(--muted-foreground)' }}>{sub}</div>
      </div>
    </div>
  );
  const wrap = { padding: 16, display: 'flex', flexDirection: 'column', gap: 16 };
  if (count > 1) return (
    <div style={wrap}>
      <Header name={count + ' 項目'} sub="複数選択" />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <Button icon="download" onClick={() => onAction('download')}>ダウンロード</Button>
        <Button variant="outline" icon="folder-input" onClick={() => onAction('move')}>移動…</Button>
        <Button variant="outline" icon="layers" onClick={() => onAction('class')}>ストレージクラスを変更…</Button>
        <Button variant="ghost" icon="trash-2" onClick={() => onAction('delete')} style={{ color: 'var(--destructive)' }}>削除</Button>
      </div>
    </div>
  );
  if (!item || item.folder) {
    const f = folderInfo;
    return (
      <div style={wrap}>
        <Header name={f.name} folder sub={f.count + ' 項目 · ' + fmtSize(f.size)} />
        <div style={grid}>
          <Row k="バケット">{bucket.name}</Row>
          <Row k="リージョン">{bucket.regionLabel}</Row>
          <Row k="プレフィックス" mono>{f.key || '/'}</Row>
          <Row k="バージョニング">{bucket.versioning ? '有効' : '無効'}</Row>
        </div>
        {item ? <div style={{ display: 'flex', gap: 8 }}>
          <Button variant="outline" icon="folder-input" onClick={() => onAction('move')} style={{ flex: 1 }}>移動…</Button>
          <Button variant="outline" icon="trash-2" onClick={() => onAction('delete')} style={{ flex: 1, color: 'var(--destructive)' }}>削除</Button>
        </div> : null}
      </div>
    );
  }
  const kind = KIND_LABEL[fileKind(item.name)];
  return (
    <div style={wrap}>
      <Header name={item.name} sub={kind + ' · ' + fmtSize(item.size)} />
      <SegmentedControl block value={tab} onChange={onTab} options={[{ value: 'info', label: '詳細' }, { value: 'versions', label: 'バージョン (' + item.versions.length + ')' }]} />
      {tab === 'info' ? (
        <>
          <div style={grid}>
            <Row k="サイズ"><span className="s3-num">{fmtSize(item.size)}（{item.size.toLocaleString()} バイト）</span></Row>
            <Row k="作成日"><span className="s3-num">{fmtDate(item.created)}</span></Row>
            <Row k="更新日"><span className="s3-num">{fmtDate(item.modified)}</span></Row>
            <div style={{ color: 'var(--muted-foreground)' }}>クラス</div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}><StorageClassBadge value={item.sc} /><Button variant="link" size="sm" onClick={() => onAction('class')}>変更…</Button></div>
            <Row k="Content-Type" mono>{item.contentType}</Row>
            <Row k="キー" mono>{item.key}</Row>
            <Row k="ETag" mono>{item.etag}</Row>
            <Row k="暗号化">SSE-S3 (AES-256)</Row>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <Button icon="download" onClick={() => onAction('download')}>ダウンロード</Button>
            <div style={{ display: 'flex', gap: 8 }}>
              <Button variant="outline" icon="folder-input" onClick={() => onAction('move')} style={{ flex: 1 }}>移動…</Button>
              <Button variant="outline" icon="trash-2" onClick={() => onAction('delete')} style={{ flex: 1, color: 'var(--destructive)' }}>削除</Button>
            </div>
          </div>
        </>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', margin: '0 -8px' }}>
          {!bucket.versioning ? <div style={{ fontSize: 12, color: 'var(--muted-foreground)', padding: '0 8px 8px' }}>このバケットはバージョニングが無効です。</div> : null}
          {item.versions.map((v, i) => (
            <div key={v.id} style={{ display: 'flex', gap: 8, alignItems: 'center', padding: '8px', borderRadius: 6, background: v.latest ? 'var(--muted)' : 'transparent' }}>
              <div style={{ width: 8, display: 'flex', justifyContent: 'center', alignSelf: 'stretch' }}>
                <span style={{ width: 7, height: 7, marginTop: 4, borderRadius: 4, background: v.latest ? 'var(--primary)' : 'var(--gray-400)' }}></span>
              </div>
              <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 3 }}>
                <div style={{ display: 'flex', gap: 6, alignItems: 'center', fontSize: 12, fontWeight: 600 }} className="s3-num">{fmtDate(v.date)}{v.latest ? <Badge variant="success">最新</Badge> : null}</div>
                <div style={{ fontSize: 11, color: 'var(--muted-foreground)', display: 'flex', gap: 6 }}>
                  <span className="s3-num">{fmtSize(v.size)}</span>·<span className="s3-mono s3-selectable" style={{ fontSize: 10.5, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{v.id}</span>
                </div>
              </div>
              <IconButton size="sm" icon="rotate-ccw" label="このバージョンを復元" disabled={v.latest} onClick={() => onRestore(v)} />
              <IconButton size="sm" icon="download" label="このバージョンをダウンロード" onClick={() => onDownloadVersion(v)} />
              <IconButton size="sm" icon="trash-2" label="このバージョンを完全に削除" disabled={item.versions.length === 1} onClick={() => onDeleteVersion(v)} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
window.Inspector = Inspector;
