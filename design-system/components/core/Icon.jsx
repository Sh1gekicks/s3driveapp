import React from 'react';
const toPascal = (s) => s.replace(/(^|-)([a-z0-9])/g, (_, __, c) => c.toUpperCase());
/** Lucide icon renderer. Requires the Lucide UMD script (window.lucide). */
export function Icon({ name, size = 16, strokeWidth = 1.75, color = 'currentColor', fill = 'none', style, className, ...rest }) {
  const lib = typeof window !== 'undefined' ? window.lucide : null;
  let node = lib ? (lib.icons && lib.icons[toPascal(name)]) || lib[toPascal(name)] : null;
  if (Array.isArray(node) && node[0] === 'svg') node = node[2];
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill={fill} stroke={color} strokeWidth={strokeWidth}
      strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" className={className}
      style={{ flexShrink: 0, display: 'block', ...style }} {...rest}>
      {Array.isArray(node) ? node.map(([tag, attrs], i) => React.createElement(tag, { key: i, ...attrs })) : null}
    </svg>
  );
}
