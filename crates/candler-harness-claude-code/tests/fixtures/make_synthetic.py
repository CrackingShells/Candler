# /// script
# requires-python = ">=3.11"
# ///
"""Write the three synthetic streams, `synthetic_hooks`, `synthetic_mcp` and `synthetic_orphans`.

Usage: uv run make_synthetic.py

The streams hold the shapes the golden corpus lacks and that Claude Code documents for
`--output-format stream-json`: hook lifecycle lines (`system` lines with subtype `hook_started`
and `hook_response`), `mcp__<server>__<tool>` calls, a denial inside a subagent, a subagent
that starts a subagent, a result of subtype `error_max_turns`, a second session in one stream,
a blank line, a line that is not JSON, a line of an unknown `type`, a user line that answers
three calls with two denials listed in another order than the calls, a `tool_progress` line
inside a subagent, and (`synthetic_orphans`) frames that carry no `session_id` and come before
the first init line. Every line is authored
here from those shapes; none is captured from a run. The output is deterministic: gzip headers
carry no time and no file name.
"""

import gzip
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
VERSION = "2.1.230"


def usage(inp: int, out: int, read: int, create: int) -> dict:
    return {
        "input_tokens": inp,
        "output_tokens": out,
        "cache_read_input_tokens": read,
        "cache_creation_input_tokens": create,
    }


class Stream:
    def __init__(self, session: str) -> None:
        self.session = session
        self.lines: list[str] = []
        self.clock = 0

    def raw(self, text: str) -> None:
        self.lines.append(text)

    def put(self, obj: dict, session: str | None = None, stamp: bool = True,
            bare: bool = False) -> None:
        obj = dict(obj)
        if not bare:
            obj.setdefault("session_id", session or self.session)
        obj.setdefault("uuid", f"00000000-0000-4000-8000-{len(self.lines):012d}")
        if stamp and obj["type"] in ("assistant", "user"):
            self.clock += 1
            obj.setdefault("timestamp", f"2026-01-02T03:04:{self.clock:02d}.000Z")
        self.lines.append(json.dumps(obj, ensure_ascii=False, separators=(",", ":")))

    def init(self, session: str | None = None) -> None:
        self.put(
            {
                "type": "system",
                "subtype": "init",
                "cwd": "/work",
                "model": "claude-synthetic-1",
                "claude_code_version": VERSION,
                "tools": ["Bash", "Agent"],
                "mcp_servers": [],
            },
            session,
        )

    def hook(self, name: str, event: str, outcome: str, code: int) -> None:
        hook_id = f"hook-{len(self.lines):04d}"
        self.put({"type": "system", "subtype": "hook_started", "hook_id": hook_id,
                  "hook_name": name, "hook_event": event})
        self.put({"type": "system", "subtype": "hook_response", "hook_id": hook_id,
                  "hook_name": name, "hook_event": event, "output": "", "stdout": "",
                  "stderr": "", "exit_code": code, "outcome": outcome})

    def assistant(self, mid: str, blocks: list[dict], parent: str | None = None,
                  use: dict | None = None, bare: bool = False) -> None:
        self.put({"type": "assistant", "parent_tool_use_id": parent,
                  "message": {"id": mid, "type": "message", "role": "assistant",
                              "model": "claude-synthetic-1", "content": blocks,
                              "usage": use or usage(1, 1, 0, 0)}}, bare=bare)

    def tool_results(self, ids: list[str], meta: list[dict]) -> None:
        """One user line that answers several calls; `meta` is listed in its own order."""
        blocks = [{"type": "tool_result", "tool_use_id": i, "content": "x"} for i in ids]
        self.put({"type": "user", "parent_tool_use_id": None,
                  "message": {"role": "user", "content": blocks},
                  "tool_result_meta": meta})

    def tool_result(self, use_id: str, text: str, parent: str | None = None,
                    error: bool = False, denial: str | None = None) -> None:
        block = {"type": "tool_result", "tool_use_id": use_id, "content": text}
        if error:
            block["is_error"] = True
        obj = {"type": "user", "parent_tool_use_id": parent,
               "message": {"role": "user", "content": [block]}}
        if denial:
            obj["tool_result_meta"] = [{"id": use_id, "non_execution_kind": denial}]
        self.put(obj)

    def write(self, name: str) -> None:
        target = HERE / "streams" / name / "agent_stream.jsonl.gz"
        target.parent.mkdir(parents=True, exist_ok=True)
        data = ("\n".join(self.lines) + "\n").encode()
        with open(target, "wb") as handle:
            with gzip.GzipFile(filename="", mode="wb", fileobj=handle, mtime=0) as gz:
                gz.write(data)


def tool(use_id: str, name: str, **inp) -> dict:
    return {"type": "tool_use", "id": use_id, "name": name, "input": inp}


def text(value: str) -> dict:
    return {"type": "text", "text": value}


