import React from 'react';
export const STORAGE_CLASSES = {
  STANDARD: { label: 'Standard', short: 'Standard', token: 'standard' },
  INTELLIGENT_TIERING: { label: 'Intelligent-Tiering', short: 'Int. Tiering', token: 'intelligent-tiering' },
  STANDARD_IA: { label: 'Standard-IA', short: 'Standard-IA', token: 'standard-ia' },
  ONEZONE_IA: { label: 'One Zone-IA', short: 'One Zone-IA', token: 'onezone-ia' },
  GLACIER_IR: { label: 'Glacier Instant Retrieval', short: 'Glacier IR', token: 'glacier-ir' },
  GLACIER: { label: 'Glacier Flexible Retrieval', short: 'Glacier', token: 'glacier' },
  DEEP_ARCHIVE: { label: 'Glacier Deep Archive', short: 'Deep Archive', token: 'deep-archive' },
};
export function StorageClassBadge({ value = 'STANDARD', short = true, plain, style }) {
  const c = STORAGE_CLASSES[value] || STORAGE_CLASSES.STANDARD;
  return (
    <span className={plain ? 's3-sc s3-sc--plain' : 's3-sc'} style={style}>
      <span className="s3-sc__dot" style={{ background: 'var(--sc-' + c.token + ')' }}></span>{short ? c.short : c.label}
    </span>
  );
}
