import { useRef } from 'react';
import { cn } from '@/lib/utils';

// ペインの境界に置く幅変更のつまみ（WAI-ARIA の Window Splitter。03 §3・§5.4）。ドラッグ、←→（10px ずつ）、
// Home／End（最小／最大）で幅を変え、ダブルクリックで既定の幅に戻す。つまみ（境界）はポインタ・←→ の向きに動く。
// 変えている間は onChange を、ドラッグ・キー操作を終えて確定したら onCommit を呼ぶ（保存はここで行う）。
// 見た目は macOS と同じく境界のヘアラインのままで、ポインタだけ col-resize にする。フォーカス時は線を出す。

const STEP = 10;

export interface ResizeHandleProps {
  label: string;
  /** 幅を変える領域の id。 */
  controls?: string;
  value: number;
  min: number;
  max: number;
  defaultValue: number;
  /** 幅を変える領域がつまみの左（before。既定）か右（after）か。右なら境界を右に動かすと狭くなる。 */
  pane?: 'before' | 'after';
  onChange: (value: number) => void;
  onCommit: () => void;
  className?: string;
  /** 境界の線（フォーカス時に `--ring` で出す要素）の class。 */
  lineClassName?: string;
}

export function ResizeHandle({
  label,
  controls,
  value,
  min,
  max,
  defaultValue,
  pane = 'before',
  onChange,
  onCommit,
  className,
  lineClassName,
}: Readonly<ResizeHandleProps>) {
  const drag = useRef<{ x: number; value: number } | null>(null);
  /** キー操作で値を変え、まだ確定していない。 */
  const keyChanged = useRef(false);
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  /** 境界を右に動かしたときの幅の増減の向き。 */
  const sign = pane === 'before' ? 1 : -1;

  const endDrag = () => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    if (value !== d.value) onCommit();
  };

  const keyValue = (key: string): number | null => {
    if (key === 'ArrowLeft') return clamp(value - STEP * sign);
    if (key === 'ArrowRight') return clamp(value + STEP * sign);
    if (key === 'Home') return min;
    if (key === 'End') return max;
    return null;
  };

  // フォーカスできる separator は WAI-ARIA 1.2 ではウィジェット（Window Splitter）で、キー操作を受け付ける。
  // Sonar（S6845・S6847）は separator を常に非インタラクティブとみなすため、該当の行を除外する。
  return (
    <div // NOSONAR
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-controls={controls}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0} // NOSONAR
      className={cn(
        'group absolute top-0 bottom-0 z-10 flex w-2 cursor-col-resize touch-none justify-center outline-none',
        className,
      )}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        // テキストの選択やフォーカスの移動を始めない
        e.preventDefault();
        e.currentTarget.setPointerCapture(e.pointerId);
        drag.current = { x: e.clientX, value };
      }}
      onPointerMove={(e) => {
        const d = drag.current;
        if (d) onChange(clamp(d.value + (e.clientX - d.x) * sign));
      }}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onKeyDown={(e) => {
        const next = keyValue(e.key);
        if (next === null) return;
        e.preventDefault();
        if (next === value) return;
        keyChanged.current = true;
        onChange(next);
      }}
      // 押し続けたときに何度も保存しないよう、キーを離したときに確定する
      onKeyUp={() => {
        if (!keyChanged.current) return;
        keyChanged.current = false;
        onCommit();
      }}
      onDoubleClick={() => {
        if (value === defaultValue) return;
        onChange(defaultValue);
        onCommit();
      }}
    >
      <span className={cn('h-full w-0.5 group-focus-visible:bg-ring', lineClassName)} />
    </div>
  );
}
