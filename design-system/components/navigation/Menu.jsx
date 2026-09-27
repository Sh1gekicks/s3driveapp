import React from 'react';
import { Icon } from '../core/Icon.jsx';
export function Menu({ items = [], onSelect, style }) {
  return (
    <div className="s3-menu" role="menu" style={style}>
      {items.map((it, i) => it.separator ? <div key={i} className="s3-menu__sep" role="separator"></div> : (
        <button key={i} role="menuitem" disabled={it.disabled} className={'s3-menu__item' + (it.destructive ? ' s3-menu__item--destructive' : '')}
          onClick={() => onSelect && onSelect(it)}>
          {it.icon ? <Icon name={it.icon} size={15} /> : <span style={{ width: 15 }}></span>}
          <span style={{ flex: 1, textAlign: 'left' }}>{it.label}</span>
          {it.shortcut ? <span className="s3-menu__kbd">{it.shortcut}</span> : null}
        </button>
      ))}
    </div>
  );
}
