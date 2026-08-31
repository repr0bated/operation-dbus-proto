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
  Card: ({ element, children }: RenderProps<{ title?: string; subtitle?: string; className?: string }>) => (
    <Card className={element.props.className}>
      {(element.props.title || element.props.subtitle) && (
        <CardHeader>
          {element.props.title && <CardTitle>{element.props.title}</CardTitle>}
          {element.props.subtitle && <CardDescription>{element.props.subtitle}</CardDescription>}
        </CardHeader>
      )}
      <CardContent>{children}</CardContent>
    </Card>
  ),

  Badge: ({ element }: RenderProps<{ label: string; variant?: 'default' | 'secondary' | 'destructive' | 'outline'; className?: string }>) => (
    <Badge variant={element.props.variant} className={element.props.className}>
      {element.props.label}
    </Badge>
  ),

  Button: ({ element, emit }: RenderProps<{ label: string; variant?: 'default' | 'destructive' | 'outline' | 'secondary' | 'ghost' | 'link'; size?: 'default' | 'sm' | 'lg' | 'icon'; disabled?: boolean; className?: string }>) => (
    <Button
      variant={element.props.variant}
      size={element.props.size}
      disabled={element.props.disabled}
      className={element.props.className}
      onClick={() => emit('click')}
    >
      {element.props.label}
    </Button>
  ),

  Text: ({ element }: RenderProps<{ content: string; variant?: keyof typeof textVariants; className?: string }>) => {
    const variant = element.props.variant || 'p';
    const Tag = variant.startsWith('h') ? (variant as 'h1' | 'h2' | 'h3' | 'h4') : 'p';
    return (
      <Tag className={cn(textVariants[variant], element.props.className)}>
        {element.props.content}
      </Tag>
    );
  },

  Row: ({ element, children }: RenderProps<{ gap?: keyof typeof gapClasses; justify?: keyof typeof justifyClasses; align?: keyof typeof alignClasses; className?: string }>) => (
    <div
      className={cn(
        'flex flex-row',
        gapClasses[element.props.gap || 'md'],
        justifyClasses[element.props.justify || 'start'],
        alignClasses[element.props.align || 'center'],
        element.props.className
      )}
    >
      {children}
    </div>
  ),

  Column: ({ element, children }: RenderProps<{ gap?: keyof typeof gapClasses; className?: string }>) => (
    <div className={cn('flex flex-col', gapClasses[element.props.gap || 'md'], element.props.className)}>
      {children}
    </div>
  ),

  Metric: ({ element }: RenderProps<{ label: string; value: string | number; trend?: 'up' | 'down' | 'flat'; format?: 'number' | 'percent' | 'currency' | 'bytes'; className?: string }>) => {
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

    const TrendIcon = element.props.trend === 'up' ? TrendingUp : element.props.trend === 'down' ? TrendingDown : Minus;
    const trendColor = element.props.trend === 'up' ? 'text-green-500' : element.props.trend === 'down' ? 'text-red-500' : 'text-muted-foreground';

    return (
      <div className={cn('flex flex-col', element.props.className)}>
        <span className="text-sm text-muted-foreground">{element.props.label}</span>
        <div className="flex items-center gap-2">
          <span className="text-2xl font-bold">{formatValue(element.props.value, element.props.format)}</span>
          {element.props.trend && <TrendIcon className={cn('h-4 w-4', trendColor)} />}
        </div>
      </div>
    );
  },

  Progress: ({ element }: RenderProps<{ value: number; max?: number; className?: string }>) => (
    <Progress value={element.props.value} max={element.props.max} className={element.props.className} />
  ),

  Separator: ({ element }: RenderProps<{ orientation?: 'horizontal' | 'vertical'; className?: string }>) => (
    <Separator orientation={element.props.orientation} className={element.props.className} />
  ),

  Alert: ({ element }: RenderProps<{ title?: string; description: string; variant?: 'default' | 'destructive'; className?: string }>) => (
    <Alert variant={element.props.variant} className={element.props.className}>
      {element.props.title && <AlertTitle>{element.props.title}</AlertTitle>}
      <AlertDescription>{element.props.description}</AlertDescription>
    </Alert>
  ),

  Input: ({ element, emit, bindings }: RenderProps<{ placeholder?: string; type?: string; disabled?: boolean; className?: string; value?: string }>) => (
    <Input
      type={element.props.type || 'text'}
      placeholder={element.props.placeholder}
      disabled={element.props.disabled}
      className={element.props.className}
      value={element.props.value || ''}
      onChange={(e) => emit('change')}
    />
  ),

  ScrollArea: ({ element, children }: RenderProps<{ className?: string; maxHeight?: string }>) => (
    <ScrollArea className={element.props.className} style={{ maxHeight: element.props.maxHeight }}>
      {children}
    </ScrollArea>
  ),
};

export type Registry = typeof registry;
