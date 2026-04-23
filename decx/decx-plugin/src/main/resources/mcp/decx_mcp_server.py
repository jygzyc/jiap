# -*- coding: utf-8 -*-
"""
DECX MCP Server - Model Context Protocol Server for Java Intelligence Analysis Platform

This module is part of DECX (Java Intelligence Analysis Platform), providing a comprehensive
set of tools for interacting with the JADX decompiler through its HTTP API. It enables AI
assistants to analyze and navigate Java applications, Android APKs, and other Java-based
code structures through the Model Context Protocol (MCP).

Part of DECX - Java Intelligence Analysis Platform
Repository: https://github.com/jygzyc/decx
"""

import requests
import os
import argparse
from pydantic import Field
from typing import Optional, Dict, Any
from fastmcp import FastMCP
from fastmcp.tools.tool import ToolResult

# Default configuration
DECX_BASE_URL = "http://127.0.0.1:25419"

mcp = FastMCP("DECX MCP Server")


async def request_to_decx(
    endpoint: str,
    json_data: Optional[Dict[str, Any]] = None,
    page: int = 1,
    api_type: str = "decx",
) -> ToolResult:
    try:
        # Directly fetch from server without caching
        url = f"{DECX_BASE_URL}/api/{api_type}/{endpoint.lstrip('/')}"

        # Include page parameter in the request JSON data, but don't override if already present
        request_data = json_data.copy() if json_data else {}
        if "page" not in request_data:
            request_data["page"] = page

        resp = requests.post(url, json=request_data, timeout=120)
        resp.raise_for_status()
        json_response = resp.json()

        # Return the response directly (decx_core handles slicing)
        return ToolResult(json_response)

    except Exception as e:
        return ToolResult({"error": str(e), "endpoint": endpoint})


#######################################################
# MCP Tool Definitions
#######################################################


