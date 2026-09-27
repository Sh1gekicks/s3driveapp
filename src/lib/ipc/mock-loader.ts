// モックバックエンドの読み込み。本番ビルド（vite build）では null になり、モックはバンドルに含まれない（07 §4）。

type MockModule = typeof import('./mock');

export const loadMock: (() => Promise<MockModule>) | null =
  import.meta.env.MODE === 'production' ? null : () => import('./mock');

export async function requireMock(): Promise<MockModule> {
  if (!loadMock) throw new Error('モックバックエンドは本番ビルドでは使用できません');
  return loadMock();
}
