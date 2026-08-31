/**
 * Spec Generation Module
 *
 * Exports for generating and validating json-render specs from plugin projections.
 */
export { generatePluginPageSpec } from "./generate-plugin-page";
export { validateGeneratedSpec, type ValidationResult } from "./validate-spec";
export { usePluginPageSpec, type UsePluginPageSpecResult } from "./use-plugin-page-spec";
