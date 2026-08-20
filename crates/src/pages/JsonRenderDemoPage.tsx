/**
 * JsonRenderDemoPage - Demo page for testing json-render integration
 */
import React, { useState } from 'react';
import { JsonRenderView } from '@/components/json-render';
import { Button } from '@/components/ui/button';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Textarea } from '@/components/ui/textarea';
import type { Spec } from '@json-render/react';

// Example spec matching TchedRouter's BoundedChildren structure
const exampleSpec: Spec = {
  root: 'main',
  elements: {
    main: {
      type: 'Column',
      props: { gap: 'md' },
      children: ['header', 'metrics_row', 'operations_card'],
    },
    header: {
      type: 'Row',
      props: { justify: 'between', align: 'center' },
      children: ['title', 'status_badge'],
    },
    title: {
      type: 'Text',
      props: { content: 'TchedRouter Status', variant: 'h3' },
    },
    status_badge: {
      type: 'Badge',
      props: { label: 'Online', variant: 'default' },
    },
    metrics_row: {
      type: 'Row',
      props: { gap: 'lg' },
      children: ['metric_ops', 'metric_success', 'metric_latency'],
    },
    metric_ops: {
      type: 'Metric',
      props: { label: 'Operations', value: 42, format: 'number' },
    },
    metric_success: {
      type: 'Metric',
      props: { label: 'Success Rate', value: 98.5, format: 'percent', trend: 'up' },
    },
    metric_latency: {
      type: 'Metric',
      props: { label: 'Avg Latency', value: '45ms' },
    },
    operations_card: {
      type: 'Card',
      props: { title: 'Recent Operations', subtitle: 'Last 5 operations' },
      children: ['op_list'],
    },
    op_list: {
      type: 'Column',
      props: { gap: 'sm' },
      children: ['op_1', 'op_2', 'op_3'],
    },
    op_1: {
      type: 'Row',
      props: { justify: 'between', align: 'center' },
      children: ['op_1_text', 'op_1_badge'],
    },
    op_1_text: {
      type: 'Text',
      props: { content: 'network_scan', variant: 'small' },
    },
    op_1_badge: {
      type: 'Badge',
      props: { label: 'ok', variant: 'default' },
    },
    op_2: {
      type: 'Row',
      props: { justify: 'between', align: 'center' },
      children: ['op_2_text', 'op_2_badge'],
    },
    op_2_text: {
      type: 'Text',
      props: { content: 'connect', variant: 'small' },
    },
    op_2_badge: {
      type: 'Badge',
      props: { label: 'pending', variant: 'secondary' },
    },
    op_3: {
      type: 'Row',
      props: { justify: 'between', align: 'center' },
      children: ['op_3_text', 'op_3_badge'],
    },
    op_3_text: {
      type: 'Text',
      props: { content: 'disconnect', variant: 'small' },
    },
    op_3_badge: {
      type: 'Badge',
      props: { label: 'err', variant: 'destructive' },
    },
  },
};

export default function JsonRenderDemoPage() {
  const [spec, setSpec] = useState<Spec | null>(exampleSpec);
  const [specJson, setSpecJson] = useState(JSON.stringify(exampleSpec, null, 2));
  const [parseError, setParseError] = useState<string | null>(null);

  const handleApply = () => {
    try {
      const parsed = JSON.parse(specJson);
      setSpec(parsed);
      setParseError(null);
    } catch (e) {
      setParseError(e instanceof Error ? e.message : 'Invalid JSON');
    }
  };

  const handleAction = (actionName: string, params?: Record<string, unknown>) => {
    console.log('Action dispatched:', actionName, params);
    alert(`Action: ${actionName}\nParams: ${JSON.stringify(params, null, 2)}`);
  };

  return (
    <div className="container mx-auto p-6 space-y-6">
      <h1 className="text-3xl font-bold">json-render Demo</h1>
      <p className="text-muted-foreground">
        Test the json-render integration with shadcn/ui components
      </p>

      <div className="grid grid-cols-2 gap-6">
        {/* Spec Editor */}
        <Card>
          <CardHeader>
            <CardTitle>Spec Editor</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <Textarea
              value={specJson}
              onChange={(e) => setSpecJson(e.target.value)}
              className="font-mono text-sm h-96"
              placeholder="Enter JSON spec..."
            />
            {parseError && (
              <p className="text-sm text-destructive">{parseError}</p>
            )}
            <div className="flex gap-2">
              <Button onClick={handleApply}>Apply Spec</Button>
              <Button variant="outline" onClick={() => setSpec(null)}>
                Clear
              </Button>
              <Button
                variant="outline"
                onClick={() => {
                  setSpecJson(JSON.stringify(exampleSpec, null, 2));
                  setSpec(exampleSpec);
                  setParseError(null);
                }}
              >
                Reset to Example
              </Button>
            </div>
          </CardContent>
        </Card>

        {/* Rendered Output */}
        <Card>
          <CardHeader>
            <CardTitle>Rendered Output</CardTitle>
          </CardHeader>
          <CardContent>
            <JsonRenderView
              spec={spec}
              onAction={handleAction}
              className="min-h-96"
            />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
