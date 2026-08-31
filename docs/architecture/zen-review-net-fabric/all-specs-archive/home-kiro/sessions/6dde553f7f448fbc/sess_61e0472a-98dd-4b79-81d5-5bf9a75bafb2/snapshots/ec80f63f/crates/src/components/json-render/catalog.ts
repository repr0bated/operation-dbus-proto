/**
 * json-render Component Catalog for TchedRouter
 * Maps available shadcn/ui components for LLM-driven UI generation
 */
import { z } from 'zod';

// Component prop schemas
const CardProps = z.object({
  title: z.string().optional(),
  subtitle: z.string().optional(),
  className: z.string().optional(),
});

const BadgeProps = z.object({
  label: z.string(),
  variant: z.enum(['default', 'secondary', 'destructive', 'outline']).optional(),
  className: z.string().optional(),
});

const ButtonProps = z.object({
  label: z.string(),
  variant: z.enum(['default', 'destructive', 'outline', 'secondary', 'ghost', 'link']).optional(),
  size: z.enum(['default', 'sm', 'lg', 'icon']).optional(),
  disabled: z.boolean().optional(),
  className: z.string().optional(),
});

const TextProps = z.object({
  content: z.string(),
  variant: z.enum(['h1', 'h2', 'h3', 'h4', 'p', 'lead', 'large', 'small', 'muted']).optional(),
  className: z.string().optional(),
});

const RowProps = z.object({
  gap: z.enum(['none', 'xs', 'sm', 'md', 'lg']).optional(),
  justify: z.enum(['start', 'center', 'end', 'between', 'around']).optional(),
  align: z.enum(['start', 'center', 'end', 'stretch']).optional(),
  className: z.string().optional(),
});

const ColumnProps = z.object({
  gap: z.enum(['none', 'xs', 'sm', 'md', 'lg']).optional(),
  className: z.string().optional(),
});

const MetricProps = z.object({
  label: z.string(),
  value: z.union([z.string(), z.number()]),
  trend: z.enum(['up', 'down', 'flat']).optional(),
  format: z.enum(['number', 'percent', 'currency', 'bytes']).optional(),
  className: z.string().optional(),
});

const ProgressProps = z.object({
  value: z.number(),
  max: z.number().optional(),
  className: z.string().optional(),
});

const SeparatorProps = z.object({
  orientation: z.enum(['horizontal', 'vertical']).optional(),
  className: z.string().optional(),
});

const AlertProps = z.object({
  title: z.string().optional(),
  description: z.string(),
  variant: z.enum(['default', 'destructive']).optional(),
  className: z.string().optional(),
});

const InputProps = z.object({
  placeholder: z.string().optional(),
  type: z.enum(['text', 'password', 'email', 'number']).optional(),
  disabled: z.boolean().optional(),
  className: z.string().optional(),
});

const ScrollAreaProps = z.object({
  className: z.string().optional(),
  maxHeight: z.string().optional(),
});

// Export catalog definition for LLM system prompt
export const catalogDefinition = {
  Card: {
    description: 'Container with optional title/subtitle, supports children',
    props: CardProps,
    children: true,
  },
  Badge: {
    description: 'Small status indicator label',
    props: BadgeProps,
    children: false,
  },
  Button: {
    description: 'Clickable action button',
    props: ButtonProps,
    children: false,
    events: ['click'],
  },
  Text: {
    description: 'Typography element for text content',
    props: TextProps,
    children: false,
  },
  Row: {
    description: 'Horizontal flex container',
    props: RowProps,
    children: true,
  },
  Column: {
    description: 'Vertical flex container',
    props: ColumnProps,
    children: true,
  },
  Metric: {
    description: 'Key-value display with optional trend indicator',
    props: MetricProps,
    children: false,
  },
  Progress: {
    description: 'Progress bar indicator',
    props: ProgressProps,
    children: false,
  },
  Separator: {
    description: 'Visual divider line',
    props: SeparatorProps,
    children: false,
  },
  Alert: {
    description: 'Notification/alert message box',
    props: AlertProps,
    children: false,
  },
  Input: {
    description: 'Text input field with two-way binding',
    props: InputProps,
    children: false,
    events: ['change'],
  },
  ScrollArea: {
    description: 'Scrollable container',
    props: ScrollAreaProps,
    children: true,
  },
} as const;

// Type exports
export type CatalogComponents = typeof catalogDefinition;
export type ComponentName = keyof CatalogComponents;
