/**
 * PluginCatalogView - Browse all sealed plugins with json-render UI
 */
import React, { useState } from 'react';
import { JsonRenderView } from './JsonRenderView';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import type { Spec } from '@json-render/react';

// All sealed plugin catalog names
const PLUGIN_CATALOG = [
  'adc', 'agentConfig', 'antigravity', 'blockchain', 'btrfs',
  'cognitiveMcp', 'compactMcp', 'config', 'cron', 'ctlPlaneChatbot',
  'datastore', 'dnsresolver', 'embeddingModel', 'emqx', 'endpoint',
  'factory', 'fail2ban', 'freedesktop', 'fullSystem', 'gcloudAdc',
  'gemmaBrain', 'ghostbridge', 'hardware', 'hostRuntime', 'humanPrincipal',
  'identitySled', 'incus', 'jsonRender', 'keypair', 'keyring',
  'largeLanguageModel', 'login1', 'mailServer', 'mcp', 'memory',
  'net', 'netmaker', 'notebooklm', 'oci', 'openflow',
  'openflowObfuscation', 'oscalSubidRegistry', 'ovsdbBridge', 'packagekit', 'pcidecl',
  'persona', 'privacyRoutes', 'procfs', 'proxyServer', 'qdrant',
  'rovsCommands', 'rtnetlink', 'schemaRenderer', 'service', 'sessDecl',
  'sharedUnixSocket', 'software', 'tchedRouter', 'unixSocket', 'users',
  'webUi', 'wgOpdbus', 'wgcf', 'wireguard', 'workflows', 'xray',
] as const;

type PluginName = typeof PLUGIN_CATALOG[number];

// Generate a placeholder spec for any plugin
function generatePluginSpec(pluginName: string): Spec {
  // Convert camelCase to Title Case
  const title = pluginName
    .replace(/([A-Z])/g, ' $1')
    .replace(/^./, (s) => s.toUpperCase())
    .trim();

  return {
    root: 'main',
    elements: {
      main: {
        type: 'Card',
        props: { title, subtitle: `Plugin: ${pluginName}` },
        children: ['content'],
      },
      content: {
        type: 'Column',
        props: { gap: 'md' },
        children: ['status_row', 'info_text', 'actions_row'],
      },
      status_row: {
        type: 'Row',
        props: { gap: 'sm', align: 'center' },
        children: ['status_label', 'status_badge'],
      },
      status_label: {
        type: 'Text',
        props: { content: 'Status:', variant: 'small' },
      },
      status_badge: {
        type: 'Badge',
        props: { label: 'Sealed', variant: 'secondary' },
      },
      info_text: {
        type: 'Text',
        props: {
          content: 'This plugin is sealed. Connect to D-Bus projection for live data.',
          variant: 'muted',
        },
      },
      actions_row: {
        type: 'Row',
        props: { gap: 'sm' },
        children: ['inspect_btn', 'refresh_btn'],
      },
      inspect_btn: {
        type: 'Button',
        props: { label: 'Inspect', variant: 'outline', size: 'sm' },
      },
      refresh_btn: {
        type: 'Button',
        props: { label: 'Refresh', variant: 'ghost', size: 'sm' },
      },
    },
  };
}

// Category groupings for the sidebar
const PLUGIN_CATEGORIES: Record<string, PluginName[]> = {
  'Network': ['net', 'netmaker', 'wireguard', 'wgOpdbus', 'wgcf', 'openflow', 'openflowObfuscation', 'ovsdbBridge', 'rtnetlink', 'dnsresolver', 'proxyServer', 'xray'],
  'AI/ML': ['cognitiveMcp', 'compactMcp', 'ctlPlaneChatbot', 'embeddingModel', 'gemmaBrain', 'largeLanguageModel', 'mcp', 'memory', 'notebooklm', 'qdrant'],
  'Identity': ['humanPrincipal', 'identitySled', 'keypair', 'keyring', 'login1', 'users', 'oscalSubidRegistry'],
  'Infrastructure': ['btrfs', 'hardware', 'hostRuntime', 'incus', 'oci', 'packagekit', 'procfs', 'service', 'software'],
  'Routing': ['tchedRouter', 'rovsCommands', 'privacyRoutes', 'ghostbridge', 'endpoint'],
  'Data': ['datastore', 'config', 'jsonRender', 'schemaRenderer', 'workflows'],
  'Security': ['fail2ban', 'blockchain', 'adc', 'gcloudAdc'],
  'System': ['cron', 'freedesktop', 'fullSystem', 'mailServer', 'sessDecl', 'unixSocket', 'sharedUnixSocket', 'pcidecl'],
  'Other': ['agentConfig', 'antigravity', 'emqx', 'factory', 'persona', 'webUi'],
};

