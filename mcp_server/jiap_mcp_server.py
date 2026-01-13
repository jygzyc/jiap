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

# Default JIAP server configuration
DEFAULT_JIAP_HOST = "127.0.0.1"
DEFAULT_JIAP_PORT = 25419
MCP_SERVER_PORT = 25420

# Global variables (will be set in main)
JIAP_HOST = DEFAULT_JIAP_HOST
JIAP_PORT = DEFAULT_JIAP_PORT
JIAP_BASE_URL = None  # Will be constructed after parsing arguments

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
    description="Retrieves all available classes in the decompiled project. Supports pagination via the page parameter (default: 1).",
)
async def get_all_classes(
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap("get_all_classes", page=page)


@mcp.tool(
    name="get_class_source",
    description="Retrieves the source code of a specific class (e.g., com.example.Myclass$Innerclass) in Smali or Java format. Supports pagination via the page parameter (default: 1).",
)
async def get_class_source(
    class_name: str = Field(
        description="Full name of the class, e.g., com.example.Myclass$Innerclass"
    ),
    smali: bool = Field(
        False,
        description="Whether to retrieve the source in Smali format (True) or Java format (False), default is False",
    ),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap(
        "get_class_source", json_data={"class": class_name, "smali": smali}, page=page
    )


@mcp.tool(
    name="search_method",
    description="Searches for methods matching the given method_name string, e.g., doSomething matches com.example.Myservice.doSomething(java.lang.String, int):int. Supports pagination via the page parameter (default: 1).",
)
async def search_method(
    method_name: str = Field(description="Method name or partial name to search for"),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap(
        "search_method", json_data={"method": method_name}, page=page
    )


@mcp.tool(
    name="search_class_key",
    description="Searches for classes whose source code contains the specified keyword. The search is case-insensitive and looks for the keyword within the class code content. Supports pagination via the page parameter (default: 1).",
)
async def search_class_key(
    key: str = Field(description="Keyword to search for within class source code"),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap("search_class_key", json_data={"key": key}, page=page)


@mcp.tool(
    name="get_method_source",
    description="Retrieves the source code of a specific method in Java or Smali format. Provide method_name as 'className.methodName(paramType):returnType', e.g., com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String. Supports pagination via the page parameter (default: 1).",
)
async def get_method_source(
    method_name: str = Field(
        description="Full method signature, e.g., com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String"
    ),
    smali: bool = Field(
        False,
        description="Whether to retrieve the source in Smali format (True) or Java format (False), default is False",
    ),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap(
        "get_method_source",
        json_data={"method": method_name, "smali": smali},
        page=page,
    )


@mcp.tool(
    name="get_class_info",
    description="Get a specific class information, such as fields and methods, by its full name in the decompiled project, e.g., com.example.myclass. Supports pagination via the page parameter (default: 1).",
)
async def get_class_info(
    class_name: str = Field(
        description="Full name of the class, e.g., com.example.Myclass$Innerclass"
    ),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap(
        "get_class_info", json_data={"class": class_name}, page=page
    )


@mcp.tool(
    name="get_method_xref",
    description="Retrieves cross-references (usage locations) for a specific method. Provide method_name as 'className.methodName(paramType):returnType', e.g., com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String. Supports pagination via the page parameter (default: 1).",
)
async def get_method_xref(
    method_name: str = Field(
        description="Full method signature, e.g., com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String"
    ),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap(
        "get_method_xref", json_data={"method": method_name}, page=page
    )


@mcp.tool(
    name="get_class_xref",
    description="Retrieves cross-references (usage locations) for a specific class. Provide class_name as 'com.example.Myclass$Innerclass', e.g., com.example.Myclass$Innerclass. Supports pagination via the page parameter (default: 1).",
)
async def get_class_xref(
    class_name: str = Field(
        description="Full name of the class, e.g., com.example.Myclass$Innerclass"
    ),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap(
        "get_class_xref", json_data={"class": class_name}, page=page
    )


@mcp.tool(
    name="get_implement",
    description="Retrieves implementing classes for a specific interface, e.g., com.example.IInterface. Supports pagination via the page parameter (default: 1).",
)
async def get_implement(
    interface_name: str = Field(
        description="Full name of the interface, e.g., com.example.IInterface"
    ),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap(
        "get_implement", json_data={"interface": interface_name}, page=page
    )


@mcp.tool(
    name="get_sub_classes",
    description="Retrieves subclasses for a specific superclass, e.g., com.example.MySuperClass. Supports pagination via the page parameter (default: 1).",
)
async def get_sub_classes(
    class_name: str = Field(
        description="Full name of the superclass, e.g., com.example.MySuperClass"
    ),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap(
        "get_sub_classes", json_data={"class": class_name}, page=page
    )


# UI Integration


@mcp.tool(
    name="selected_text",
    description="Retrieves the currently selected text in the JADX GUI. Supports pagination via the page parameter (default: 1).",
)
async def selected_text(
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap("selected_text", page=page)


@mcp.tool(
    name="selected_class",
    description="Retrieves the currently selected class in the JADX GUI. Supports pagination via the page parameter (default: 1).",
)
async def selected_class(
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap("selected_class", page=page)


# Android App specific endpoints
@mcp.tool(
    name="get_app_manifest",
    description="Retrieves the Android application manifest (AndroidManifest.xml) content. Supports pagination via the page parameter (default: 1).",
)
async def get_app_manifest(
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap("get_app_manifest", page=page)


@mcp.tool(
    name="get_main_activity",
    description="Retrieves the main activity of the Android application. Supports pagination via the page parameter (default: 1).",
)
async def get_main_activity(
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap("get_main_activity", page=page)


@mcp.tool(
    name="get_application",
    description="Retrieves the Android application class and its information. Supports pagination via the page parameter (default: 1).",
)
async def get_application(
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap("get_application", page=page)


# Android Framework specific endpoints
@mcp.tool(
    name="get_system_service_impl",
    description="Retrieves system service implementation details of an interface, e.g., android.os.IMyService. Supports pagination via the page parameter (default: 1).",
)
async def get_system_service_impl(
    interface_name: str = Field(
        description="Full name of the system service interface, e.g., android.os.IMyService"
    ),
    page: int = Field(1, description="Page number for pagination, default is 1"),
) -> ToolResult:
    return await request_to_jiap(
        "get_system_service_impl", json_data={"interface": interface_name}, page=page
    )


# Health check
@mcp.tool(
    name="health_check",
    description="Checks if the JIAP server is running and returns its status.",
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
        "--jiap-host", type=str, help=f"JIAP server host (default: {DEFAULT_JIAP_HOST})"
    )
    parser.add_argument(
        "--jiap-port", type=int, help=f"JIAP server port (default: {DEFAULT_JIAP_PORT})"
    )
    parser.add_argument(
        "--jiap-url",
        type=str,
        help=f"Full JIAP server URL (overrides --jiap-host and --jiap-port)",
    )
    parser.add_argument(
        "--mcp-port",
        type=int,
        help=f"Port for this MCP server to listen on (default: JIAP_PORT + 1)",
    )
    return parser.parse_args()


if __name__ == "__main__":
    args = parse_arguments()

    env_jiap_port = os.getenv("JIAP_PORT")
    if args.jiap_port:
        JIAP_PORT = args.jiap_port
    elif env_jiap_port:
        try:
            JIAP_PORT = int(env_jiap_port)
        except ValueError:
            JIAP_PORT = DEFAULT_JIAP_PORT
    else:
        JIAP_PORT = DEFAULT_JIAP_PORT

    if args.mcp_port:
        MCP_SERVER_PORT = args.mcp_port
    else:
        env_mcp_port = os.getenv("MCP_PORT")
        if env_mcp_port:
            try:
                MCP_SERVER_PORT = int(env_mcp_port)
            except ValueError:
                MCP_SERVER_PORT = JIAP_PORT + 1
        else:
            MCP_SERVER_PORT = JIAP_PORT + 1

    if args.jiap_url:
        JIAP_BASE_URL = args.jiap_url
    elif os.getenv("JIAP_URL"):
        JIAP_BASE_URL = os.getenv("JIAP_URL")
    else:
        JIAP_HOST = args.jiap_host or os.getenv("JIAP_HOST") or DEFAULT_JIAP_HOST
        JIAP_BASE_URL = f"http://{JIAP_HOST}:{JIAP_PORT}"

    if JIAP_BASE_URL and not (
        JIAP_BASE_URL.startswith("http://") or JIAP_BASE_URL.startswith("https://")
    ):
        JIAP_BASE_URL = f"http://{JIAP_BASE_URL}"

    import logging

    logging.getLogger("fastmcp").setLevel(logging.WARNING)

    mcp.run(transport="http", host="0.0.0.0", port=MCP_SERVER_PORT, show_banner=False)