# Basic JADX endpoints
@mcp.tool(
    name="get_classes",
    description="List all decompiled classes. Retrieve complete class list from project.",
)
async def get_classes(
    limit: Optional[int] = Field(
        None,
        description="Limit returned classes after package filtering",
    ),
    include_packages: Optional[list[str]] = Field(
        None,
        description="Only list classes under these Java packages",
    ),
    exclude_packages: Optional[list[str]] = Field(
        None,
        description="Exclude classes under these Java packages",
    ),
    regex: bool = Field(True, description="Treat filter values as regular expressions"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    filter_options = {
        "includes": include_packages or [],
        "excludes": exclude_packages or [],
    }
    if limit is not None:
        filter_options["limit"] = limit
    if not regex:
        filter_options["regex"] = False
    return await request_to_decx(
        "get_classes", json_data={"filter": filter_options}, page=page
    )


@mcp.tool(
    name="get_class_source",
    description="Get class source code. View Java or Smali decompiled class implementation, optionally limited to N lines.",
)
async def get_class_source(
    class_name: str = Field(
        description="Full class name (e.g., com.example.Myclass$Innerclass)"
    ),
    limit: Optional[int] = Field(
        None,
        description="Limit returned source lines",
    ),
    smali: bool = Field(
        False,
        description="Return Smali format (true) or Java (false, default)",
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    filter_options = {}
    if limit is not None:
        filter_options["limit"] = limit
    return await request_to_decx(
        "get_class_source",
        json_data={"cls": class_name, "smali": smali, "filter": filter_options},
        page=page,
    )


@mcp.tool(
    name="search_method",
    description="Find methods by name. Search method signatures across all classes.",
)
async def search_method(
    method_name: str = Field(description="Method name or partial name to search"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx(
        "search_method", json_data={"mth": method_name}, page=page
    )


@mcp.tool(
    name="search_class_key",
    description="Grep one class by keyword. Returns matching source lines and method signatures.",
)
async def search_class_key(
    class_name: str = Field(description="Full class name to grep"),
    key: str = Field(description="Regular expression to search within the class"),
    limit: int = Field(description="Limit returned grep line results"),
    case_sensitive: bool = Field(False, description="Use case-sensitive matching"),
    regex: bool = Field(True, description="Treat key as a regular expression"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    grep = {
        "limit": limit,
        "caseSensitive": case_sensitive,
        "regex": regex,
    }
    payload = {
        "cls": class_name,
        "key": key,
        "grep": grep,
    }
    return await request_to_decx("search_class_key", json_data=payload, page=page)


@mcp.tool(
    name="search_global_key",
    description="Search all class bodies by keyword.",
)
async def search_global_key(
    key: str = Field(description="Keyword to search globally"),
    limit: Optional[int] = Field(
        None,
        description="Limit returned matching results",
    ),
    include_packages: Optional[list[str]] = Field(
        None,
        description="Only search classes and methods under these Java packages",
    ),
    exclude_packages: Optional[list[str]] = Field(
        None,
        description="Exclude classes and methods under these Java packages",
    ),
    case_sensitive: bool = Field(False, description="Use case-sensitive matching"),
    regex: bool = Field(True, description="Treat key as a regular expression"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    search = {
        "includes": include_packages or [],
        "excludes": exclude_packages or [],
        "caseSensitive": case_sensitive,
        "regex": regex,
    }
    if limit is not None:
        search["limit"] = limit
    payload = {
        "key": key,
        "search": search,
    }
    return await request_to_decx("search_global_key", json_data=payload, page=page)


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
    return await request_to_decx(
        "get_method_source",
        json_data={"mth": method_name, "smali": smali},
        page=page,
    )


@mcp.tool(
    name="get_class_context",
    description="Get class context. View class symbol, fields, and methods.",
)
async def get_class_context(
    class_name: str = Field(
        description="Full class name (e.g., com.example.Myclass$Innerclass)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx(
        "get_class_context", json_data={"cls": class_name}, page=page
    )


@mcp.tool(
    name="get_method_context",
    description="Get method context. Returns method signature, callers, and callees.",
)
async def get_method_context(
    method_name: str = Field(
        description="Full signature (e.g., com.example.Myclass.myMethod(java.lang.String,int):String)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx(
        "get_method_context", json_data={"mth": method_name}, page=page
    )


@mcp.tool(
    name="get_method_cfg",
    description="Get method control flow graph as DOT source.",
)
async def get_method_cfg(
    method_name: str = Field(
        description="Full signature (e.g., com.example.Myclass.myMethod(java.lang.String,int):String)"
    ),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx(
        "get_method_cfg", json_data={"mth": method_name}, page=page
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
    return await request_to_decx(
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
    return await request_to_decx(
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
    return await request_to_decx(
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
    return await request_to_decx(
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
    return await request_to_decx(
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
    return await request_to_decx("get_selected_text", page=page)


@mcp.tool(
    name="selected_class",
    description="Get GUI selected class. Retrieve current class from JADX editor.",
)
async def selected_class(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx("get_selected_class", page=page)


# Android App specific endpoints
@mcp.tool(
    name="get_aidl",
    description="Get AIDL interfaces. Supports package include/exclude filters.",
)
async def get_aidl(
    limit: Optional[int] = Field(
        None,
        description="Limit returned AIDL interfaces after package filtering",
    ),
    include_packages: Optional[list[str]] = Field(
        None,
        description="Only include AIDL interfaces under these Java packages",
    ),
    exclude_packages: Optional[list[str]] = Field(
        None,
        description="Exclude AIDL interfaces under these Java packages",
    ),
    regex: bool = Field(True, description="Treat filter values as regular expressions"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    filter_options = {
        "includes": include_packages or [],
        "excludes": exclude_packages or [],
    }
    if limit is not None:
        filter_options["limit"] = limit
    if not regex:
        filter_options["regex"] = False
    return await request_to_decx("get_aidl", json_data={"filter": filter_options}, page=page)


@mcp.tool(
    name="get_app_manifest",
    description="Get Android manifest. View AndroidManifest.xml content.",
)
async def get_app_manifest(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx("get_app_manifest", page=page)


@mcp.tool(
    name="get_main_activity",
    description="Get main activity. View Android app launcher activity.",
)
async def get_main_activity(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx("get_main_activity", page=page)


@mcp.tool(
    name="get_application",
    description="Get Application class. View Android Application subclass.",
)
async def get_application(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx("get_application", page=page)


@mcp.tool(
    name="get_exported_components",
    description="Get exported components. Optionally filter by activity, service, receiver, or provider.",
)
async def get_exported_components(
    component_types: Optional[list[str]] = Field(
        None,
        description="Only include these component types: activity, service, receiver, provider",
    ),
    exclude_component_types: Optional[list[str]] = Field(
        None,
        description="Exclude these component types: activity, service, receiver, provider",
    ),
    regex: bool = Field(True, description="Treat component type filters as regular expressions"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    payload = {
        "includes": component_types or [],
        "excludes": exclude_component_types or [],
    }
    if not regex:
        payload["regex"] = False
    return await request_to_decx(
        "get_exported_components",
        json_data=payload,
        page=page,
    )


@mcp.tool(
    name="get_deep_links",
    description="Get deep links. List app URL schemes and intent filters.",
)
async def get_deep_links(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx("get_deep_links", page=page)


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
    return await request_to_decx(
        "get_system_service_impl", json_data={"iface": interface_name}, page=page
    )


# Resource Analysis endpoints
@mcp.tool(
    name="get_all_resources",
    description="List resource file names including resources.arsc sub-files. Supports file name include filters.",
)
async def get_all_resources(
    includes: Optional[list[str]] = Field(
        None,
        description="Only include resource file names matching these patterns",
    ),
    regex: bool = Field(True, description="Treat filter values as regular expressions"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    filter_options = {
        "includes": includes or [],
    }
    if not regex:
        filter_options["regex"] = False
    return await request_to_decx(
        "get_all_resources", json_data={"filter": filter_options}, page=page
    )


@mcp.tool(
    name="get_resource_file",
    description="Get resource file content. View a specific resource file by name.",
)
async def get_resource_file(
    resource_name: str = Field(description="Resource file name (e.g., res/values/strings.xml)"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx("get_resource_file", json_data={"res": resource_name}, page=page)


@mcp.tool(
    name="get_strings",
    description="Get app strings. Retrieve strings.xml content from app resources.",
)
async def get_strings(
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    return await request_to_decx("get_strings", page=page)


@mcp.tool(
    name="get_dynamic_receivers",
    description="Get dynamically registered BroadcastReceivers. Supports package include/exclude filters.",
)
async def get_dynamic_receivers(
    limit: Optional[int] = Field(
        None,
        description="Limit scanned classes after package filtering",
    ),
    include_packages: Optional[list[str]] = Field(
        None,
        description="Only scan classes under these Java packages",
    ),
    exclude_packages: Optional[list[str]] = Field(
        None,
        description="Exclude classes under these Java packages",
    ),
    regex: bool = Field(True, description="Treat filter values as regular expressions"),
    page: int = Field(1, description="Page number for pagination (default: 1)"),
) -> ToolResult:
    filter_options = {
        "includes": include_packages or [],
        "excludes": exclude_packages or [],
    }
    if limit is not None:
        filter_options["limit"] = limit
    if not regex:
        filter_options["regex"] = False
    return await request_to_decx("get_dynamic_receivers", json_data={"filter": filter_options}, page=page)


# Health check
@mcp.tool(
    name="health_check",
    description="Check server status. Verify DECX server connectivity.",
)
async def health_check() -> ToolResult:
    """Checks if the DECX server is running and returns its status."""
    try:
        resp = requests.get(f"{DECX_BASE_URL}/health", timeout=10)
        resp.raise_for_status()
        return ToolResult(resp.json())
    except Exception as e:
        return ToolResult({"status": "Error", "url": DECX_BASE_URL, "error": str(e)})


def parse_arguments():
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="DECX MCP Server - Model Context Protocol Server for Java Intelligence Analysis Platform"
    )
    parser.add_argument(
        "--url",
        type=str,
        default=DECX_BASE_URL,
        help=f"DECX server base URL (default: {DECX_BASE_URL})",
    )
    return parser.parse_args()


if __name__ == "__main__":
    args = parse_arguments()
    DECX_BASE_URL = args.url or os.getenv("DECX_URL", "")
    if not DECX_BASE_URL.startswith(("http://", "https://")):
        DECX_BASE_URL = f"http://{DECX_BASE_URL}"

    url_without_protocol = DECX_BASE_URL.split("://")[-1]
    if ":" in url_without_protocol:
        MCP_SERVER_PORT = int(url_without_protocol.split(":")[-1]) + 1
    else:
        MCP_SERVER_PORT = 25420

    mcp.run(transport="http", host="0.0.0.0", port=MCP_SERVER_PORT, show_banner=False)