def hooks() -> None:
    s = Stream("11111111-1111-4111-8111-111111111111")
    s.init()
    s.hook("SessionStart:startup", "SessionStart", "success", 0)
    s.put({"type": "user", "parent_tool_use_id": None,
           "message": {"role": "user", "content": "Run the checks."}})
    # One message split over two lines that share a message id and repeat its usage.
    s.assistant("msg_h1", [text("Starting a helper.")], use=usage(10, 5, 100, 20))
    s.assistant("msg_h1", [tool("toolu_h_agent", "Agent", prompt="inspect")],
                use=usage(10, 5, 100, 20))
    s.put({"type": "system", "subtype": "task_started", "task_id": "t1",
           "tool_use_id": "toolu_h_agent"})
    s.put({"type": "user", "parent_tool_use_id": "toolu_h_agent",
           "message": {"role": "user", "content": [text("inspect")]}})
    s.hook("PreToolUse:Bash", "PreToolUse", "blocking", 2)
    s.assistant("msg_h2", [tool("toolu_h_child_bash", "Bash", command="rm -rf /")],
                parent="toolu_h_agent", use=usage(4, 3, 50, 0))
    s.tool_result("toolu_h_child_bash", "blocked by hook", parent="toolu_h_agent",
                  error=True, denial="permission-rule")
    s.tool_result("toolu_h_agent", "helper done")
    s.raw("")
    s.raw("this line is not json")
    s.put({"type": "future_event", "detail": {"nested": [1, 2, 3], "none": None}})
    s.assistant("msg_h3", [{"type": "thinking", "thinking": "", "signature": "x"}],
                use=usage(6, 2, 70, 5))
    s.assistant("msg_h3", [tool("toolu_h_bash", "Bash", command="false"),
                           tool("toolu_h_rm", "Bash", command="rm -r .")],
                use=usage(6, 2, 70, 5))
    s.tool_result("toolu_h_bash", "exit 1", error=True)
    s.tool_result("toolu_h_rm", "needs approval", error=True, denial="user-rejected")
    # Three calls answered by one line; the two denials are listed in the other order.
    s.assistant("msg_h4", [tool("toolu_h_ok", "Bash", command="true"),
                           tool("toolu_h_c", "Bash", command="rm a"),
                           tool("toolu_h_d", "Bash", command="rm b")],
                use=usage(2, 2, 10, 0))
    s.tool_results(["toolu_h_ok", "toolu_h_c", "toolu_h_d"],
                   [{"id": "toolu_h_d", "non_execution_kind": "user-rejected"},
                    {"id": "toolu_h_c", "non_execution_kind": "permission-rule"}])
    s.put({"type": "result", "subtype": "success", "is_error": False, "num_turns": 2,
           "duration_ms": 10, "result": "done", "stop_reason": "end_turn",
           "total_cost_usd": 0.25, "usage": usage(20, 40, 220, 25),
           "permission_denials": [
               {"tool_name": "Bash", "tool_use_id": "toolu_h_child_bash",
                "tool_input": {"command": "rm -rf /"}},
               {"tool_name": "Bash", "tool_use_id": "toolu_h_rm",
                "tool_input": {"command": "rm -r ."}}],
           "api_error_status": None, "terminal_reason": "completed"})
    s.write("synthetic_hooks")


def mcp() -> None:
    s = Stream("22222222-2222-4222-8222-222222222222")
    s.init()
    s.put({"type": "user", "parent_tool_use_id": None,
           "message": {"role": "user", "content": [text("List the cluster.")]}})
    s.assistant("msg_m1", [tool("toolu_m_fac", "mcp__rikyu-hpc__get_facility"),
                           tool("toolu_m_ls", "mcp__plugin_banyan_banyan-hpc__fs_ls", path="/")],
                use=usage(8, 4, 90, 10))
    s.tool_result("toolu_m_fac", "{\"name\":\"rikyu\"}")
    s.tool_result("toolu_m_ls", "ls failed", error=True)
    s.assistant("msg_m2", [tool("toolu_m_n1", "Agent", prompt="outer")],
                use=usage(5, 2, 40, 0))
    s.assistant("msg_m3", [tool("toolu_m_n2", "Agent", prompt="inner")],
                parent="toolu_m_n1", use=usage(3, 1, 30, 0))
    s.assistant("msg_m4", [tool("toolu_m_deep", "Bash", command="ls")],
                parent="toolu_m_n2", use=usage(2, 1, 20, 0))
    # A heartbeat of the Bash call that the grandchild holds.
    s.put({"type": "tool_progress", "tool_use_id": "toolu_m_deep",
           "parent_tool_use_id": "toolu_m_deep", "tool_name": "Bash",
           "elapsed_time_seconds": 2})
    s.tool_result("toolu_m_deep", "ok", parent="toolu_m_n2")
    s.tool_result("toolu_m_n1", "outer done")
    s.put({"type": "result", "subtype": "error_max_turns", "is_error": True, "num_turns": 3,
           "duration_ms": 10, "stop_reason": "tool_use", "total_cost_usd": 0.1,
           "usage": usage(13, 8, 130, 10), "permission_denials": [],
           "api_error_status": None, "terminal_reason": "max_turns"})
    # A second session in the same stream that never reaches a result.
    s.init("33333333-3333-4333-8333-333333333333")
    s.put({"type": "user", "parent_tool_use_id": None,
           "message": {"role": "user", "content": "second"}},
          "33333333-3333-4333-8333-333333333333")
    s.write("synthetic_mcp")


def orphans() -> None:
    s = Stream("44444444-4444-4444-8444-444444444444")
    # Two frames with no session_id before the first init line: a result and a message.
    s.put({"type": "result", "subtype": "error_max_turns", "is_error": True, "num_turns": 1,
           "duration_ms": 10, "stop_reason": "tool_use", "total_cost_usd": 0.5,
           "usage": usage(7, 3, 70, 9), "permission_denials": [],
           "api_error_status": None, "terminal_reason": "max_turns"}, bare=True)
    s.assistant("msg_o1", [text("Before any init.")], use=usage(4, 2, 40, 6), bare=True)
    s.init()
    s.put({"type": "user", "parent_tool_use_id": None,
           "message": {"role": "user", "content": "continue"}})
    # The message msg_o1 continues with a later usage; only its first line counts.
    s.assistant("msg_o1", [tool("toolu_o_bash", "Bash", command="ls")], use=usage(4, 9, 40, 6))
    s.assistant("msg_o2", [text("done")], use=usage(2, 1, 20, 0))
    s.write("synthetic_orphans")


if __name__ == "__main__":
    hooks()
    mcp()
    orphans()
