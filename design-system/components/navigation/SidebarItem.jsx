import React from 'react';
import { Icon } from '../core/Icon.jsx';
export function SidebarItem({ icon, label, active, count, iconColor, onClick, style }) {
  return (
    <button className="s3-side-item" data-active={active ? '' : undefined} onClick={onClick} style={style}>
      {icon ? <Icon name={icon} size={16} color={iconColor || 'var(--primary)'} /> : null}
      <span style={{ flex: 1, textAlign: 'left', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{label}</span>
      {count != null ? <span className="s3-side-item__count">{count}</span> : null}
    </button>
  );
}
export function SidebarSection({ title, children }) {
  return <div className="s3-side-section">{title ? <div className="s3-side-heading">{title}</div> : null}{children}</div>;
}
