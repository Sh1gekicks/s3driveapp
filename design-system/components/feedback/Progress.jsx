import React from 'react';
export function Progress({ value = 0, max = 100, segments, height = 6, color, style }) {
  const segs = segments || [{ value, color: color || 'var(--primary)' }];
  return (
    <div className="s3-progress" style={{ height, ...style }} role="progressbar" aria-valuenow={value} aria-valuemax={max}>
      {segs.map((s, i) => <span key={i} style={{ width: Math.max(0, (s.value / max) * 100) + '%', background: s.color }}></span>)}
    </div>
  );
}
