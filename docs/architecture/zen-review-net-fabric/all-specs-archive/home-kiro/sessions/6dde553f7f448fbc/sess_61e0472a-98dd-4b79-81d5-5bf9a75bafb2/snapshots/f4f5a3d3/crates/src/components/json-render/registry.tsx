/**
 * json-render Component Registry
 * 
 * Uses defineRegistry from @json-render/react to map catalog
 * components to real shadcn/ui implementations.
 */
import React from 'react';
import { defineRegistry } from '@json-render/react';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { Separator } from '@/components/ui/separator';
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { cn } from '@/lib/utils';
import { TrendingUp, TrendingDown, Minus, Activity, Circle } from 'lucide-react';
import { catalog } from './catalog';

// Gap utilities
const gapClasses = {
  none: 'gap-0',
  xs: 'gap-1',
  sm: 'gap-2',
  md: 'gap-4',
  lg: 'gap-6',
};

const justifyClasses = {
  start: 'justify-start',
  center: 'justify-center',
  end: 'justify-end',
  between: 'justify-between',
  around: 'justify-around',
};

const alignClasses = {
  start: 'items-start',
  center: 'items-center',
  end: 'items-end',
  stretch: 'items-stretch',
};

// Text variant classes
const textVariants = {
  h1: 'scroll-m-20 text-4xl font-extrabold tracking-tight lg:text-5xl',
  h2: 'scroll-m-20 text-3xl font-semibold tracking-tight',
  h3: 'scroll-m-20 text-2xl font-semibold tracking-tight',
  h4: 'scroll-m-20 text-xl font-semibold tracking-tight',
  p: 'leading-7',
  lead: 'text-xl text-muted-foreground',
  large: 'text-lg font-semibold',
  small: 'text-sm font-medium leading-none',
  muted: 'text-sm text-muted-foreground',
};

// Status colors
const statusColors = {
  running: 'text-green-500',
  active: 'bg-green-500',
  stopped: 'text-muted-foreground',
  inactive: 'bg-muted-foreground',
  error: 'text-destructive',
  pending: 'bg-yellow-500',
  unknown: 'text-muted-foreground',
};

// Format value helper
const formatValue = (val: string | number, fmt?: string) => {
  if (typeof val === 'string') return val;
  switch (fmt) {
    case 'percent': return `${val}%`;
    case 'currency': return `$${val.toLocaleString()}`;
    case 'bytes': {
      const units = ['B', 'KB', 'MB', 'GB', 'TB'];
      let size = val;
      let unitIndex = 0;
      while (size >= 1024 && unitIndex < units.length - 1) {
        size /= 1024;
        unitIndex++;
      }
      return `${size.toFixed(1)} ${units[unitIndex]}`;
    }
    default: return val.toLocaleString();
  }
};

/**
 * Registry created with defineRegistry for type-safe component mapping.
 */
