import React from 'react';
import { Icon } from './Icon.jsx';
const cx = (...a) => a.filter(Boolean).join(' ');
export function Button({ variant = 'default', size = 'md', icon, iconRight, children, className, ...rest }) {
  const is = size === 'sm' ? 14 : 16;
  return (
    <button className={cx('s3-btn', 's3-btn--' + variant, size !== 'md' && 's3-btn--' + size, className)} {...rest}>
      {icon ? <Icon name={icon} size={is} /> : null}
      {children}
      {iconRight ? <Icon name={iconRight} size={is} /> : null}
    </button>
  );
}
