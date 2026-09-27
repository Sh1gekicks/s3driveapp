import React from 'react';
import { Icon } from '../core/Icon.jsx';
const cx = (...a) => a.filter(Boolean).join(' ');
export function Input({ icon, label, hint, invalid, size = 'md', className, style, id, ...rest }) {
  const fid = id || (label ? 'in-' + label : undefined);
  const field = (
    <div className="s3-field" style={label || hint ? undefined : style}>
      {icon ? <span className="s3-field__icon"><Icon name={icon} size={14} /></span> : null}
      <input id={fid} aria-invalid={invalid || undefined}
        className={cx('s3-input', icon && 's3-input--with-icon', size === 'sm' && 's3-input--sm', className)} {...rest} />
    </div>
  );
  if (!label && !hint) return field;
  return (
    <div className="s3-formrow" style={style}>
      {label ? <label className="s3-label" htmlFor={fid}>{label}</label> : null}
      {field}
      {hint ? <div className={cx('s3-hint', invalid && 's3-hint--error')}>{hint}</div> : null}
    </div>
  );
}
