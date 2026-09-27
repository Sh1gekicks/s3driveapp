// Modal sheets: new folder, delete, move, change storage class.
function NewFolderDialog({ existing, onCancel, onCreate }) {
  const { Dialog, Button, Input } = window.DS;
  const [name, setName] = React.useState('新規フォルダ');
  const dup = existing.includes(name.trim());
  const ok = name.trim() && !dup && !name.includes('/');
  return (
    <Dialog icon="folder-plus" title="新規フォルダ" description="現在の場所にプレフィックスを作成します。" onClose={onCancel}
      footer={<><Button variant="outline" onClick={onCancel}>キャンセル</Button><Button disabled={!ok} onClick={() => onCreate(name.trim())}>作成</Button></>}>
      <Input label="名前" value={name} autoFocus onFocus={(e) => e.target.select()} onChange={(e) => setName(e.target.value)}
        onKeyDown={(e) => { if (e.key === 'Enter' && ok) onCreate(name.trim()); if (e.key === 'Escape') onCancel(); }}
        invalid={dup} hint={dup ? '同じ名前のフォルダがあります' : undefined} />
    </Dialog>
  );
}
function DeleteDialog({ items, versioning, onCancel, onDelete }) {
  const { Dialog, Button, Checkbox } = window.DS;
  const [all, setAll] = React.useState(false);
  const title = items.length === 1 ? '「' + items[0].name + '」を削除しますか？' : items.length + ' 項目を削除しますか？';
  const desc = versioning && !all ? '削除マーカーが作成されます。以前のバージョンからいつでも復元できます。' : 'この操作は取り消せません。';
  return (
    <Dialog icon="trash-2" tone="destructive" title={title} description={desc} onClose={onCancel}
      footer={<><Button variant="outline" onClick={onCancel}>キャンセル</Button><Button variant="destructive" onClick={() => onDelete(all)}>{all ? '完全に削除' : '削除'}</Button></>}>
      {versioning ? <Checkbox checked={all} onChange={setAll} label="すべてのバージョンを完全に削除する" /> : null}
    </Dialog>
  );
}
function MoveDialog({ folders, moving, onCancel, onMove }) {
  const { Dialog, Button, FileIcon } = window.DS;
  const blocked = (k) => moving.some((m) => m.folder && k.startsWith(m.key)) || moving.every((m) => m.parent === k);
  const [dest, setDest] = React.useState(null);
  const all = [{ key: '', name: '/（ルート）' }, ...folders];
  return (
    <Dialog icon="folder-input" title={moving.length === 1 ? '「' + moving[0].name + '」を移動' : moving.length + ' 項目を移動'} description="移動先のフォルダを選択してください。" width={440} onClose={onCancel}
      footer={<><Button variant="outline" onClick={onCancel}>キャンセル</Button><Button disabled={dest == null} onClick={() => onMove(dest)}>移動</Button></>}>
      <div style={{ maxHeight: 240, overflow: 'auto', borderRadius: 8, boxShadow: 'inset 0 0 0 0.5px var(--border)', padding: 4 }}>
        {all.map((f) => {
          const depth = f.key ? f.key.split('/').length - 2 : 0;
          const dis = blocked(f.key); const s = dest === f.key;
          return (
            <div key={f.key || 'root'} onClick={() => !dis && setDest(f.key)} style={{ display: 'flex', alignItems: 'center', gap: 8, height: 28, padding: '0 8px', paddingLeft: 8 + depth * 18, borderRadius: 6, fontSize: 13,
              opacity: dis ? 0.4 : 1, background: s ? 'var(--row-selected)' : 'transparent', color: s ? 'var(--row-selected-foreground)' : 'var(--foreground)' }}>
              <FileIcon folder />{f.name}
            </div>
          );
        })}
      </div>
    </Dialog>
  );
}
function StorageClassDialog({ items, onCancel, onApply }) {
  const { Dialog, Button, StorageClassBadge, Badge, STORAGE_CLASSES } = window.DS;
  const { PRICE, CLASS_INFO } = window.S3DATA;
  const files = items.filter((i) => !i.folder);
  const cur = files.length && files.every((f) => f.sc === files[0].sc) ? files[0].sc : null;
  const [v, setV] = React.useState(cur || 'STANDARD');
  return (
    <Dialog icon="layers" title="ストレージクラスを変更" width={500} onClose={onCancel}
      description={(items.length === 1 ? '「' + items[0].name + '」' : items.length + ' 項目') + ' のクラスを変更します。オブジェクトのコピーとして実行され、リクエスト料金がかかります。'}
      footer={<><Button variant="outline" onClick={onCancel}>キャンセル</Button><Button disabled={v === cur} onClick={() => onApply(v)}>変更</Button></>}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {Object.keys(STORAGE_CLASSES).map((k) => {
          const s = v === k; const info = CLASS_INFO[k];
          return (
            <div key={k} onClick={() => setV(k)} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '7px 10px', borderRadius: 8,
              boxShadow: s ? 'inset 0 0 0 1.5px var(--primary)' : 'inset 0 0 0 0.5px var(--border)', background: s ? 'color-mix(in oklch, var(--primary) 6%, transparent)' : 'transparent' }}>
              <span style={{ width: 14, height: 14, borderRadius: 7, flexShrink: 0, boxShadow: s ? 'inset 0 0 0 4px var(--primary)' : 'inset 0 0 0 1px var(--input)', background: 'var(--field-bg)' }}></span>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}><StorageClassBadge value={k} short={false} plain style={{ fontWeight: 500, fontSize: 13 }} />{k === cur ? <Badge variant="outline">現在</Badge> : null}{info.slow ? <Badge variant="warning" icon="clock">取り出し {info.retrieval}</Badge> : null}</div>
                <div style={{ fontSize: 11, color: 'var(--muted-foreground)', marginTop: 2, paddingLeft: 13 }}>{info.desc}{info.min ? ' · 最低保存期間 ' + info.min : ''}</div>
              </div>
              <div className="s3-num" style={{ fontSize: 12, color: 'var(--muted-foreground)', whiteSpace: 'nowrap' }}>${PRICE[k]}/GB</div>
            </div>
          );
        })}
      </div>
    </Dialog>
  );
}
Object.assign(window, { NewFolderDialog, DeleteDialog, MoveDialog, StorageClassDialog });
