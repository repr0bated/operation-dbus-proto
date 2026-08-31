/**
 * json-render Component Catalog
 * 
 * Uses defineCatalog from @json-render/core with the React schema
 * to create a type-safe catalog of permitted components.
 */
import { defineCatalog } from '@json-render/core';
import { schema } from '@json-render/react/schema';
import { z } from 'zod';

/**
 * The catalog defines all permitted components and their props.
 * The AI generates UI element trees constrained strictly to these definitions.
 */
export const catalog = defineCatalog(schema, {
  components: {
    Card: {
      props: z.object({
        title: z.string().optional(),
        subtitle: z.string().optional(),
        className: z.string().optional(),
      }),
      slots: ['default'],
      description: 'Container with optional title/subtitle, supports children',
    },
    Badge: {
      props: z.object({
        label: z.string(),
        variant: z.enum(['default', 'secondary', 'destructive', 'outline']).optional(),
        className: z.string().optional(),
      }),
      description: 'Small status indicator label',
    },
    Button: {
      props: z.object({
        label: z.string(),
        variant: z.enum(['default', 'destructive', 'outline', 'secondary', 'ghost', 'link']).optional(),
        size: z.enum(['default', 'sm', 'lg', 'icon']).optional(),
        disabled: z.boolean().optional(),
        className: z.string().optional(),
      }),
      description: 'Clickable action button',
    },
    Text: {
      props: z.object({
        content: z.string(),
        variant: z.enum(['h1', 'h2', 'h3', 'h4', 'p', 'lead', 'large', 'small', 'muted']).optional(),
        className: z.string().optional(),
      }),
      description: 'Typography element for text content',
    },
    Row: {
      props: z.object({
        gap: z.enum(['none', 'xs', 'sm', 'md', 'lg']).optional(),
        justify: z.enum(['start', 'center', 'end', 'between', 'around']).optional(),
        align: z.enum(['start', 'center', 'end', 'stretch']).optional(),
        className: z.string().optional(),
      }),
      slots: ['default'],
      description: 'Horizontal flex container',
    },
    Column: {
      props: z.object({
        gap: z.enum(['none', 'xs', 'sm', 'md', 'lg']).optional(),
        className: z.string().optional(),
      }),
      slots: ['default'],
      description: 'Vertical flex container',
    },
    Metric: {
      props: z.object({
        label: z.string(),
        value: z.union([z.string(), z.number()]),
        trend: z.enum(['up', 'down', 'flat']).optional(),
        format: z.enum(['number', 'percent', 'currency', 'bytes']).optional(),
        className: z.string().optional(),
      }),
      description: 'Key-value display with optional trend indicator',
    },
    Progress: {
      props: z.object({
        value: z.number(),
        max: z.number().optional(),
        className: z.string().optional(),
      }),
      description: 'Progress bar indicator',
    },
    Separator: {
      props: z.object({
        orientation: z.enum(['horizontal', 'vertical']).optional(),
        className: z.string().optional(),
      }),
      description: 'Visual divider line',
    },
    Alert: {
      props: z.object({
        title: z.string().optional(),
        description: z.string(),
        variant: z.enum(['default', 'destructive']).optional(),
        className: z.string().optional(),
      }),
      description: 'Notification/alert message box',
    },
    Input: {
      props: z.object({
        placeholder: z.string().optional(),
        type: z.enum(['text', 'password', 'email', 'number']).optional(),
        disabled: z.boolean().optional(),
        className: z.string().optional(),
        value: z.string().optional(),
      }),
      description: 'Text input field with two-way binding',
    },
    ScrollArea: {
      props: z.object({
        className: z.string().optional(),
        maxHeight: z.string().optional(),
      }),
      slots: ['default'],
      description: 'Scrollable container',
    },
    // Plugin-specific components
    PluginCard: {
      props: z.object({
        pluginId: z.string(),
        status: z.enum(['running', 'stopped', 'error', 'unknown']).optional(),
        className: z.string().optional(),
      }),
      slots: ['default'],
      description: 'Plugin status card with header showing state',
    },
    MetricGrid: {
      props: z.object({
        columns: z.number().optional(),
        className: z.string().optional(),
      }),
      slots: ['default'],
      description: 'Grid layout for metrics (2-4 columns)',
    },
    StatusBadge: {
      props: z.object({
        status: z.enum(['active', 'inactive', 'pending', 'error']),
        label: z.string().optional(),
      }),
      description: 'Colored status badge with semantic meaning',
    },
    ActionGroup: {
      props: z.object({
        className: z.string().optional(),
      }),
      slots: ['default'],
      description: 'Container for grouped action buttons',
    },
  },
});

export type Catalog = typeof catalog;
