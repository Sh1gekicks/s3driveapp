import React from 'react';
import { Icon } from '../core/Icon.jsx';
export function Dialog({ open = true, icon, tone = 'default', title, description, children, footer, onClose, width = 420 }) {
  if (!open) return null;
  const c = tone === 'destructive' ? 'var(--destructive)' : 'var(--primary)';
  return (
    <div className="s3-overlay" onMouseDown={(e) => { if (e.target === e.currentTarget && onClose) onClose(); }}>
      <div className="s3-dialog" role="dialog" aria-modal="true" style={{ width }}>
        <div style={{ display: 'flex', gap: 12 }}>
          {icon ? <div className="s3-dialog__icon" style={{ color: c, background: 'color-mix(in oklch, ' + c + ' 12%, transparent)' }}><Icon name={icon} size={18} /></div> : null}
          <div style={{ flex: 1, minWidth: 0 }}>
            <div className="s3-dialog__title">{title}</div>
            {description ? <div className="s3-dialog__desc">{description}</div> : null}
          </div>
        </div>
        {children ? <div className="s3-dialog__body">{children}</div> : null}
        {footer ? <div className="s3-dialog__footer">{footer}</div> : null}
      </div>
    </div>
  );
}
