export function App() {
  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 p-6">
      <h1 className="text-2xl font-bold mb-4">Catalog</h1>
      <div className="grid gap-2" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))" }}>
        <div className="rounded-md border border-neutral-800 bg-neutral-900/50 px-3 py-2">
          <div className="text-sm font-mono text-neutral-100">Blockchain</div>
          <div className="text-[11px] font-mono text-neutral-500 mt-0.5">blockchain</div>
        </div>
        <div className="rounded-md border border-neutral-800 bg-neutral-900/50 px-3 py-2">
          <div className="text-sm font-mono text-neutral-100">ADC</div>
          <div className="text-[11px] font-mono text-neutral-500 mt-0.5">adc</div>
        </div>
      </div>
    </div>
  );
}
