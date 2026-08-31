/**
 * PluginCatalogPage - Browse all 60+ sealed plugins with json-render UI
 */
import React from 'react';
import { PluginCatalogView } from '@/components/json-render';

export default function PluginCatalogPage() {
  return (
    <div className="h-[calc(100vh-4rem)]">
      <PluginCatalogView className="h-full" />
    </div>
  );
}
