/**
 * json-render Component Registry
 * Maps catalog components to shadcn/ui implementations
 */
import React from 'react';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { Separator } from '@/components/ui/separator';
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { cn } from '@/lib/utils';
import { TrendingUp, TrendingDown, Minus } from 'lucide-react';
import type { ComponentRenderProps } from '@json-render/react';

// Helper type for element with typed props
type RenderProps<P = Record<string, unknown>> = ComponentRenderProps<P>;

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

// Component implementations
export const registry = {
  Card: ({ props, children }: BaseComponentProps<{ title?: string; subtitle?: string; className?: string }>) => (
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

  Badge: ({ props }: BaseComponentProps<{ label: string; variant?: 'default' | 'secondary' | 'destructive' | 'outline'; className?: string }>) => (
    <Badge variant={props.variant} className={props.className}>
      {props.label}
    </Badge>
  ),

  Button: ({ props, emit }: BaseComponentProps<{ label: string; variant?: 'default' | 'destructive' | 'outline' | 'secondary' | 'ghost' | 'link'; size?: 'default' | 'sm' | 'lg' | 'icon'; disabled?: boolean; className?: string }>) => (
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

  Text: ({ props }: BaseComponentProps<{ content: string; variant?: keyof typeof textVariants; className?: string }>) => {
    const variant = props.variant || 'p';
    const Tag = variant.startsWith('h') ? (variant as 'h1' | 'h2' | 'h3' | 'h4') : 'p';
    return (
      <Tag className={cn(textVariants[variant], props.className)}>
        {props.content}
      </Tag>
    );
  },

  Row: ({ props, children }: BaseComponentProps<{ gap?: keyof typeof gapClasses; justify?: keyof typeof justifyClasses; align?: keyof typeof alignClasses; className?: string }>) => (
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

  Column: ({ props, children }: BaseComponentProps<{ gap?: keyof typeof gapClasses; className?: string }>) => (
    <div className={cn('flex flex-col', gapClasses[props.gap || 'md'], props.className)}>
      {children}
    </div>
  ),

  Metric: ({ props }: BaseComponentProps<{ label: string; value: string | number; trend?: 'up' | 'down' | 'flat'; format?: 'number' | 'percent' | 'currency' | 'bytes'; className?: string }>) => {
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

  Progress: ({ props }: BaseComponentProps<{ value: number; max?: number; className?: string }>) => (
    <Progress value={props.value} max={props.max} className={props.className} />
  ),

  Separator: ({ props }: BaseComponentProps<{ orientation?: 'horizontal' | 'vertical'; className?: string }>) => (
    <Separator orientation={props.orientation} className={props.className} />
  ),

  Alert: ({ props }: BaseComponentProps<{ title?: string; description: string; variant?: 'default' | 'destructive'; className?: string }>) => (
    <Alert variant={props.variant} className={props.className}>
      {props.title && <AlertTitle>{props.title}</AlertTitle>}
      <AlertDescription>{props.description}</AlertDescription>
    </Alert>
  ),

  Input: ({ props, emit, bindings }: BaseComponentProps<{ placeholder?: string; type?: string; disabled?: boolean; className?: string; value?: string }>) => (
    <Input
      type={props.type || 'text'}
      placeholder={props.placeholder}
      disabled={props.disabled}
      className={props.className}
      value={props.value || ''}
      onChange={(e) => emit('change')}
    />
  ),

  ScrollArea: ({ props, children }: BaseComponentProps<{ className?: string; maxHeight?: string }>) => (
    <ScrollArea className={props.className} style={{ maxHeight: props.maxHeight }}>
      {children}
    </ScrollArea>
  ),
};

export type Registry = typeof registry;
