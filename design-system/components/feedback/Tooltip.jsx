import React from 'react';
export function Tooltip({ content, side = 'bottom', children, open }) {
  const [hover, setHover] = React.useState(false);
  const show = open != null ? open : hover;
  return (
    <span className="s3-tip-anchor" onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}>
      {children}
      {show ? <span className={'s3-tooltip s3-tooltip--' + side} role="tooltip">{content}</span> : null}
    </span>
  );
}
