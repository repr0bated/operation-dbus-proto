export function App() {
  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 p-6">
      <h1 className="text-2xl font-bold mb-4">Test - Renderer Debugging</h1>
      <p>If you see this, React works. The Renderer component is not initializing.</p>
      <p className="mt-4 text-sm text-neutral-400">
        Issue: shellSpec references components or state paths that don't exist or fail to initialize.
      </p>
    </div>
  );
}
