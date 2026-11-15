import { type ReactNode } from "react";

export interface AppShellProps {
  toolbar?: ReactNode;
  sidebar?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
}

/** The classic chrome shared by the main window: toolbar, sidebar, main pane, status footer. */
export function AppShell({ toolbar, sidebar, children, footer }: AppShellProps) {
  return (
    <div className="app-shell h-screen w-screen flex flex-col min-w-[700px] overflow-hidden bg-white dark:bg-[#1e1e1e] text-[#111111] dark:text-[#e0e0e0]">
      {toolbar}
      <div className="work-area flex flex-1 min-h-0 border-b border-[#aaa] dark:border-[#383838]">
        {sidebar}
        <main className="main-pane flex-1 flex flex-col min-w-0 bg-white dark:bg-[#1e1e1e]">{children}</main>
      </div>
      {footer}
    </div>
  );
}