interface PluginCatalogViewProps {
  className?: string;
}

export function PluginCatalogView({ className }: PluginCatalogViewProps) {
  const [selectedPlugin, setSelectedPlugin] = useState<PluginName>('tchedRouter');
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(
    new Set(['Network', 'AI/ML', 'Routing'])
  );

  const spec = generatePluginSpec(selectedPlugin);

  const toggleCategory = (category: string) => {
    setExpandedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(category)) {
        next.delete(category);
      } else {
        next.add(category);
      }
      return next;
    });
  };

  const filteredPlugins = searchQuery
    ? PLUGIN_CATALOG.filter((p) => p.toLowerCase().includes(searchQuery.toLowerCase()))
    : null;

  const handleAction = (actionName: string, params?: Record<string, unknown>) => {
    console.log(`[${selectedPlugin}] Action:`, actionName, params);
  };

  return (
    <div className={cn('flex h-full', className)}>
      {/* Sidebar */}
      <div className="w-64 border-r flex flex-col">
        <div className="p-3 border-b">
          <Input
            placeholder="Search plugins..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="h-8"
          />
        </div>
        <ScrollArea className="flex-1">
          <div className="p-2">
            {searchQuery && filteredPlugins ? (
              // Search results
              <div className="space-y-1">
                {filteredPlugins.map((plugin) => (
                  <Button
                    key={plugin}
                    variant={selectedPlugin === plugin ? 'secondary' : 'ghost'}
                    size="sm"
                    className="w-full justify-start text-sm font-mono"
                    onClick={() => setSelectedPlugin(plugin)}
                  >
                    {plugin}
                  </Button>
                ))}
                {filteredPlugins.length === 0 && (
                  <p className="text-sm text-muted-foreground p-2">No plugins found</p>
                )}
              </div>
            ) : (
              // Category view
              Object.entries(PLUGIN_CATEGORIES).map(([category, plugins]) => (
                <div key={category} className="mb-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="w-full justify-between text-sm font-medium"
                    onClick={() => toggleCategory(category)}
                  >
                    <span>{category}</span>
                    <Badge variant="outline" className="ml-2">
                      {plugins.length}
                    </Badge>
                  </Button>
                  {expandedCategories.has(category) && (
                    <div className="ml-2 mt-1 space-y-0.5">
                      {plugins.map((plugin) => (
                        <Button
                          key={plugin}
                          variant={selectedPlugin === plugin ? 'secondary' : 'ghost'}
                          size="sm"
                          className="w-full justify-start text-xs font-mono h-7"
                          onClick={() => setSelectedPlugin(plugin)}
                        >
                          {plugin}
                        </Button>
                      ))}
                    </div>
                  )}
                </div>
              ))
            )}
          </div>
        </ScrollArea>
        <div className="p-3 border-t text-xs text-muted-foreground">
          {PLUGIN_CATALOG.length} plugins
        </div>
      </div>

      {/* Main content */}
      <div className="flex-1 p-6 overflow-auto">
        <div className="mb-4 flex items-center gap-3">
          <h2 className="text-2xl font-bold font-mono">{selectedPlugin}</h2>
          <Badge variant="outline">Sealed</Badge>
        </div>
        <JsonRenderView
          spec={spec}
          onAction={handleAction}
          className="max-w-2xl"
        />
      </div>
    </div>
  );
}

export default PluginCatalogView;