export const { registry } = defineRegistry(catalog, {
  components: {
    Card: ({ props, children }) => (
      <Card className={props.className}>
        {(props.title || props.subtitle) && (
          <CardHeader>
            {props.title && <CardTitle>{props.title}</CardTitle>}
            {props.subtitle && <CardDescription>{props.subtitle}</CardDescription>}
          </CardHeader>
        )}
        <CardContent>{children}</CardContent>
      </Card>
    ),

    Badge: ({ props }) => (
      <Badge variant={props.variant} className={props.className}>
        {props.label}
      </Badge>
    ),

    Button: ({ props, emit }) => (
      <Button
        variant={props.variant}
        size={props.size}
        disabled={props.disabled}
        className={props.className}
        onClick={() => emit('click')}
      >
        {props.label}
      </Button>
    ),

    Text: ({ props }) => {
      const variant = props.variant || 'p';
      const Tag = variant.startsWith('h') ? (variant as 'h1' | 'h2' | 'h3' | 'h4') : 'p';
      return (
        <Tag className={cn(textVariants[variant], props.className)}>
          {props.content}
        </Tag>
      );
    },

    Row: ({ props, children }) => (
      <div
        className={cn(
          'flex flex-row',
          gapClasses[props.gap || 'md'],
          justifyClasses[props.justify || 'start'],
          alignClasses[props.align || 'center'],
          props.className
        )}
      >
        {children}
      </div>
    ),

    Column: ({ props, children }) => (
      <div className={cn('flex flex-col', gapClasses[props.gap || 'md'], props.className)}>
        {children}
      </div>
    ),

    Metric: ({ props }) => {
      const TrendIcon = props.trend === 'up' ? TrendingUp : props.trend === 'down' ? TrendingDown : Minus;
      const trendColor = props.trend === 'up' ? 'text-green-500' : props.trend === 'down' ? 'text-red-500' : 'text-muted-foreground';

      return (
        <div className={cn('flex flex-col', props.className)}>
          <span className="text-sm text-muted-foreground">{props.label}</span>
          <div className="flex items-center gap-2">
            <span className="text-2xl font-bold">{formatValue(props.value, props.format)}</span>
            {props.trend && <TrendIcon className={cn('h-4 w-4', trendColor)} />}
          </div>
        </div>
      );
    },

    Progress: ({ props }) => (
      <Progress value={props.value} max={props.max} className={props.className} />
    ),

    Separator: ({ props }) => (
      <Separator orientation={props.orientation} className={props.className} />
    ),

    Alert: ({ props }) => (
      <Alert variant={props.variant} className={props.className}>
        {props.title && <AlertTitle>{props.title}</AlertTitle>}
        <AlertDescription>{props.description}</AlertDescription>
      </Alert>
    ),

    Input: ({ props, emit, bindings }) => (
      <Input
        type={props.type || 'text'}
        placeholder={props.placeholder}
        disabled={props.disabled}
        className={props.className}
        value={props.value || ''}
        onChange={() => emit('change')}
      />
    ),

    ScrollArea: ({ props, children }) => (
      <ScrollArea className={props.className} style={{ maxHeight: props.maxHeight }}>
        {children}
      </ScrollArea>
    ),

    // Plugin-specific components
    PluginCard: ({ props, children }) => {
      const statusIcon = {
        running: <Activity className="h-4 w-4 text-green-500 animate-pulse" />,
        stopped: <Circle className="h-4 w-4 text-muted-foreground" />,
        error: <Circle className="h-4 w-4 text-destructive" />,
        unknown: <Circle className="h-4 w-4 text-muted-foreground" />,
      };

      return (
        <Card className={props.className}>
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <CardTitle className="text-lg font-mono">{props.pluginId}</CardTitle>
              {props.status && statusIcon[props.status]}
            </div>
          </CardHeader>
          <CardContent>{children}</CardContent>
        </Card>
      );
    },

    MetricGrid: ({ props, children }) => (
      <div
        className={cn(
          'grid gap-4',
          props.columns === 2 ? 'grid-cols-2' :
          props.columns === 3 ? 'grid-cols-3' :
          props.columns === 4 ? 'grid-cols-4' :
          'grid-cols-2 md:grid-cols-4',
          props.className
        )}
      >
        {children}
      </div>
    ),

    StatusBadge: ({ props }) => {
      const colorMap = {
        active: 'bg-green-500/20 text-green-600 border-green-500/50',
        inactive: 'bg-muted text-muted-foreground',
        pending: 'bg-yellow-500/20 text-yellow-600 border-yellow-500/50',
        error: 'bg-destructive/20 text-destructive border-destructive/50',
      };

      return (
        <span className={cn(
          'inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium border',
          colorMap[props.status]
        )}>
          <span className={cn('h-1.5 w-1.5 rounded-full', statusColors[props.status])} />
          {props.label || props.status}
        </span>
      );
    },

    ActionGroup: ({ props, children }) => (
      <div className={cn('flex gap-2', props.className)}>
        {children}
      </div>
    ),
  },
});

export type Registry = typeof registry;
