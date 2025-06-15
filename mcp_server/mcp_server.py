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
import sys
import httpx
import logging
from typing import List, Optional, Dict, Any
from mcp.server.fastmcp import FastMCP

# Set up logging configuration
logger = logging.getLogger()
logger.setLevel(logging.ERROR)

# Console handler for logging to the console
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.ERROR)
console_handler.setFormatter(logging.Formatter('%(asctime)s - %(levelname)s - %(message)s'))
logger.addHandler(console_handler)

# Default JADX server base address if not provided as command line argument
DEFAULT_JIAP_BASE_SERVER = "http://127.0.0.1:8080"
# Get JADX server base URI from command line or use default
jiap_base_server_uri = sys.argv[1].rstrip('/') if len(sys.argv) > 1 else DEFAULT_JIAP_BASE_SERVER

# Initialize the MCP server with a descriptive name
mcp = FastMCP("JIAP MCP Server")

async def request_to_jadx(endpoint: str, json_data: Optional[Dict[str, Any]] = None, api_type: str = "jadx", method: str = "POST") -> str:
    """
    Simplified HTTP request function for JADX server communication.
    
    Args:
        endpoint: API endpoint (without leading slash)
        json_data: Optional JSON payload
        api_type: API type ("jadx" or "file")
        method: HTTP method (GET, POST, DELETE)
        
    Returns:
        JSON response as formatted string or error message
    """
    try:
        url = f"{jiap_base_server_uri}/api/{api_type}/{endpoint.lstrip('/')}"
        
        async with httpx.AsyncClient() as client:
            if method.upper() == "GET":
                resp = await client.get(url, timeout=60)
            elif method.upper() == "DELETE":
                resp = await client.delete(url, json=json_data or {}, timeout=60)
            else:
                resp = await client.post(url, json=json_data or {}, timeout=60)
            
            resp.raise_for_status()
            json_response = resp.json()
            
            # Handle error responses
            if isinstance(json_response, dict) and not json_response.get("success", True):
                return f"Error: {json_response.get('message', 'Unknown error')}"
            
            return json.dumps(json_response, ensure_ascii=False, indent=2)
                
    except httpx.HTTPStatusError as e:
        return f"Error: HTTP {e.response.status_code}: {e.response.text}"
    except httpx.TimeoutException:
        return "Error: Request timeout - JADX server may be processing a large operation"
    except httpx.ConnectError:
        return "Error: Connection failed - JADX server may not be running"
    except Exception as e:
        return f"Error: {str(e)}"

# Backward compatibility alias
async def post_to_jadx(endpoint: str, json_data: Optional[Dict[str, Any]] = None, api_type: str = "jadx") -> str:
    """Backward compatibility wrapper for request_to_jadx with POST method"""
    return await request_to_jadx(endpoint, json_data, api_type, "POST")

# File Management and Processing Tools

@mcp.tool(name="process_remote_file", description="Downloads and processes a file from a remote URL for decompilation analysis.")
async def process_remote_file(url: str) -> str:
    return await request_to_jadx("remote_handle", {"url": url}, api_type="file")

@mcp.tool(name="list_files", description="Retrieves a list of all uploaded and initialized decompilers with their file IDs and names.")
async def list_files() -> str:
    return await request_to_jadx("list", api_type="file", method="GET")

@mcp.tool(name="delete_file", description="Deletes an uploaded file and removes its decompiler instance by file ID.")
async def delete_file(file_id: str) -> str:
    return await request_to_jadx("delete", {"id": file_id}, api_type="file", method="DELETE")

@mcp.tool(name="remove_decompiler", description="Removes a specific JADX decompiler instance and releases memory for the given fileId.")
async def remove_decompiler(file_id: str) -> str:
    return await request_to_jadx("remove_decompiler", {"id": file_id})

@mcp.tool(name="remove_all_decompilers", description="Removes all JADX decompiler instances and releases all memory.")
async def remove_all_decompilers() -> str:
    return await request_to_jadx("remove_all_decompilers")


# JADX Decompiler Core Analysis Tools

@mcp.tool(name="get_all_classes", description="Retrieves all available classes from the decompiled APK/JAR/DEX/AAR file.")
async def get_all_classes(file_id: str) -> str:
    return await request_to_jadx("get_all_classes", {"id": file_id})


