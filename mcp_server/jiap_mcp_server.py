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
import logging
from typing import Optional, Dict, Any
from fastmcp import FastMCP, Context
from fastmcp.tools.tool import ToolResult

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S'
)
logger = logging.getLogger(__name__)

JIAP_BASE_SERVER = "http://127.0.0.1:25419"
mcp = FastMCP("JIAP MCP Server")

_global_cache = {}
MAX_CACHE_SIZE = 5  # Maximum number of cached items

def _get_cache_key(endpoint: str, json_data: Optional[Dict[str, Any]]) -> str:
    data_str = json.dumps(json_data or {}, sort_keys=True)
    return f"{endpoint}::{data_str}"

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

    cache_key = _get_cache_key(endpoint=endpoint, json_data=json_data)

    if not force_refresh and cache_key in _global_cache:
        cached_data = _global_cache[cache_key]
        return ToolResult(_get_slice(cached_data, slice_number, list_slice_size, code_max_chars))
    try:
        url = f"{JIAP_BASE_SERVER}/api/{api_type}/{endpoint.lstrip('/')}"
        resp = requests.post(url, json=json_data or {}, timeout=120)
        resp.raise_for_status()
        json_response = resp.json()

        if isinstance(json_response, dict) and "error" in json_response:
            return ToolResult({"error": json_response["error"], "endpoint": endpoint})

        response_data = json_response.get('data', json_response)
        _global_cache[cache_key] = response_data
        if len(_global_cache) > MAX_CACHE_SIZE:
            oldest_key = next(iter(_global_cache))
            del _global_cache[oldest_key]
            logger.debug(f"Cache full, removed oldest item: {oldest_key}")
        return ToolResult(_get_slice(response_data, slice_number, list_slice_size, code_max_chars))

    except Exception as e:
        logger.error(f"Request failed: {e}")
        return ToolResult({"error": str(e), "endpoint": endpoint})

#######################################################
# MCP Tool Definitions
#######################################################

# Basic JADX endpoints
@mcp.tool(
    name="get_all_classes",
    description="Retrieves all available classes in the decompiled project. Supports pagination via the page parameter (default: 1)."
)
async def get_all_classes(ctx: Context, page: int = 1) -> ToolResult:
    await ctx.info("Fetching all classes from JIAP Plugin...")
    return await request_to_jiap("get_all_classes", slice_number=page)

@mcp.tool(
    name="get_class_source",
    description="Retrieves the source code of a specific class (eg: com.example.Myclass$Innerclass) in Smali or Java format. Supports pagination via the page parameter (default: 1)."
)
async def get_class_source(ctx: Context, class_name: str, smali: bool = False, page: int = 1) -> ToolResult:
    await ctx.info(f"Fetching source for class {class_name} from JIAP Plugin...")
    return await request_to_jiap("get_class_source", json_data={"class": class_name, "smali": smali}, slice_number=page)

@mcp.tool(
    name="get_method_source",
    description="Retrieves the source code of a specific method in Java or Smali format. Provide method_name as 'className.methodName(paramType):returnType', eg: com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String. Supports pagination via the page parameter (default: 1)."
)
async def get_method_source(ctx: Context,method_name: str, smali: bool = False, page: int = 1) -> ToolResult:
    await ctx.info(f"Fetching source for method {method_name} from JIAP Plugin...")
    return await request_to_jiap("get_method_source", json_data={"method": method_name, "smali": smali}, slice_number=page)

@mcp.tool(
    name="get_class_info",
    description="Get a specific class information, such as fields and methods, by its full name in the decompiled project, eg: com.example.myclass. Supports pagination via the page parameter (default: 1)."
)
async def get_class_info(ctx: Context, class_name: str, page: int = 1) -> ToolResult:
    await ctx.info(f"Getting for class {class_name} in JIAP Plugin...")
    return await request_to_jiap("get_class_info", json_data={"class": class_name}, slice_number=page)

@mcp.tool(
    name="get_method_xref",
    description="Retrieves cross-references (usage locations) for a specific method. Provide method_name as 'className.methodName(paramType):returnType', eg: com.example.Myclass$Innerclass.myMethod(java.lang.String, int):java.lang.String. Supports pagination via the page parameter (default: 1)."
)
async def get_method_xref(ctx: Context,method_name: str, page: int = 1) -> ToolResult:
    await ctx.info(f"Fetching method xref for {method_name} from JIAP Plugin...")
    return await request_to_jiap("get_method_xref", json_data={"method": method_name}, slice_number=page)

@mcp.tool(
    name="get_class_xref",
    description="Retrieves cross-references (usage locations) for a specific class. Provide class_name as 'com.example.Myclass$Innerclass', eg: com.example.Myclass$Innerclass. Supports pagination via the page parameter (default: 1)."
)
async def get_class_xref(ctx: Context,class_name: str, page: int = 1) -> ToolResult:
    await ctx.info(f"Fetching class xref for {class_name} from JIAP Plugin...")
    return await request_to_jiap("get_class_xref", json_data={"class": class_name}, slice_number=page)

