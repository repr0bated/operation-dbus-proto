import type { ComponentType } from "react";
import { Shell } from "./components/shell";
import { NavItem } from "./components/nav";
import { PageHeader } from "./components/page-header";
import { Container, Grid, Card } from "./components/layout";
import { Kv, Text, Badge, Stat } from "./components/data";
import { PluginState, StateValue } from "./components/live";

export const registry: Record<string, ComponentType<any>> = {
  shell: Shell,
  navItem: NavItem,
  pageHeader: PageHeader,
  container: Container,
  grid: Grid,
  card: Card,
  kv: Kv,
  text: Text,
  badge: Badge,
  stat: Stat,
  pluginState: PluginState,
  stateValue: StateValue,
};
