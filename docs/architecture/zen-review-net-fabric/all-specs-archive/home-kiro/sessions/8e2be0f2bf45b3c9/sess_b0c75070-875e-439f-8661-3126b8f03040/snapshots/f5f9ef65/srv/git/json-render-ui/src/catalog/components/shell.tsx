import type { ReactNode } from "react";

interface ShellProps {
  props: { navCollapsed?: boolean | null };
  slots?: { nav?: ReactNode; header?: ReactNode; content?: ReactNode };
  children?: ReactNode;
}

export function Shell({ props, slots }: ShellProps) {
  const collapsed = props.navCollapsed ?? false;
  return (
    <div className="flex h-screen overflow-hidden">
      <aside
        className={`${collapsed ? "w-14" : "w-56"} flex-shrink-0 border-r border-neutral-800 bg-neutral-900 flex flex-col transition-all`}
      >
        <div className="p-3 text-sm font-bold tracking-tight text-neutral-300 border-b border-neutral-800">
          {collapsed ? "3t" : "3tched"}
        </div>
        <nav className="flex-1 overflow-y-auto py-2">{slots?.nav}</nav>
      </aside>
      <main className="flex-1 flex flex-col overflow-hidden">
        {slots?.header && (
          <header className="border-b border-neutral-800 px-6 py-3">
            {slots.header}
          </header>
        )}
        <div className="flex-1 overflow-y-auto p-6">{slots?.content ?? children}</div>
      </main>
    </div>
  );
}

const children = null;
