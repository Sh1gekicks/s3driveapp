// File browser body: list (sortable columns) or icon grid, drag & drop upload, status bar.
function FileList({ items, mode, selected, onSelect, onOpen, onContext, sort, onSort, showPath, narrow, onDropFiles, dropLabel, status, empty }) {
  const { FileIcon, StorageClassBadge, Icon, fileKind } = window.DS;
  const { fmtDate, fmtSize, KIND_LABEL } = window.S3DATA;
  const [drag, setDrag] = React.useState(false);
  const cols = narrow ? 'minmax(140px, 1fr) 124px 72px 112px' : 'minmax(200px, 1fr) 140px 80px 120px 120px';
  const isSel = (it) => selected.includes(it.key);
  const dnd = {
    onDragOver: (e) => { if (!onDropFiles) return; e.preventDefault(); setDrag(true); },
    onDragLeave: (e) => { if (!e.currentTarget.contains(e.relatedTarget)) setDrag(false); },
    onDrop: (e) => { e.preventDefault(); setDrag(false); if (onDropFiles && e.dataTransfer.files.length) onDropFiles(Array.from(e.dataTransfer.files)); },
  };
  const Head = ({ k, label, align }) => (
    <div onClick={() => k && onSort(k)} style={{ display: 'flex', alignItems: 'center', gap: 4, justifyContent: align === 'right' ? 'flex-end' : 'flex-start', padding: '0 8px', color: sort.key === k ? 'var(--foreground)' : 'var(--muted-foreground)', fontWeight: 500 }}>
      {label}{sort.key === k ? <Icon name={sort.dir > 0 ? 'chevron-up' : 'chevron-down'} size={12} /> : null}
    </div>
  );
  const rowProps = (it) => ({
    onMouseDown: (e) => { if (e.button === 0) { e.stopPropagation(); onSelect(it, e); } },
    onDoubleClick: () => onOpen(it),
    onContextMenu: (e) => { e.preventDefault(); e.stopPropagation(); if (!isSel(it)) onSelect(it, {}); onContext(e, it); },
  });
  return (
    <div {...dnd} style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column', position: 'relative' }}>
      <div onMouseDown={() => onSelect(null)} onContextMenu={(e) => { e.preventDefault(); onSelect(null); onContext(e, null); }} style={{ flex: 1, overflow: 'auto', minHeight: 0 }}>
        {items.length === 0 ? (
          <div style={{ height: '100%', display: 'grid', placeItems: 'center', color: 'var(--muted-foreground)', textAlign: 'center', fontSize: 13 }}>
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8 }}><Icon name={empty.icon} size={32} strokeWidth={1.25} />{empty.text}</div>
          </div>
        ) : mode === 'list' ? (
          <div style={{ fontSize: 13 }}>
            <div style={{ display: 'grid', gridTemplateColumns: cols, height: 28, alignItems: 'center', fontSize: 12, position: 'sticky', top: 0, zIndex: 1, background: 'var(--background)', borderBottom: '0.5px solid var(--border)', padding: '0 8px' }}>
              <Head k="name" label="名前" /><Head k="modified" label="更新日" /><Head k="size" label="サイズ" align="right" />{narrow ? null : <Head label="種類" />}<Head k="sc" label="ストレージクラス" />
            </div>
            <div style={{ padding: '4px 8px' }}>
              {items.map((it, i) => {
                const s = isSel(it);
                const sub = s ? { color: 'inherit', opacity: 0.85 } : { color: 'var(--muted-foreground)' };
                return (
                  <div key={it.key} {...rowProps(it)} style={{ display: 'grid', gridTemplateColumns: cols, height: 28, alignItems: 'center', borderRadius: 6,
                    background: s ? 'var(--row-selected)' : i % 2 ? 'var(--row-stripe)' : 'transparent', color: s ? 'var(--row-selected-foreground)' : 'var(--foreground)' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 8px', minWidth: 0 }}>
                      <FileIcon name={it.name} folder={it.folder} />
                      <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{it.name}</span>
                      {showPath && it.parent ? <span style={{ ...sub, fontSize: 12, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{it.parent}</span> : null}
                    </div>
                    <div className="s3-num" style={{ ...sub, padding: '0 8px', fontSize: 12 }}>{fmtDate(it.modified)}</div>
                    <div className="s3-num" style={{ ...sub, padding: '0 8px', fontSize: 12, textAlign: 'right' }}>{it.folder ? '—' : fmtSize(it.size)}</div>
                    {narrow ? null : <div style={{ ...sub, padding: '0 8px', fontSize: 12, whiteSpace: 'nowrap' }}>{KIND_LABEL[fileKind(it.name, it.folder)]}</div>}
                    <div style={{ padding: '0 8px' }}>{it.folder ? <span style={sub}>—</span> : <StorageClassBadge value={it.sc} plain style={s ? { color: 'inherit' } : undefined} />}</div>
                  </div>
                );
              })}
            </div>
          </div>
        ) : (
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(104px, 1fr))', gap: 8, padding: 16 }}>
            {items.map((it) => {
              const s = isSel(it);
              return (
                <div key={it.key} {...rowProps(it)} style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6, padding: 6 }}>
                  <div style={{ width: 72, height: 64, borderRadius: 8, display: 'grid', placeItems: 'center', background: s ? 'var(--row-selected-inactive)' : 'transparent' }}><FileIcon name={it.name} folder={it.folder} size={44} /></div>
                  <span style={{ fontSize: 12, textAlign: 'center', padding: '1px 5px', borderRadius: 4, maxWidth: '100%', wordBreak: 'break-all', display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden',
                    background: s ? 'var(--row-selected)' : 'transparent', color: s ? 'var(--row-selected-foreground)' : 'var(--foreground)' }}>{it.name}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>
      <div className="s3-num" style={{ height: 26, flexShrink: 0, borderTop: '0.5px solid var(--border)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 11, color: 'var(--muted-foreground)' }}>{status}</div>
      {drag ? (
        <div style={{ position: 'absolute', inset: 8, borderRadius: 10, border: '2px dashed var(--primary)', background: 'color-mix(in oklch, var(--primary) 8%, transparent)', display: 'grid', placeItems: 'center', pointerEvents: 'none' }}>
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 8, color: 'var(--primary)', fontSize: 13, fontWeight: 600 }}><Icon name="cloud-upload" size={32} />{dropLabel}</div>
        </div>
      ) : null}
    </div>
  );
}
window.FileList = FileList;
