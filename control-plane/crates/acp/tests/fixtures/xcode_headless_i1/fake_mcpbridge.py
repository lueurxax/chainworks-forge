"""Synthetic stdio peer. Never contacts Apple or opens a project."""
import json
import os
import sys
import time
from pathlib import Path

scenario = sys.argv[1]
for line in sys.stdin:
    if len(sys.argv) > 3:
        with open(sys.argv[3], "ab") as raw:
            raw.write(line.encode())
    try:
        request = json.loads(line)
    except json.JSONDecodeError:
        break
    if "id" not in request:
        continue
    response = {"jsonrpc": "2.0", "id": request["id"], "result": {
        "environment_clean": not any(k in os.environ for k in (
            "CHAINWORKS_MCP_TOKEN", "MCP_XCODE_SESSION_ID", "I1_SECRET_TEST"))}}
    if scenario.startswith("full"):
        with open(sys.argv[2], "a") as log:
            log.write(json.dumps(request) + "\n")
        method = request["method"]
        if method == "initialize":
            response["result"] = {"protocolVersion": "2025-06-18",
                "serverInfo": {"name": "xcode-tools", "version": "25317"}, "capabilities": {"tools": {}}}
        elif method == "tools/list":
            response["result"] = {"tools": json.loads(Path(__file__).with_name("apple-contract.json").read_text())["tools"]}
        elif method == "tools/call":
            params = request["params"]
            args = params["arguments"]
            if params["name"] == "XcodeOpenWorkspace":
                if scenario == "full_pending":
                    time.sleep(30)
                if scenario == "full_lost":
                    break
                if scenario == "full_malformed":
                    print("{broken", flush=True)
                    continue
                label = Path(args["path"]).parent.name
                result = {"workspaceIdentifier": label, "workspacePath": args["path"]}
            elif params["name"] == "XcodeRead":
                assert args["workspaceIdentifier"] in ("A", "B")
                assert args["filePath"] == "Fixture/Sources/Sentinel.txt"
                assert args["offset"] == 1 and args["limit"] == 8
                result = {"content": "     1\tCW_I1_" + args["workspaceIdentifier"] + "\n     2\t",
                    "filePath": args["filePath"], "fileSize": 8, "totalLines": 2, "linesRead": 2, "startLine": 1}
            else:
                raise AssertionError("unexpected tool")
            response["result"] = {"structuredContent": result, "isError": False}
            if scenario == "full_tool_error":
                response["result"] = {"isError": True, "content": [{"type": "text", "text": "synthetic error: " + "detail " * 1024}]}
        else:
            raise AssertionError("unexpected method")
    elif scenario == "flood":
        for _ in range(70):
            print(json.dumps({"jsonrpc": "2.0", "method": "notifications/progress"}), flush=True)
    elif scenario == "oversized":
        print(" " * (1024 * 1024 + 1), flush=True)
    elif scenario == "malformed":
        print("{broken", flush=True)
    elif scenario == "wrong_id":
        response["id"] += 1
    elif scenario == "eof":
        break
    elif scenario == "timeout":
        time.sleep(30)
    elif scenario == "changed":
        print(json.dumps({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"}), flush=True)
    notification = ""
    if (scenario == "full_list_changed" and request["method"] == "tools/list") or (
        scenario == "full_final_changed" and request.get("params", {}).get("name") == "XcodeRead"):
        notification = "\n" + json.dumps({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"})
    print(json.dumps(response) + notification, flush=True)
sys.exit(1 if scenario == "exit_one" else 0)
