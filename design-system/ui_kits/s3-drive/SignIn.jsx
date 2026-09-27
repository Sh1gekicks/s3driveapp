// Onboarding: Google sign-in → connect an existing bucket with IAM credentials.
function SignIn({ onDone }) {
  const { AppWindow, Button, Input, Select, Icon } = window.DS;
  const { user } = window.S3DATA;
  const [step, setStep] = React.useState(1);
  const [busy, setBusy] = React.useState(false);
  const connect = () => { setBusy(true); setTimeout(onDone, 900); };
  const google = () => { setBusy(true); setTimeout(() => { setBusy(false); setStep(2); }, 700); };
  const Step = ({ n, label }) => (
    <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: step >= n ? 'var(--foreground)' : 'var(--muted-foreground)', fontWeight: step === n ? 600 : 400 }}>
      <span style={{ width: 18, height: 18, borderRadius: 9, display: 'grid', placeItems: 'center', fontSize: 11, fontWeight: 600,
        background: step > n ? 'var(--success)' : step === n ? 'var(--primary)' : 'var(--muted)', color: step >= n ? '#fff' : 'var(--muted-foreground)' }}>
        {step > n ? <Icon name="check" size={11} strokeWidth={3} /> : n}</span>{label}
    </div>
  );
  return (
    <AppWindow width="100%" height="100%">
      <div data-tauri-drag-region="" style={{ height: '100%', display: 'grid', placeItems: 'center', padding: 24 }}>
        <div style={{ width: 380, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 20 }}>
          <img src="../../assets/app-icon.svg" width="88" height="88" alt="" />
          <div style={{ textAlign: 'center' }}>
            <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.01em' }}>S3 Drive へようこそ</div>
            <div style={{ fontSize: 13, color: 'var(--muted-foreground)', marginTop: 4 }}>S3 バケットを、いつものドライブのように。</div>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <Step n={1} label="アカウント" /><span style={{ width: 24, height: 0.5, background: 'var(--border)' }}></span><Step n={2} label="バケットに接続" />
          </div>
          {step === 1 ? (
            <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: 10 }}>
              <Button size="lg" variant="outline" onClick={google} disabled={busy} style={{ width: '100%' }}>{busy ? 'ブラウザで認証中…' : 'Google でサインイン'}</Button>
              <div style={{ fontSize: 11, color: 'var(--muted-foreground)', textAlign: 'center', textWrap: 'pretty' }}>ブラウザが開きます。認証後、自動でアプリに戻ります。</div>
            </div>
          ) : (
            <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 10px', borderRadius: 8, background: 'var(--muted)', fontSize: 12 }}>
                <Icon name="circle-check" size={14} color="var(--success)" /><span style={{ flex: 1 }}>{user.email} でサインイン中</span>
                <Button variant="link" size="sm" onClick={() => setStep(1)}>変更</Button>
              </div>
              <Input label="アクセスキー ID" placeholder="AKIA…" defaultValue="AKIA4Z7XEXAMPLE7Q2LM" className="s3-mono" />
              <Input label="シークレットアクセスキー" type="password" defaultValue="wJalrXUtnFEMI/K7MDENG" />
              <Input label="IAM ロール ARN（任意）" placeholder="arn:aws:iam::123456789012:role/…" defaultValue="arn:aws:iam::123456789012:role/S3DriveAccess" hint="指定すると AssumeRole で一時認証情報を取得します" />
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
                <Select label="リージョン" defaultValue="ap-northeast-1" options={[{ value: 'ap-northeast-1', label: '東京' }, { value: 'ap-northeast-3', label: '大阪' }, { value: 'us-east-1', label: 'バージニア北部' }]} />
                <Input label="バケット名" defaultValue="acme-media-tokyo" />
              </div>
              <Button size="lg" onClick={connect} disabled={busy} style={{ width: '100%', marginTop: 4 }}>{busy ? '接続を確認中…' : '接続'}</Button>
              <div style={{ display: 'flex', gap: 6, alignItems: 'center', justifyContent: 'center', fontSize: 11, color: 'var(--muted-foreground)' }}>
                <Icon name="lock" size={12} />認証情報は macOS キーチェーンに保存されます
              </div>
            </div>
          )}
        </div>
      </div>
    </AppWindow>
  );
}
window.SignIn = SignIn;
