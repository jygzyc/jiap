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

import json
import requests
import os
import argparse
import time
from collections import OrderedDict
from pydantic import Field
from typing import Optional, Dict, Any
from fastmcp import FastMCP
from fastmcp.tools.tool import ToolResult

# Default JIAP server URL, can be overridden via environment variable or command line argument
JIAP_PORT = 25419
DEFAULT_JIAP_BASE_SERVER_IP = "http://127.0.0.1"
DEFAULT_JIAP_BASE_SERVER = f"{DEFAULT_JIAP_BASE_SERVER_IP}:{JIAP_PORT}"
JIAP_BASE_SERVER = os.getenv("JIAP_BASE_SERVER", DEFAULT_JIAP_BASE_SERVER)
mcp = FastMCP("JIAP MCP Server")

# LRU cache with TTL implementation
JIAP_CACHE_MAX_SIZE = int(os.getenv("JIAP_CACHE_MAX_SIZE", "5"))
JIAP_CACHE_TTL = int(os.getenv("JIAP_CACHE_TTL", "300"))  # seconds


class LRUCacheTTL:
    def __init__(self, max_size: int = 5, ttl: Optional[int] = 300):
        self.max_size = max_size
        self.ttl = ttl
        self._data: "OrderedDict[str, tuple]" = OrderedDict()

    def get(self, key: str):
        item = self._data.get(key)
        if not item:
            return None
        value, expiry = item
        if expiry is not None and time.time() > expiry:
            try:
                del self._data[key]
            except KeyError:
                pass
            return None
        # mark as recently used
        try:
            self._data.move_to_end(key)
        except Exception:
            pass
        return value

    def set(self, key: str, value: Any):
        expiry = (time.time() + self.ttl) if self.ttl is not None else None
        self._data[key] = (value, expiry)
        try:
            self._data.move_to_end(key)
        except Exception:
            pass
        # evict oldest if over capacity
        while len(self._data) > self.max_size:
            self._data.popitem(last=False)


_cache = LRUCacheTTL(JIAP_CACHE_MAX_SIZE, JIAP_CACHE_TTL)

def _get_cache_key(endpoint: str, json_data: Optional[Dict[str, Any]], slice_number: int = 1) -> str:
    data_str = json.dumps(json_data or {}, sort_keys=True)
    return f"{endpoint}::{data_str}::page_{slice_number}"

def _get_slice(data: Any, slice_number: int, list_slice_size: int = 1000, code_max_chars: int = 60000) -> Any:
    if not isinstance(data, dict):
        return data

    # Create a copy of the original data
    result = data.copy()

    # Add page field
    result['page'] = slice_number

    # Handle code slicing
    if 'code' in data:
        text = str(data['code'])
        lines = text.split('\n')
        start_line = (slice_number - 1) * 1000
        end_line = start_line + 1000
        if start_line >= len(lines):
            result['code'] = ""
        else:
            selected_lines = lines[start_line:end_line]
            sliced_code = '\n'.join(selected_lines)
            if len(sliced_code) > code_max_chars:
                truncated_lines = []
                char_count = 0
                for line in selected_lines:
                    if char_count + len(line) + 1 <= code_max_chars:
                        truncated_lines.append(line)
                        char_count += len(line) + 1
                    else:
                        break
                sliced_code = '\n'.join(truncated_lines)
            result['code'] = sliced_code

    # Handle list slicing for any field ending with '-list'
    for key, value in data.items():
        if key.endswith('-list') and isinstance(value, list):
            start = (slice_number - 1) * list_slice_size
            result[key] = value[start:start + list_slice_size]

    return result

async def request_to_jiap(
    endpoint: str,
    json_data: Optional[Dict[str, Any]] = None,
    slice_number: int = 1,
    api_type: str = "jiap",
    list_slice_size: int = 1000,
    code_max_chars: int = 60000,
    force_refresh: bool = False,
) -> ToolResult:

    cache_key = _get_cache_key(endpoint=endpoint, json_data=json_data, slice_number=slice_number)

    if not force_refresh:
        cached = _cache.get(cache_key)
        if cached is not None:
            return ToolResult(cached)
    try:
        url = f"{JIAP_BASE_SERVER}/api/{api_type}/{endpoint.lstrip('/')}"
        resp = requests.post(url, json=json_data or {}, timeout=120)
        resp.raise_for_status()
        json_response = resp.json()

        if isinstance(json_response, dict) and "error" in json_response:
            return ToolResult({"error": json_response["error"], "endpoint": endpoint})

        response_data = json_response.get('data', json_response)
        sliced_data = _get_slice(response_data, slice_number, list_slice_size, code_max_chars)
        _cache.set(cache_key, sliced_data)
        return ToolResult(sliced_data)

    except Exception as e:
        return ToolResult({"error": str(e), "endpoint": endpoint})

#######################################################
# MCP Tool Definitions
#######################################################

