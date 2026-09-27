import React from 'react';
import { Icon } from '../core/Icon.jsx';
export function Breadcrumbs({ items = [], onNavigate, style }) {
  return (
    <nav className="s3-crumbs" style={style}>
      {items.map((it, i) => (
        <React.Fragment key={i}>
          {i > 0 ? <Icon name="chevron-right" size={12} color="var(--muted-foreground)" /> : null}
          <button className="s3-crumbs__item" data-current={i === items.length - 1 ? '' : undefined} onClick={() => onNavigate && onNavigate(i)}>
            {it.icon ? <Icon name={it.icon} size={14} /> : null}{it.label}
          </button>
        </React.Fragment>
      ))}
    </nav>
  );
}
