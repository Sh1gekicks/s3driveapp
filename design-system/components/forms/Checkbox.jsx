import React from 'react';
import { Icon } from '../core/Icon.jsx';
export function Checkbox({ checked, indeterminate, onChange, label, disabled, style }) {
  const on = checked || indeterminate;
  return (
    <label className="s3-check" data-disabled={disabled ? '' : undefined} style={style}>
      <input type="checkbox" checked={!!checked} disabled={disabled} onChange={(e) => onChange && onChange(e.target.checked)} />
      <span className="s3-check__box" data-on={on ? '' : undefined}>
        {on ? <Icon name={indeterminate ? 'minus' : 'check'} size={11} strokeWidth={3} /> : null}
      </span>
      {label ? <span>{label}</span> : null}
    </label>
  );
}