@mcp.tool(
    name="get_implement",
    description="Retrieves implementing classes for a specific interface, eg: com.example.IInterface. Supports pagination via the page parameter (default: 1)."
)
async def get_implement(ctx: Context,interface_name: str, page: int = 1) -> ToolResult:
    await ctx.info(f"Fetching implementations for {interface_name} from JIAP Plugin...")
    return await request_to_jiap("get_implement", json_data={"interface": interface_name}, slice_number=page)

@mcp.tool(
    name="get_sub_classes",
    description="Retrieves subclasses for a specific superclass, eg: com.example.MySuperClass. Supports pagination via the page parameter (default: 1)."
)
async def get_sub_classes(ctx: Context,class_name: str, page: int = 1) -> ToolResult:
    await ctx.info(f"Fetching subclasses for {class_name} from JIAP Plugin...")
    return await request_to_jiap("get_sub_classes", json_data={"class": class_name}, slice_number=page)

# UI Integration

@mcp.tool(
    name="selected_text",
    description="Retrieves the currently selected text in the JADX GUI. Supports pagination via the page parameter (default: 1)."
)
async def selected_text(ctx: Context, page: int = 1) -> ToolResult:
    await ctx.info("Fetching selected text from JIAP Plugin...")
    return await request_to_jiap("selected_text", slice_number=page)


# Android App specific endpoints
@mcp.tool(
    name="get_app_manifest",
    description="Retrieves the Android application manifest (AndroidManifest.xml) content. Supports pagination via the page parameter (default: 1)."
)
async def get_app_manifest(ctx: Context, page: int = 1) -> ToolResult:
    await ctx.info("Fetching AndroidManifest.xml from JIAP Plugin...")
    return await request_to_jiap("get_app_manifest", slice_number=page)

@mcp.tool(
    name="get_main_activity",
    description="Retrieves the main activity of the Android application. Supports pagination via the page parameter (default: 1)."
)
async def get_main_activity(ctx: Context, page: int = 1) -> ToolResult:
    await ctx.info("Fetching main activity from JIAP Plugin...")
    return await request_to_jiap("get_main_activity", slice_number=page)


# Android Framework specific endpoints
@mcp.tool(
    name="get_system_service_impl",
    description="Retrieves system service implementation details of an interface, eg: android.os.IMyService. Supports pagination via the page parameter (default: 1)."
)
async def get_system_service_impl(ctx: Context,interface_name: str, page: int = 1) -> ToolResult:
    await ctx.info(f"Fetching system service implementation for {interface_name} from JIAP Plugin...")
    return await request_to_jiap("get_system_service_impl", json_data={"interface": interface_name}, slice_number=page)


#######################################################
# MCP Prompt Definitions
#######################################################

# Android Framework Security Analysis Prompt
@mcp.prompt(
    name="android_framework_security_audit",
    description="Perform comprehensive security analysis on Android framework layer APIs. Check for permission validation, UID/PID verification, and potential bypass vulnerabilities. Use get_system_service_impl tool to retrieve system service interface implementations."
)
async def android_framework_security_audit(ctx: Context, api_method: str) -> str:
    return f"""
    You are an Android security engineer specializing in framework layer API security analysis.
    Your task is to conduct a comprehensive security assessment of the specified Android framework API method.

    Available Tools:
    - Use get_system_service_impl to retrieve system service interface implementations (e.g., android.os.IMyService)
    - Use get_method_source to analyze specific method implementations
    - Use get_class_source to examine class-level security controls

    Analysis Requirements:
    1. Permission Validation: Check if the API properly validates caller permissions
    2. UID Verification: Verify if User ID validation is implemented and secure
    3. PID Verification: Check if Process ID validation is properly implemented
    4. Security Logic Assessment: Evaluate the reasonableness and effectiveness of security implementation
    5. Vulnerability Identification: Identify potential security bypass methods

    Output Format:
    - Security Assessment Summary
    - Permission Validation Analysis
    - UID/PID Validation Analysis
    - Security Implementation Evaluation
    - Identified Vulnerabilities
    - Proof of Concept (PoC) for any bypass methods
    - Security Recommendations

    Target API Method: {api_method}

    Provide detailed analysis including any identified vulnerabilities and proof-of-concept exploit code if applicable."""

# Health check
@mcp.tool(
    name="health_check",
    description="Checks if the JIAP server is running and returns its status."
)
async def health_check(ctx: Context) -> ToolResult:
    """Checks if the JIAP server is running and returns its status."""
    try:
        resp = requests.get(f"{JIAP_BASE_SERVER}/health", timeout=10)
        resp.raise_for_status()
        return ToolResult(resp.json())
    except Exception as e:
        await ctx.error(f"Health check failed: {e}")
        return ToolResult({"status": "Error", "url": JIAP_BASE_SERVER, "error": str(e)})

if __name__ == "__main__":
    mcp.run(transport="http", host="0.0.0.0", port=25420)

