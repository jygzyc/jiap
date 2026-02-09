# -*- coding: utf-8 -*-
"""
JIAP MCP Server - Model Context Protocol Server for Java Intelligence Analysis Platform

This module is part of JIAP (Java Intelligence Analysis Platform), providing a comprehensive
set of tools for interacting with the JADX decompiler through its HTTP API. It enables AI
assistants to analyze and navigate Java applications, Android APKs, and other Java-based
code structures through the Model Context Protocol (MCP).

Part of JIAP - Java Intelligence Analysis Platform
Repository: https://github.com/jygzyc/jiap
"""

import requests
import os
import argparse
from pydantic import Field
from typing import Optional, Dict, Any
from fastmcp import FastMCP
from fastmcp.tools.tool import ToolResult

# Default configuration
JIAP_BASE_URL = "http://127.0.0.1:25419"

mcp = FastMCP("JIAP MCP Server")


async def request_to_jiap(
    endpoint: str,
    json_data: Optional[Dict[str, Any]] = None,
    page: int = 1,
    api_type: str = "jiap",
) -> ToolResult:
    try:
        # Directly fetch from server without caching
        url = f"{JIAP_BASE_URL}/api/{api_type}/{endpoint.lstrip('/')}"

        # Include page parameter in the request JSON data, but don't override if already present
        request_data = json_data.copy() if json_data else {}
        if "page" not in request_data:
            request_data["page"] = page

        resp = requests.post(url, json=request_data, timeout=120)
        resp.raise_for_status()
        json_response = resp.json()

        if isinstance(json_response, dict) and "error" in json_response:
            return ToolResult({"error": json_response["error"], "endpoint": endpoint})

        # Return the response directly (jiap_core handles slicing)
        return ToolResult(json_response)

    except Exception as e:
        return ToolResult({"error": str(e), "endpoint": endpoint})


#######################################################
# MCP Tool Definitions
#######################################################


