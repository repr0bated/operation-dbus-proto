import { existsSync, readFileSync } from "fs";
import { describe, expect, it } from "vitest";

import { STREAM_PLUGIN_IDS } from "@/json-render/catalog/stream-plugins";

const BLOB_MANIFEST_PATH = "/dev/shm/opdbus/plugin-blobs/.manifest.json";

describe("STREAM_PLUGIN_IDS sync with blob catalog", () => {
  it.skipIf(!existsSync(BLOB_MANIFEST_PATH))(
    "every sealed plugin is in STREAM_PLUGIN_IDS",
    () => {
      const manifest = JSON.parse(readFileSync(BLOB_MANIFEST_PATH, "utf-8"));
      const sealedPlugins = Object.keys(manifest.plugins ?? {});
      const streamSet = new Set(STREAM_PLUGIN_IDS as readonly string[]);

      const missing = sealedPlugins.filter((id) => !streamSet.has(id));
      expect(missing, `Sealed plugins missing from STREAM_PLUGIN_IDS: ${missing.join(", ")}`).toEqual(
        [],
      );
    },
  );

  it.skipIf(!existsSync(BLOB_MANIFEST_PATH))(
    "every STREAM_PLUGIN_ID is a sealed plugin",
    () => {
      const manifest = JSON.parse(readFileSync(BLOB_MANIFEST_PATH, "utf-8"));
      const sealedPlugins = new Set(Object.keys(manifest.plugins ?? {}));

      const extra = (STREAM_PLUGIN_IDS as readonly string[]).filter((id) => !sealedPlugins.has(id));
      expect(extra, `STREAM_PLUGIN_IDS entries not in sealed catalog: ${extra.join(", ")}`).toEqual(
        [],
      );
    },
  );
});
