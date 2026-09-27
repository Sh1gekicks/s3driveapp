import React from 'react';
import { Icon } from '../core/Icon.jsx';
export function Badge({ variant = 'secondary', icon, children, style }) {
  return <span className={'s3-badge s3-badge--' + variant} style={style}>{icon ? <Icon name={icon} size={11} strokeWidth={2} /> : null}{children}</span>;
}