# Basic JADX endpoints
@mcp.tool(
    name="get_all_classes",
    description="Retrieves all available classes in the decompiled project. Supports pagination via the page parameter (default: 1)."
)
async def get_all_classes(
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_all_classes", slice_number=page)


@mcp.tool(
    name="get_class_source",
    description="Retrieves the source code of a specific class (e.g., com.example.Myclass$Innerclass) in Smali or Java format. Supports pagination via the page parameter (default: 1)."
)
async def get_class_source(
    class_name: str = Field(description="Full name of the class, e.g., com.example.Myclass$Innerclass"),
    smali: bool = Field(False, description="Whether to retrieve the source in Smali format (True) or Java format (False), default is False"),
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_class_source", json_data={"class": class_name, "smali": smali}, slice_number=page)


@mcp.tool(
    name="search_method",
    description="Searches for methods matching the given method_name string, e.g., doSomething matches com.example.Myservice.doSomething(java.lang.String, int):int. Supports pagination via the page parameter (default: 1)."
)
async def search_method(
    method_name: str = Field(description="Method name or partial name to search for"),
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("search_method", json_data={"method": method_name}, slice_number=page)


@mcp.tool(
    name="get_method_source",
    description="Retrieves the source code of a specific method in Java or Smali format. Provide method_name as 'className.methodName(paramType):returnType', e.g., com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String. Supports pagination via the page parameter (default: 1)."
)
async def get_method_source(
    method_name: str = Field(description="Full method signature, e.g., com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String"),
    smali: bool = Field(False, description="Whether to retrieve the source in Smali format (True) or Java format (False), default is False"),
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_method_source", json_data={"method": method_name, "smali": smali}, slice_number=page)


@mcp.tool(
    name="get_class_info",
    description="Get a specific class information, such as fields and methods, by its full name in the decompiled project, e.g., com.example.myclass. Supports pagination via the page parameter (default: 1)."
)
async def get_class_info(
    class_name: str = Field(description="Full name of the class, e.g., com.example.Myclass$Innerclass"),
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_class_info", json_data={"class": class_name}, slice_number=page)


@mcp.tool(
    name="get_method_xref",
    description="Retrieves cross-references (usage locations) for a specific method. Provide method_name as 'className.methodName(paramType):returnType', e.g., com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String. Supports pagination via the page parameter (default: 1)."
)
async def get_method_xref(
    method_name: str = Field(description="Full method signature, e.g., com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String"),
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_method_xref", json_data={"method": method_name}, slice_number=page)


@mcp.tool(
    name="get_class_xref",
    description="Retrieves cross-references (usage locations) for a specific class. Provide class_name as 'com.example.Myclass$Innerclass', e.g., com.example.Myclass$Innerclass. Supports pagination via the page parameter (default: 1)."
)
async def get_class_xref(
    class_name: str = Field(description="Full name of the class, e.g., com.example.Myclass$Innerclass"),
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_class_xref", json_data={"class": class_name}, slice_number=page)


@mcp.tool(
    name="get_implement",
    description="Retrieves implementing classes for a specific interface, e.g., com.example.IInterface. Supports pagination via the page parameter (default: 1)."
)
async def get_implement(
    interface_name: str = Field(description="Full name of the interface, e.g., com.example.IInterface"),
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_implement", json_data={"interface": interface_name}, slice_number=page)


@mcp.tool(
    name="get_sub_classes",
    description="Retrieves subclasses for a specific superclass, e.g., com.example.MySuperClass. Supports pagination via the page parameter (default: 1)."
)
async def get_sub_classes(
    class_name: str = Field(description="Full name of the superclass, e.g., com.example.MySuperClass"),
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_sub_classes", json_data={"class": class_name}, slice_number=page)

# UI Integration

@mcp.tool(
    name="selected_text",
    description="Retrieves the currently selected text in the JADX GUI. Supports pagination via the page parameter (default: 1)."
)
async def selected_text(
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("selected_text", slice_number=page)


# Android App specific endpoints
@mcp.tool(
    name="get_app_manifest",
    description="Retrieves the Android application manifest (AndroidManifest.xml) content. Supports pagination via the page parameter (default: 1)."
)
async def get_app_manifest(
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_app_manifest", slice_number=page)


@mcp.tool(
    name="get_main_activity",
    description="Retrieves the main activity of the Android application. Supports pagination via the page parameter (default: 1)."
)
async def get_main_activity(
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_main_activity", slice_number=page)


# Android Framework specific endpoints
@mcp.tool(
    name="get_system_service_impl",
    description="Retrieves system service implementation details of an interface, e.g., android.os.IMyService. Supports pagination via the page parameter (default: 1)."
)
async def get_system_service_impl(
    interface_name: str = Field(description="Full name of the system service interface, e.g., android.os.IMyService"),
    page: int = Field(1, description="Page number for pagination, default is 1")
) -> ToolResult:
    return await request_to_jiap("get_system_service_impl", json_data={"interface": interface_name}, slice_number=page)

# Health check
@mcp.tool(
    name="health_check",
    description="Checks if the JIAP server is running and returns its status."
)
async def health_check() -> ToolResult:
    """Checks if the JIAP server is running and returns its status."""
    try:
        resp = requests.get(f"{JIAP_BASE_SERVER}/health", timeout=10)
        resp.raise_for_status()
        return ToolResult(resp.json())
    except Exception as e:
        return ToolResult({"status": "Error", "url": JIAP_BASE_SERVER, "error": str(e)})

def parse_arguments():
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="JIAP MCP Server - Model Context Protocol Server for Java Intelligence Analysis Platform"
    )
    parser.add_argument(
        "--server",
        type=str,
        default=DEFAULT_JIAP_BASE_SERVER_IP,
        help=f"JIAP server IP (default: {DEFAULT_JIAP_BASE_SERVER_IP}, can also be set via JIAP_BASE_SERVER env var)"
    )
    return parser.parse_args()

if __name__ == "__main__":
    args = parse_arguments()

    # Override JIAP_BASE_SERVER if command line argument is provided
    if args.server != DEFAULT_JIAP_BASE_SERVER_IP:
        JIAP_BASE_SERVER = f"{args.server}:{JIAP_PORT}"

    mcp.run(transport="http", host="0.0.0.0", port=25420)

