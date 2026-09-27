import React from 'react';
export function Switch({ checked, onChange, label, disabled, style }) {
  const btn = (
    <button type="button" role="switch" aria-checked={!!checked} disabled={disabled} className="s3-switch"
      onClick={() => onChange && onChange(!checked)}><span className="s3-switch__thumb"></span></button>
  );
  if (!label) return btn;
  return <label className="s3-switch-row" style={style}><span>{label}</span>{btn}</label>;
}