@mcp.tool(name="search_class_by_name", description="Searches for a specific class by its full name in the decompiled project, eg: com.example.myclass.")
async def search_class_by_name(file_id: str, class_name: str) -> str:
    return await request_to_jadx("search_class_by_name", {"id": file_id, "class": class_name})


@mcp.tool(name="search_method_by_name", description="Searches for methods by name across all classes in the project.")
async def search_method_by_name(file_id: str, method_name: str) -> str:
    return await request_to_jadx("search_method_by_name", {"id": file_id, "method": method_name})


@mcp.tool(name="list_methods_of_class", description="Lists all methods within a specific class, eg: com.example.myclass.")
async def list_methods_of_class(file_id: str, class_name: str) -> str:
    return await request_to_jadx("list_methods_of_class", {"id": file_id, "class": class_name})


@mcp.tool(name="list_fields_of_class", description="Lists all fields within a specific class, eg: com.example.myclass.")
async def list_fields_of_class(file_id: str, class_name: str) -> str:
    return await request_to_jadx("list_fields_of_class", {"id": file_id, "class": class_name})


@mcp.tool(name="get_class_source", description="Retrieves the source code of a specific class (eg: com.example.myclass.) in Smali or Java format (true or false).")
async def get_class_source(file_id: str, class_name: str, smali: bool = False) -> str:
    return await request_to_jadx("get_class_source", {"id": file_id, "class": class_name, "smali": smali})


@mcp.tool(name="get_method_source", description="Retrieves the source code of a specific method in Java or Smali format. Provide method_info as 'className.methodName(paramType):returnType'.")
async def get_method_source(file_id: str, method_info: str, smali: bool = False) -> str:
    return await request_to_jadx("get_method_source", {"id": file_id, "method": method_info, "smali": smali})


@mcp.tool(name="get_method_xref", description="Retrieves cross-references (usage locations) for a specific method. Provide method_info as 'className.methodName(paramType):returnType'.")
async def get_method_xref(file_id: str, method_info: str) -> str:
    return await request_to_jadx("get_method_xref", {"id": file_id, "method": method_info})


@mcp.tool(name="get_class_xref", description="Retrieves cross-references (usage locations) for a specific class.")
async def get_class_xref(file_id: str, class_name: str) -> str:
    return await request_to_jadx("get_class_xref", {"id": file_id, "class": class_name})


@mcp.tool(name="get_field_xref", description="Retrieves cross-references (usage locations) for a specific field. Provide field_info as 'className.fieldName: type'.")
async def get_field_xref(file_id: str, field_info: str) -> str:
    return await request_to_jadx("get_field_xref", {"id": file_id, "field": field_info})


@mcp.tool(name="get_interface_impl", description="Retrieves implementing classes for a specific interface, eg: com.example.IInterface.")
async def get_interface_impl(file_id: str, interface_name: str) -> str:
    return await request_to_jadx("get_interface_impl", {"id": file_id, "interface": interface_name})


@mcp.tool(name="get_subclasses", description="Retrieves subclasses for a specific superclass.")
async def get_subclasses(file_id: str, class_name: str) -> str:
    return await request_to_jadx("get_subclass", {"id": file_id, "class": class_name})


# Inheritance and Interface Analysis Tools


@mcp.tool(name="get_system_service_impl", description="Retrieves system service implementation details of an interfaceDescriptor, eg: android.os.IMyService.")
async def get_system_service_impl(file_id: str, class_name: str) -> str:
    return await request_to_jadx("get_system_service_impl", {"id": file_id, "class": class_name})


@mcp.tool(name="get_app_manifest", description="Retrieves the Android application manifest (AndroidManifest.xml) content.")
async def get_app_manifest(file_id: str) -> str:
    return await request_to_jadx("get_app_manifest", {"id": file_id})


# Testing and Debugging Tools

@mcp.tool(name="test_endpoint", description="Tests the JADX decompiler service connectivity and validates that the server is working correctly.")
async def test_endpoint() -> str:
    return await request_to_jadx("test")


if __name__ == "__main__":
    logger.info("Java Intelligence Analysis Platform mcp server")
    mcp.run(transport="stdio")
