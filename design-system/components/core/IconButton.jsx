import React from 'react';
import { Icon } from './Icon.jsx';
const cx = (...a) => a.filter(Boolean).join(' ');
export function IconButton({ icon, label, variant = 'ghost', size = 'md', active, className, ...rest }) {
  return (
    <button aria-label={label} title={label} data-active={active ? '' : undefined}
      className={cx('s3-btn', 's3-btn--' + variant, 's3-iconbtn', size !== 'md' && 's3-iconbtn--' + size, className)} {...rest}>
      <Icon name={icon} size={size === 'sm' ? 14 : 16} />
    </button>
  );
}
