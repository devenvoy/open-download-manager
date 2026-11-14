import { type ReactNode } from "react";

export interface AppShellProps {
  toolbar?: ReactNode;
  sidebar?: ReactNode;
  children: ReactNode;
}

/** The chrome shared by every window: a toolbar, an optional sidebar, content. */
export function AppShell({ toolbar, sidebar, children }: AppShellProps) {
  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      {toolbar ? (
        <header className="flex h-16 shrink-0 items-center gap-2 border-b border-border px-2">
          {toolbar}
        </header>
      ) : null}
      <div className="flex min-h-0 flex-1">
        {sidebar}
        <main className="flex-1 overflow-auto p-4">{children}</main>
      </div>
    </div>
  );
}
