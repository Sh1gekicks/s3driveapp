import React from 'react';
import { Icon } from '../core/Icon.jsx';
const cx = (...a) => a.filter(Boolean).join(' ');
export function Select({ options = [], label, size = 'md', className, style, id, ...rest }) {
  const fid = id || (label ? 'sel-' + label : undefined);
  const el = (
    <div className="s3-field" style={label ? undefined : style}>
      <select id={fid} className={cx('s3-input', 's3-select', size === 'sm' && 's3-input--sm', className)} {...rest}>
        {options.map((o) => { const v = typeof o === 'string' ? { value: o, label: o } : o;
          return <option key={v.value} value={v.value}>{v.label}</option>; })}
      </select>
      <span className="s3-select__chev"><Icon name="chevrons-up-down" size={12} /></span>
    </div>
  );
  if (!label) return el;
  return <div className="s3-formrow" style={style}><label className="s3-label" htmlFor={fid}>{label}</label>{el}</div>;
}