# Basic JADX endpoints
@mcp.tool(
    name="get_all_classes",
    description="List all decompiled classes. Retrieve complete class list from project.",
)
async def get_all_classes(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("get_all_classes", page=page)


@mcp.tool(
    name="get_class_source",
    description="Get class source code. View Java or Smali decompiled class implementation.",
)
async def get_class_source(
    class_name: str = Field(
        description="Full class name (e.g., com.example.Myclass$Innerclass)"
    ),
    smali: bool = Field(
        False,
        description="Return Smali format (true) or Java (false, default)",
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "get_class_source", json_data={"cls": class_name, "smali": smali}, page=page
    )


@mcp.tool(
    name="search_method",
    description="Find methods by name. Search method signatures across all classes.",
)
async def search_method(
    method_name: str = Field(description="Method name or partial name to search"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "search_method", json_data={"mth": method_name}, page=page
    )


@mcp.tool(
    name="search_class_key",
    description="Search class source by keyword. Find classes containing specific text/code.",
)
async def search_class_key(
    key: str = Field(description="Keyword to search within class source"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("search_class_key", json_data={"key": key}, page=page)


@mcp.tool(
    name="get_method_source",
    description="Get method source code. View Java or Smali implementation.",
)
async def get_method_source(
    method_name: str = Field(
        description="Full signature (e.g., com.example.Myclass.myMethod(String,int):String)"
    ),
    smali: bool = Field(
        False,
        description="Return Smali format (true) or Java (false, default)",
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "get_method_source",
        json_data={"mth": method_name, "smali": smali},
        page=page,
    )


@mcp.tool(
    name="get_class_info",
    description="Get class structure. View fields, methods.",
)
async def get_class_info(
    class_name: str = Field(
        description="Full class name (e.g., com.example.Myclass$Innerclass)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "get_class_info", json_data={"cls": class_name}, page=page
    )


@mcp.tool(
    name="get_method_xref",
    description="Find method usages. Locate all caller references (xref).",
)
async def get_method_xref(
    method_name: str = Field(
        description="Full signature (e.g., com.example.Myclass.myMethod(java.lang.String,int):String)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "get_method_xref", json_data={"mth": method_name}, page=page
    )


@mcp.tool(
    name="get_field_xref",
    description="Find field usages. Locate all field references (xref).",
)
async def get_field_xref(
    field_name: str = Field(
        description="Full field signature (e.g., com.example.Myclass.myField:int)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "get_field_xref", json_data={"fld": field_name}, page=page
    )


@mcp.tool(
    name="get_class_xref",
    description="Find class usages. Locate all caller references (xref).",
)
async def get_class_xref(
    class_name: str = Field(
        description="Full class name (e.g., com.example.Myclass$Innerclass)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "get_class_xref", json_data={"cls": class_name}, page=page
    )


@mcp.tool(
    name="get_implement",
    description="Find interface implementations. Get all classes implementing an interface.",
)
async def get_implement(
    interface_name: str = Field(
        description="Full interface name (e.g., com.example.IInterface)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "get_implement", json_data={"iface": interface_name}, page=page
    )


@mcp.tool(
    name="get_sub_classes",
    description="Find subclasses. Get all classes extending a superclass.",
)
async def get_sub_classes(
    class_name: str = Field(
        description="Full superclass name (e.g., com.example.MySuperClass)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "get_sub_classes", json_data={"cls": class_name}, page=page
    )


# UI Integration


@mcp.tool(
    name="selected_text",
    description="Get GUI selected text. Retrieve current selection from JADX editor.",
)
async def selected_text(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("selected_text", page=page)


@mcp.tool(
    name="selected_class",
    description="Get GUI selected class. Retrieve current class from JADX editor.",
)
async def selected_class(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("selected_class", page=page)


# Android App specific endpoints
@mcp.tool(
    name="get_app_manifest",
    description="Get Android manifest. View AndroidManifest.xml content.",
)
async def get_app_manifest(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("get_app_manifest", page=page)


@mcp.tool(
    name="get_main_activity",
    description="Get main activity. View Android app launcher activity.",
)
async def get_main_activity(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("get_main_activity", page=page)


@mcp.tool(
    name="get_application",
    description="Get Application class. View Android Application subclass.",
)
async def get_application(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("get_application", page=page)


@mcp.tool(
    name="get_exported_components",
    description="Get exported components. List activities, services, receivers, providers with permissions.",
)
async def get_exported_components(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("get_exported_components", page=page)


@mcp.tool(
    name="get_deep_links",
    description="Get deep links. List app URL schemes and intent filters.",
)
async def get_deep_links(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("get_deep_links", page=page)


# Android Framework specific endpoints
@mcp.tool(
    name="get_system_service_impl",
    description="Get system service implementation. Find Android framework interface implementation.",
)
async def get_system_service_impl(
    interface_name: str = Field(
        description="Full interface name (e.g., android.os.IMyService)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap(
        "get_system_service_impl", json_data={"iface": interface_name}, page=page
    )


# Vulnerability Mining endpoints
@mcp.tool(
    name="get_dynamic_receivers",
    description="Get dynamically registered BroadcastReceivers. Scan for registerReceiver calls.",
)
async def get_dynamic_receivers(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_jiap("get_dynamic_receivers", page=page)


# Health check
@mcp.tool(
    name="health_check",
    description="Check server status. Verify JIAP server connectivity.",
)
async def health_check() -> ToolResult:
    """Checks if the JIAP server is running and returns its status."""
    try:
        resp = requests.get(f"{JIAP_BASE_URL}/health", timeout=10)
        resp.raise_for_status()
        return ToolResult(resp.json())
    except Exception as e:
        return ToolResult({"status": "Error", "url": JIAP_BASE_URL, "error": str(e)})


def parse_arguments():
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="JIAP MCP Server - Model Context Protocol Server for Java Intelligence Analysis Platform"
    )
    parser.add_argument(
        "--url",
        type=str,
        default=JIAP_BASE_URL,
        help=f"JIAP server base URL (default: {JIAP_BASE_URL})",
    )
    return parser.parse_args()


if __name__ == "__main__":
    args = parse_arguments()
    JIAP_BASE_URL = args.url or os.getenv("JIAP_URL", "")
    if not JIAP_BASE_URL.startswith(("http://", "https://")):
        JIAP_BASE_URL = f"http://{JIAP_BASE_URL}"

    url_without_protocol = JIAP_BASE_URL.split("://")[-1]
    if ":" in url_without_protocol:
        MCP_SERVER_PORT = int(url_without_protocol.split(":")[-1]) + 1
    else:
        MCP_SERVER_PORT = 25420

    mcp.run(
        transport="http",
        host="0.0.0.0",
        port=MCP_SERVER_PORT,
        show_banner=False
    )
