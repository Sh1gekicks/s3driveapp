import React from 'react';
import { Icon } from '../core/Icon.jsx';
export function SegmentedControl({ options = [], value, onChange, block, style }) {
  return (
    <div className={block ? 's3-seg s3-seg--block' : 's3-seg'} role="tablist" style={style}>
      {options.map((o) => (
        <button key={o.value} role="tab" aria-selected={o.value === value} title={o.label}
          className="s3-seg__item" data-active={o.value === value ? '' : undefined} onClick={() => onChange && onChange(o.value)}>
          {o.icon ? <Icon name={o.icon} size={14} /> : null}{o.icon && o.iconOnly ? null : o.label}
        </button>
      ))}
    </div>
  );
}
