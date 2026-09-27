import React from 'react';
import { Icon } from '../core/Icon.jsx';
import { Progress } from './Progress.jsx';
export function Toast({ icon = 'info', tone = 'default', title, description, progress, action, onClose, style }) {
  const c = { default: 'var(--primary)', success: 'var(--success)', destructive: 'var(--destructive)', warning: 'var(--warning)' }[tone];
  return (
    <div className="s3-toast" role="status" style={style}>
      <Icon name={icon} size={18} color={c} />
      <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 3 }}>
        <div className="s3-toast__title">{title}</div>
        {description ? <div className="s3-toast__desc">{description}</div> : null}
        {progress != null ? <Progress value={progress} height={4} style={{ marginTop: 5 }} /> : null}
      </div>
      {action}
      {onClose ? <button className="s3-btn s3-btn--ghost s3-iconbtn s3-iconbtn--sm" aria-label="閉じる" onClick={onClose}><Icon name="x" size={14} /></button> : null}
    </div>
  );
}
