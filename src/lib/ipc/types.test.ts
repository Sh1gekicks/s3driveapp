// ts-rs が生成した型がすべて types.ts から再エクスポートされているかを確かめる。

import { describe, expect, it } from 'vitest';
import source from './types.ts?raw';

const generated = Object.keys(import.meta.glob('./bindings/*.ts'))
  .map((path) => path.replace(/^\.\/bindings\/|\.ts$/g, ''))
  .sort();

describe('types.ts', () => {
  it('bindings/ の型をすべて再エクスポートする', () => {
    const exported = [...source.matchAll(/from '\.\/bindings\/(\w+)'/g)].map((m) => m[1]).sort();
    expect(generated.length).toBeGreaterThan(0);
    expect(exported).toEqual(generated);
  });
});
