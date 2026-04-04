import { Outlet } from "react-router-dom";
import { TopStatusBar } from "./TopStatusBar";
import { SidebarNav } from "./SidebarNav";

export function AppShell() {
  return (
    <div className="flex h-screen flex-col overflow-hidden">
      <TopStatusBar />
      <div className="flex flex-1 overflow-hidden">
        <SidebarNav />
        <main className="flex-1 overflow-auto">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
