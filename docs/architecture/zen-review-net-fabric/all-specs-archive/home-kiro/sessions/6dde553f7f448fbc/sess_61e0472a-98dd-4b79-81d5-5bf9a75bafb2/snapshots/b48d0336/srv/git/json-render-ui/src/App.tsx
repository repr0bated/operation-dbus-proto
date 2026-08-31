import { Renderer } from "@json-render/react";
import { defineRegistry } from "@json-render/react";
import { defineCatalog } from "@json-render/core";
import { z } from "zod";

const minimalCatalog = defineCatalog(
  { root: z.string(), elements: z.record(z.any()) },
  {
    components: {
      container: { slots: ["default"] },
      text: { props: z.object({ content: z.string() }) },
    },
  }
);

const minimalRegistry = defineRegistry(minimalCatalog, {
  components: {
    container: ({ slots }) => <div>{slots?.default}</div>,
    text: ({ props }) => <p>{props.content}</p>,
  },
});

const testSpec = {
  root: "main",
  elements: {
    main: {
      type: "container",
      slots: { default: ["greeting"] },
    },
    greeting: {
      type: "text",
      props: { content: "Hello - Renderer is working!" },
    },
  },
};

export function App() {
  return (
    <div className="min-h-screen bg-neutral-950">
      <Renderer spec={testSpec} registry={minimalRegistry.registry} />
    </div>
  );
}
