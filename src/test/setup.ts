import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach } from 'vitest';

// vitest の globals を使わないため、描画した画面の後始末を明示する
afterEach(() => cleanup());
