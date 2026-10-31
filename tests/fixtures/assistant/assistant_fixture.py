# Controlled JSONL subprocess for AssistantSessionTests. No model or project writes.
import json
import os
import sys
import time


def emit(value, split=False):
    data = (json.dumps(value, ensure_ascii=False) + "\n").encode("utf-8")
    if split:
        for byte in data:
            os.write(1, bytes([byte]))
            time.sleep(0.0001)
    else:
        os.write(1, data)


def response(request, result):
    emit({"id": request["id"], "result": result})


def notification(method, params):
    emit({"method": method, "params": params})


thread_id = "fixture-thread"
turn_id = "fixture-turn"
thread_count = 0
initialized = False
pending_approval = False
mcp_approval_granted = False

for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    params = request.get("params", {})
    if method == "initialize":
        assert sys.argv[1:] == ["app-server", "--listen", "stdio://"]
        assert params["clientInfo"]["name"] == "latexide_macos"
        if SCENARIO == "silent":
            time.sleep(10)
        elif SCENARIO == "partial":
            os.write(1, b'{"id":')
            sys.exit(0)
        elif SCENARIO == "invalid":
            os.write(1, b'\xff\n')
            time.sleep(10)
        elif SCENARIO == "oversized":
            os.write(1, b'x' * 2048)
            time.sleep(10)
        else:
            response(request, {"userAgent": "codex-fixture/1.0 pid=" + str(os.getpid()), "platformFamily": "unix", "platformOs": "macos", "codexHome": os.getcwd()})
    elif method == "initialized":
        initialized = True
    elif method == "config/read":
        assert initialized
        response(request, {"config": "invalid"} if SCENARIO == "malformed-config" else {"config": {"mcp_servers": {"unrelated": {"command": "never-launch-this", "enabled": True}}}})
    elif method == "thread/start":
        if SCENARIO == "malformed-config":
            emit({"id": request["id"], "error": {"code": -32000, "message": "fixture: unsafe thread request after malformed config"}})
            continue
        assert initialized
        assert params["sandbox"] == "read-only"
        assert params["approvalPolicy"] == "never"
        assert params["ephemeral"] is True
        assert "model" not in params
        config = params["config"]
        assert config["features.shell_tool"] is False
        assert config["features.apps"] is False
        assert config["mcp_servers"]["unrelated"]["enabled"] is False
        bridge = config["mcp_servers"]["latexide"]
        if SCENARIO == "mcp-approval":
            expected_tools = {"workspace_info", "list_project_files", "read_document", "apply_edits", "save_document", "get_diagnostics", "request_build", "get_operation", "cancel_build"}
            mcp_approval_granted = (
                set(bridge.get("enabled_tools", [])) == expected_tools
                and bridge.get("default_tools_approval_mode") == "prompt"
                and set(bridge.get("tools", {})) == expected_tools
                and all(bridge["tools"][name].get("approval_mode") == "approve" for name in expected_tools)
            )
        assert bridge["enabled"] is True
        assert bridge["required"] is True
        assert bridge["args"][0] == "--session"
        assert os.path.realpath(bridge["args"][1]) == os.path.join(os.getcwd(), "private-session.json")
        assert os.path.realpath(bridge["command"]) == os.path.realpath(sys.argv[0])
        assert "save_document" in params["developerInstructions"]
        thread_count += 1
        thread_id = "fixture-thread-" + str(thread_count)
        response(request, {"thread": {"id": thread_id, "turns": [], "createdAt": 1, "updatedAt": 1, "cwd": os.getcwd(), "ephemeral": True, "cliVersion": "fixture/1.0", "modelProvider": "fixture", "preview": "", "sessionId": "fixture-session", "status": {"type": "idle"}, "source": "appServer", "path": None, "name": None}, "model": "configured-fixture-model", "modelProvider": "fixture", "cwd": os.getcwd(), "sandbox": {"type": "readOnly"}, "approvalPolicy": "never", "approvalsReviewer": "user"})
    elif method == "turn/start":
        assert params["threadId"] == thread_id
        assert params["approvalPolicy"] == "never"
        assert params["sandboxPolicy"] == {"type": "readOnly", "networkAccess": False}
        assert "model" not in params
        if SCENARIO == "cancel":
            time.sleep(0.15)
        elif SCENARIO == "late":
            time.sleep(0.2)
        response(request, {"turn": {"id": turn_id, "items": [], "status": "inProgress"}})
        if SCENARIO == "mcp-approval" and not mcp_approval_granted:
            notification("turn/completed", {"threadId": thread_id, "turn": {"id": turn_id, "status": "failed", "error": {"message": "fixture: MCP approval required but policy is never; IDE-only grant missing"}}})
            continue
        if SCENARIO == "cancel":
            continue
        if SCENARIO == "approval":
            emit({"id": "approval-request", "method": "item/commandExecution/requestApproval", "params": {"threadId": thread_id, "turnId": turn_id, "itemId": "cmd"}})
            pending_approval = True
            continue
        if SCENARIO == "failure":
            notification("turn/completed", {"threadId": thread_id, "turn": {"id": turn_id, "items": [], "status": "failed", "error": {"message": "fixture authentication failed"}}})
            continue
        item = {"id": "mcp-read", "type": "mcpToolCall", "server": "latexide", "tool": "read_document", "arguments": {}, "status": "inProgress"}
        notification("item/started", {"threadId": thread_id, "turnId": turn_id, "item": item, "startedAtMs": 1})
        item["status"] = "completed"
        notification("item/completed", {"threadId": thread_id, "turnId": turn_id, "item": item, "completedAtMs": 2})
        text = "Árvíz 😀 kész." if SCENARIO != "long" else ("ő😀" * 2048)
        notification("item/started", {"threadId": thread_id, "turnId": turn_id, "item": {"id": "agent", "type": "agentMessage", "text": ""}, "startedAtMs": 3})
        if SCENARIO == "long":
            for start in range(0, len(text), 200):
                notification("item/agentMessage/delta", {"threadId": thread_id, "turnId": turn_id, "itemId": "agent", "delta": text[start:start + 200]})
        else:
            emit({"method": "item/agentMessage/delta", "params": {"threadId": thread_id, "turnId": turn_id, "itemId": "agent", "delta": text}}, split=True)
        notification("item/completed", {"threadId": thread_id, "turnId": turn_id, "item": {"id": "agent", "type": "agentMessage", "text": text}, "completedAtMs": 4})
        notification("turn/completed", {"threadId": thread_id, "turn": {"id": turn_id, "items": [], "status": "completed"}})
    elif method == "turn/interrupt":
        assert params == {"threadId": thread_id, "turnId": turn_id}
        response(request, {})
        notification("turn/completed", {"threadId": thread_id, "turn": {"id": turn_id, "items": [], "status": "interrupted"}})
        notification("item/agentMessage/delta", {"threadId": thread_id, "turnId": turn_id, "itemId": "late", "delta": "late-after-cancel"})
    elif request.get("id") == "approval-request":
        assert pending_approval and request["result"]["decision"] == "decline"
        emit({"id": "unknown-request", "method": "future/unsupported", "params": {}})
    elif request.get("id") == "unknown-request":
        assert request["error"]["code"] == -32601
        notification("item/agentMessage/delta", {"threadId": thread_id, "turnId": turn_id, "itemId": "agent", "delta": "requests-refused"})
        notification("turn/completed", {"threadId": thread_id, "turn": {"id": turn_id, "items": [], "status": "completed"}})
    else:
        raise AssertionError("Unexpected request: " + str(method))
