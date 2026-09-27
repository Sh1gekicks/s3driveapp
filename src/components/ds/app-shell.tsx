import type * as React from 'react';
import { cn } from '@/lib/utils';

// DS: components/shell/AppWindow.jsx（03 §3）。サイドバー 220、ツールバー 52（タイトルバーを兼ねる）、インスペクタ 280。
// 信号機ボタンは OS が描画するため、DS の TrafficLights モックは使わない（02 §6）。

export interface AppShellProps {
  sidebar: React.ReactNode;
  toolbar: React.ReactNode;
  inspector?: React.ReactNode;
  /** 幅が狭いときにインスペクタをコンテンツの右側に重ねて表示する（01 §5.5）。 */
  overlayInspector?: React.ReactNode;
  children: React.ReactNode;
}

export function AppShell({ sidebar, toolbar, inspector, overlayInspector, children }: AppShellProps) {
  return (
    <div className="flex h-full overflow-hidden text-foreground">
      <aside
        aria-label="サイドバー"
        className="flex w-(--sidebar-w) shrink-0 flex-col bg-sidebar pt-(--titlebar-h) hairline-r"
        style={{ borderRightColor: 'var(--sidebar-border)' }}
        data-tauri-drag-region
      >
        {sidebar}
      </aside>
      <main className="flex min-w-0 flex-1 flex-col bg-background">
        <Toolbar>{toolbar}</Toolbar>
        <div className="relative flex min-h-0 flex-1 flex-col">
          {children}
          {overlayInspector ? (
            <div className="absolute top-0 right-0 bottom-0 z-5 w-[min(300px,85%)] overflow-auto bg-background elevation-md hairline-l">
              {overlayInspector}
            </div>
          ) : null}
        </div>
      </main>
      {inspector ? (
        <aside
          aria-label="インスペクタ"
          className="w-(--inspector-w) shrink-0 overflow-auto bg-background hairline-l"
        >
          {inspector}
        </aside>
      ) : null}
    </div>
  );
}

/** ツールバー（高さ 52px、タイトルバーを兼ねるドラッグ領域。操作部品にはドラッグ属性を付けない。02 §7.4）。 */
export function Toolbar({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <header
      data-tauri-drag-region
      className={cn(
        'flex h-(--titlebar-h) min-w-0 shrink-0 items-center gap-2 overflow-hidden pr-3 pl-3.5 hairline-b',
        className,
      )}
    >
      {children}
    </header>
  );
}

/** 余白（ドラッグ領域）。 */
export function DragSpacer({ className }: { className?: string }) {
  return <div data-tauri-drag-region className={cn('min-w-3 flex-1 self-stretch', className)} />;
}
