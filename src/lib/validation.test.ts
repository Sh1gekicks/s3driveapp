import { describe, expect, it } from 'vitest';
import { connectionSchema, credentialSchema, validateName } from './validation';

// テスト用の架空の値。シークレットスキャンに実在のキーと誤検知されないよう、アクセスキー ID は分けて書く
const KEY_SUFFIX = 'TESTFAKEKEY00000';
const KEY_ID = `AKIA${KEY_SUFFIX}`;
const SECRET = 'test-secret-access-key-for-s3drive-00000';

const base = {
  credentialMode: 'new' as const,
  credentialId: '',
  accessKeyId: KEY_ID,
  secretAccessKey: SECRET,
  roleArn: '',
  externalId: '',
  region: 'ap-northeast-1',
  bucket: 'acme-media-tokyo',
};

function errors(values: Partial<typeof base> | Record<string, unknown>) {
  const r = connectionSchema.safeParse({ ...base, ...values });
  return r.success ? {} : Object.fromEntries(r.error.issues.map((i) => [i.path.join('.'), i.message]));
}

describe('接続の入力（03 §4.2）', () => {
  it('正しい入力', () => {
    expect(errors({})).toEqual({});
    expect(errors({ roleArn: 'arn:aws:iam::123456789012:role/S3DriveAccess' })).toEqual({});
  });

  it('アクセスキー ID は AKIA で始まる 20 文字', () => {
    expect(errors({ accessKeyId: `ASIA${KEY_SUFFIX}` })).toHaveProperty('accessKeyId');
    expect(errors({ accessKeyId: 'AKIA123' })).toHaveProperty('accessKeyId');
    expect(errors({ accessKeyId: '' }).accessKeyId).toBe('入力してください');
  });

  it('シークレットアクセスキーは 40 文字', () => {
    expect(errors({ secretAccessKey: 'short' })).toHaveProperty('secretAccessKey');
  });

  it('既存の認証情報では入力を求めない', () => {
    expect(
      errors({ credentialMode: 'existing', credentialId: 'cred-1', accessKeyId: '', secretAccessKey: '' }),
    ).toEqual({});
    expect(errors({ credentialMode: 'existing', credentialId: '' })).toHaveProperty('credentialId');
  });

  it('ロール ARN の形式', () => {
    expect(errors({ roleArn: 'arn:aws:iam::123:role/x' })).toHaveProperty('roleArn');
    expect(errors({ roleArn: 'arn:aws:s3:::bucket' })).toHaveProperty('roleArn');
  });

  it.each(['ab', 'UPPER', '-start', 'end-', 'a..b', 'a_b', 'x'.repeat(64)])(
    '不正なバケット名: %s',
    (bucket) => {
      expect(errors({ bucket })).toHaveProperty('bucket');
    },
  );

  it.each(['abc', 'my.bucket-1', 'a'.repeat(63)])('正しいバケット名: %s', (bucket) => {
    expect(errors({ bucket })).toEqual({});
  });

  it('認証情報の更新（DLG-06）', () => {
    expect(
      credentialSchema.safeParse({ accessKeyId: base.accessKeyId, secretAccessKey: base.secretAccessKey })
        .success,
    ).toBe(true);
    expect(
      credentialSchema.safeParse({ accessKeyId: 'x', secretAccessKey: base.secretAccessKey }).success,
    ).toBe(false);
  });
});

describe('名前の検証（DLG-01／10）', () => {
  it('問題がなければ null', () => {
    expect(validateName('新規フォルダ', 'projects/')).toBeNull();
  });
  it('空・スラッシュ・ドット', () => {
    expect(validateName('  ', '')).toBe('名前を入力してください');
    expect(validateName('a/b', '')).toBe('名前に「/」は使用できません');
    expect(validateName('..', '')).toBe('この名前は使用できません');
  });
  it('同じ階層の同名', () => {
    expect(validateName('photos', '', ['photos', 'backups'])).toBe('同じ名前のフォルダがあります');
  });
  it('キー全体が 1,024 バイトを超える', () => {
    // 「あ」は UTF-8 で 3 バイト
    expect(validateName('あ'.repeat(341), '')).toBeNull();
    expect(validateName('あ'.repeat(342), '')).toBe('名前が長すぎます');
  });
});
