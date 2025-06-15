# JIAP MCP Server

Model Context Protocol (MCP) server for JIAP (Java Intelligence Analysis Platform), providing seamless integration between AI assistants and the JADX decompilation engine.

## Quick Start

```json
{
   "mcpServers": {
      "jiap-mcp-server": {
         "command": "uv",
         "args": [
            "--directory",
            "<dir to mcp server>",
            "run",
            "mcp_server.py"
         ]
      }
   }
}
```

## Configuration

### Environment Variables

- `JADX_SERVER_URL`: JADX server URL (default: http://127.0.0.1:8080)
- `LOG_LEVEL`: Logging level (default: ERROR)

### Requirements

- Python 3.10+
- JIAP Server running
- Dependencies: `httpx`, `fastmcp`

## License

Apache License - Part of JIAP (Java Intelligence Analysis Platform)
