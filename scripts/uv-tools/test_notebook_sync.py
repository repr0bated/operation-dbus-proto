from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import notebook_sync


def busctl_reply(envelope: object) -> str:
    body = json.dumps(envelope, separators=(",", ":"))
    return "s " + json.dumps(body) + "\n"


class NotebookSyncHelpersTest(unittest.TestCase):
    def test_builds_argv_only_invoke_tool_call(self) -> None:
        command = notebook_sync.build_plugin_call_command(
            "invoke_tool",
            {
                "tool_name": "add_source_text",
                "arguments": {"title": "a file.md", "text": "$(not a shell)"},
            },
            bus_address="unix:path=/tmp/test-session-bus.sock",
        )

        self.assertEqual(command[0], "busctl")
        self.assertEqual(command[1], "--address=unix:path=/tmp/test-session-bus.sock")
        self.assertEqual(command[3], notebook_sync.BRIDGE_BUS_NAME)
        self.assertEqual(command[4], notebook_sync.COGNITIVE_OBJECT_PATH)
        self.assertEqual(command[5], notebook_sync.PLUGIN_INTERFACE)
        self.assertEqual(command[6:9], ["Call", "ss", "invoke_tool"])
        self.assertEqual(
            json.loads(command[9]),
            {
                "tool_name": "add_source_text",
                "arguments": {"title": "a file.md", "text": "$(not a shell)"},
            },
        )

    def test_parses_accountability_envelope(self) -> None:
        result = {"content": [{"type": "text", "text": '{"ok":true}'}]}
        output = busctl_reply(
            {
                "success": True,
                "event_id": 42,
                "event_hash": "abc",
                "result": result,
            }
        )
        self.assertEqual(notebook_sync.parse_call_reply(output), result)
        self.assertEqual(notebook_sync.unwrap_tool_payload(result), {"ok": True})

    def test_rejects_non_successful_bridge_envelope(self) -> None:
        output = busctl_reply({"success": False, "result": {}})
        with self.assertRaises(notebook_sync.NotebookSyncError):
            notebook_sync.parse_call_reply(output)

    def test_extracts_notebook_id_from_mcp_text_content(self) -> None:
        result = {
            "content": [
                {
                    "type": "text",
                    "text": '{"notebook":{"id":"123e4567-e89b-42d3-a456-426614174000"}}',
                }
            ]
        }
        self.assertEqual(
            notebook_sync.extract_notebook_id(result),
            "123e4567-e89b-42d3-a456-426614174000",
        )

    def test_saved_notebook_id_prefers_environment_then_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sources = Path(temporary)
            (sources / "NOTEBOOK_ID").write_text("file-id\n", encoding="utf-8")
            self.assertEqual(
                notebook_sync.saved_notebook_id(
                    sources, {"OP_NOTEBOOK_ID": "environment-id"}
                ),
                "environment-id",
            )
            self.assertEqual(notebook_sync.saved_notebook_id(sources, {}), "file-id")

    def test_notebook_env_quotes_single_quotes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sources = Path(temporary)
            notebook_sync.write_notebook_state("notebook-id", "Jeremy's Notes", sources)
            state = (sources / "notebook.env").read_text(encoding="utf-8")
            self.assertIn("NOTEBOOK_TITLE='Jeremy'\"'\"'s Notes'", state)
            self.assertEqual(
                notebook_sync.saved_notebook_id(sources, {}), "notebook-id"
            )


if __name__ == "__main__":
    unittest.main()
