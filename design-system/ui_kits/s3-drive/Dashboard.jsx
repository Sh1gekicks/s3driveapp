// ストレージとコスト: usage, region, storage-class breakdown, monthly cost estimate.
function Dashboard({ bucket }) {
  const { Progress, StorageClassBadge, Badge, STORAGE_CLASSES, Icon } = window.DS;
  const { PRICE } = window.S3DATA;
  const b = bucket;
  const rows = Object.entries(b.usage).map(([k, gb]) => ({ k, gb, cost: gb * PRICE[k] }));
  const total = rows.reduce((a, r) => a + r.gb, 0);
  const storage = rows.reduce((a, r) => a + r.cost, 0);
  const sum = storage + b.cost.requests + b.cost.transfer + b.cost.retrieval;
  const delta = ((sum - b.prevMonth) / b.prevMonth) * 100;
  const usd = (n) => '$' + n.toFixed(n < 1 ? 3 : 2);
  const maxDay = Math.max(...b.daily.filter((x) => x != null));
  const Card = ({ children, style }) => <div style={{ background: 'var(--card)', borderRadius: 10, boxShadow: 'var(--shadow-sm)', padding: 16, minWidth: 0, ...style }}>{children}</div>;
  const Kpi = ({ label, value, sub, icon }) => (
    <Card style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, fontWeight: 600, color: 'var(--muted-foreground)' }}><Icon name={icon} size={13} />{label}</div>
      <div className="s3-num" style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.01em' }}>{value}</div>
      <div style={{ fontSize: 12, color: 'var(--muted-foreground)' }}>{sub}</div>
    </Card>
  );
  const H = ({ children, right }) => <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', marginBottom: 12 }}><div style={{ fontSize: 13, fontWeight: 600 }}>{children}</div>{right}</div>;
  const num = { textAlign: 'right', fontVariantNumeric: 'tabular-nums' };
  return (
    <div style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 16, maxWidth: 1000 }}>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(170px, 1fr))', gap: 12 }}>
        <Kpi icon="hard-drive" label="利用容量" value={total.toFixed(1) + ' GB'} sub={b.objects.toLocaleString() + ' オブジェクト'} />
        <Kpi icon="receipt" label="今月の推定コスト" value={'$' + sum.toFixed(2)} sub={<span>前月比 <span style={{ color: delta > 0 ? 'var(--destructive)' : 'var(--success)', fontWeight: 500 }}>{(delta > 0 ? '+' : '') + delta.toFixed(1)}%</span></span>} />
        <Kpi icon="globe" label="リージョン" value={b.regionShort} sub={b.region} />
        <Kpi icon="history" label="バージョニング" value={b.versioning ? '有効' : '無効'} sub={b.versioning ? '削除・上書きから復元可能' : 'バケット設定で有効化'} />
      </div>
      <Card>
        <H right={<span style={{ fontSize: 12, color: 'var(--muted-foreground)' }}>単価は {b.regionShort} リージョンの公開料金</span>}>ストレージクラス別</H>
        <Progress max={total} height={10} segments={rows.map((r) => ({ value: r.gb, color: 'var(--sc-' + STORAGE_CLASSES[r.k].token + ')' }))} />
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(160px,1fr) 90px 70px 110px 90px', fontSize: 12, marginTop: 14 }}>
          {['クラス', '容量', '割合', '単価 / GB·月', '月額'].map((h, i) => <div key={h} style={{ ...(i ? num : {}), color: 'var(--muted-foreground)', fontWeight: 500, paddingBottom: 6, borderBottom: '0.5px solid var(--border)' }}>{h}</div>)}
          {rows.map((r) => (
            <React.Fragment key={r.k}>
              <div style={{ padding: '7px 0', borderBottom: '0.5px solid var(--border)' }}><StorageClassBadge value={r.k} short={false} plain /></div>
              <div style={{ ...num, padding: '7px 0', borderBottom: '0.5px solid var(--border)' }}>{r.gb.toFixed(1)} GB</div>
              <div style={{ ...num, padding: '7px 0', borderBottom: '0.5px solid var(--border)', color: 'var(--muted-foreground)' }}>{((r.gb / total) * 100).toFixed(1)}%</div>
              <div style={{ ...num, padding: '7px 0', borderBottom: '0.5px solid var(--border)', color: 'var(--muted-foreground)' }}>${PRICE[r.k]}</div>
              <div style={{ ...num, padding: '7px 0', borderBottom: '0.5px solid var(--border)', fontWeight: 500 }}>{usd(r.cost)}</div>
            </React.Fragment>
          ))}
        </div>
      </Card>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: 16 }}>
        <Card>
          <H>コスト内訳（9月）</H>
          <div style={{ display: 'flex', flexDirection: 'column', fontSize: 13 }}>
            {[['ストレージ', storage], ['リクエスト（PUT / GET / LIST）', b.cost.requests], ['データ転送（アウト）', b.cost.transfer], ['取り出し', b.cost.retrieval]].map(([l, v]) => (
              <div key={l} style={{ display: 'flex', justifyContent: 'space-between', padding: '7px 0', borderBottom: '0.5px solid var(--border)' }}><span>{l}</span><span className="s3-num">{usd(v)}</span></div>
            ))}
            <div style={{ display: 'flex', justifyContent: 'space-between', padding: '10px 0 0', fontWeight: 700 }}><span>合計</span><span className="s3-num">${sum.toFixed(2)}</span></div>
          </div>
        </Card>
        <Card>
          <H right={<Badge variant="outline">月末予測 ${(sum / 27 * 30).toFixed(2)}</Badge>}>日別コスト</H>
          <div style={{ display: 'flex', alignItems: 'flex-end', gap: 3, height: 120 }}>
            {b.daily.map((v, i) => (
              <div key={i} title={v != null ? '9/' + (i + 1) + ' ' + usd(v) : '予測'} style={{ flex: 1, borderRadius: 2, height: ((v != null ? v : maxDay * 0.9) / maxDay) * 100 + '%',
                background: v != null ? 'var(--primary)' : 'transparent', boxShadow: v != null ? 'none' : 'inset 0 0 0 1px var(--border)' }}></div>
            ))}
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11, color: 'var(--muted-foreground)', marginTop: 6 }} className="s3-num"><span>9/1</span><span>9/8</span><span>9/15</span><span>9/22</span><span>9/30</span></div>
        </Card>
      </div>
      <div style={{ fontSize: 11, color: 'var(--muted-foreground)' }}>コストは AWS Cost Explorer の見積もりです。反映まで最大 24 時間かかります。</div>
    </div>
  );
}
window.Dashboard = Dashboard;
