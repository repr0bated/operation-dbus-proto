import { CatalogIndexEl } from "./catalog/components/catalog-index";

export function App() {
  return (
    <div className="min-h-screen bg-neutral-950 text-neutral-100 p-6">
      <h1 className="text-2xl font-bold mb-4">Catalog Index</h1>
      <CatalogIndexEl props={{ className: "" }} />
    </div>
  );
}
