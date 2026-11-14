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
    <div className="app-shell">
      {toolbar}
      <div className="work-area">
        {sidebar}
        <main className="main-pane">{children}</main>
      </div>
      {footer}
    </div>
  );
}
