// リージョンの選択肢（SCR-01 ステップ 2。項目は「東京」「大阪」などの日本語名）。
// 対応表はバックエンド（util::region）と同じ内容にする。

export interface RegionOption {
  code: string;
  label: string;
  short: string;
}

export const REGIONS: RegionOption[] = [
  { code: 'ap-northeast-1', label: 'アジアパシフィック (東京)', short: '東京' },
  { code: 'ap-northeast-3', label: 'アジアパシフィック (大阪)', short: '大阪' },
  { code: 'ap-northeast-2', label: 'アジアパシフィック (ソウル)', short: 'ソウル' },
  { code: 'ap-east-1', label: 'アジアパシフィック (香港)', short: '香港' },
  { code: 'ap-southeast-1', label: 'アジアパシフィック (シンガポール)', short: 'シンガポール' },
  { code: 'ap-southeast-2', label: 'アジアパシフィック (シドニー)', short: 'シドニー' },
  { code: 'ap-southeast-3', label: 'アジアパシフィック (ジャカルタ)', short: 'ジャカルタ' },
  { code: 'ap-south-1', label: 'アジアパシフィック (ムンバイ)', short: 'ムンバイ' },
  { code: 'us-east-1', label: '米国東部 (バージニア北部)', short: 'バージニア北部' },
  { code: 'us-east-2', label: '米国東部 (オハイオ)', short: 'オハイオ' },
  { code: 'us-west-1', label: '米国西部 (北カリフォルニア)', short: '北カリフォルニア' },
  { code: 'us-west-2', label: '米国西部 (オレゴン)', short: 'オレゴン' },
  { code: 'ca-central-1', label: 'カナダ (中部)', short: 'カナダ中部' },
  { code: 'eu-west-1', label: '欧州 (アイルランド)', short: 'アイルランド' },
  { code: 'eu-west-2', label: '欧州 (ロンドン)', short: 'ロンドン' },
  { code: 'eu-west-3', label: '欧州 (パリ)', short: 'パリ' },
  { code: 'eu-central-1', label: '欧州 (フランクフルト)', short: 'フランクフルト' },
  { code: 'eu-north-1', label: '欧州 (ストックホルム)', short: 'ストックホルム' },
  { code: 'sa-east-1', label: '南米 (サンパウロ)', short: 'サンパウロ' },
];

export const DEFAULT_REGION = 'ap-northeast-1';

export function regionShort(code: string): string {
  return REGIONS.find((r) => r.code === code)?.short ?? code;
}
