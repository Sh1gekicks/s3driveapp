import React from 'react';
export function TrafficLights({ style }) {
  return <div className="s3-traffic" style={style}><span style={{ background: '#ff5f57' }}></span><span style={{ background: '#febc2e' }}></span><span style={{ background: '#28c840' }}></span></div>;
}
export function AppWindow({ sidebar, toolbar, inspector, children, width = 1100, height = 700, style }) {
  return (
    <div className="s3-window" style={{ width, height, ...style }}>
      <TrafficLights />
      {sidebar ? <aside className="s3-window__sidebar">{sidebar}</aside> : null}
      <div className="s3-window__main">
        {toolbar !== undefined ? <header className="s3-window__toolbar" data-tauri-drag-region="" style={sidebar ? undefined : { paddingLeft: 88 }}>{toolbar}</header> : null}
        <div className="s3-window__content">{children}</div>
      </div>
      {inspector ? <aside className="s3-window__inspector">{inspector}</aside> : null}
    </div>
  );
}
