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
import hashlib
from typing import List, Optional, Dict, Any
from collections import OrderedDict
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

# Cache configuration
MAX_CACHE_SIZE = 100  # Maximum number of cached responses
LINES_PER_PAGE = 100  # Maximum lines per page

# Global cache for storing JADX responses
_response_cache: OrderedDict[str, str] = OrderedDict()

def _generate_cache_key(endpoint: str, json_data: Optional[Dict[str, Any]] = None, api_type: str = "jadx", method: str = "POST") -> str:
    """Generate a unique cache key for the request."""
    key_data = {
        "endpoint": endpoint,
        "json_data": json_data or {},
        "api_type": api_type,
        "method": method
    }
    key_string = json.dumps(key_data, sort_keys=True)
    return hashlib.md5(key_string.encode()).hexdigest()

def _paginate_response(response: str, page: int = 1) -> Dict[str, Any]:
    """Paginate a response by splitting it into lines and returning the specified page."""
    lines = response.split('\n')
    total_lines = len(lines)
    total_pages = (total_lines + LINES_PER_PAGE - 1) // LINES_PER_PAGE
    
    if page < 1:
        page = 1
    elif page > total_pages:
        page = total_pages
    
    start_idx = (page - 1) * LINES_PER_PAGE
    end_idx = min(start_idx + LINES_PER_PAGE, total_lines)
    
    paginated_lines = lines[start_idx:end_idx]
    paginated_content = '\n'.join(paginated_lines)
    
    return {
        "content": paginated_content,
        "pagination": {
            "current_page": page,
            "total_pages": total_pages,
            "total_lines": total_lines,
            "lines_per_page": LINES_PER_PAGE,
            "start_line": start_idx + 1,
            "end_line": end_idx
        }
    }

def _manage_cache_size():
    """Remove oldest entries if cache exceeds maximum size."""
    while len(_response_cache) > MAX_CACHE_SIZE:
        _response_cache.popitem(last=False)  # Remove oldest item

async def request_to_jadx(endpoint: str, json_data: Optional[Dict[str, Any]] = None, api_type: str = "jadx", method: str = "POST", page: int = 1, use_cache: bool = True) -> str:
    """
    Simplified HTTP request function for JADX server communication with caching and pagination support.
    
    Args:
        endpoint: API endpoint (without leading slash)
        json_data: Optional JSON payload
        api_type: API type ("jadx" or "file")
        method: HTTP method (GET, POST, DELETE)
        page: Page number for pagination (default: 1)
        use_cache: Whether to use caching (default: True)
        
    Returns:
        JSON response as formatted string or error message, with pagination info if applicable
    """
    # Generate cache key
    cache_key = _generate_cache_key(endpoint, json_data, api_type, method)
    
    # Check cache first (only for GET requests and when use_cache is True)
    if use_cache and method.upper() == "GET" and cache_key in _response_cache:
        cached_response = _response_cache[cache_key]
        # Move to end (LRU)
        _response_cache.move_to_end(cache_key)
        
        # Apply pagination to cached response
        paginated = _paginate_response(cached_response, page)
        return json.dumps({
            "data": json.loads(paginated["content"]) if paginated["content"].strip().startswith(('{', '[')) else paginated["content"],
            "pagination": paginated["pagination"]
        }, ensure_ascii=False, indent=2)
    
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
            
            # Convert response to string for caching and pagination
            response_str = json.dumps(json_response, ensure_ascii=False, indent=2)
            
            # Cache the response (only for GET requests)
            if use_cache and method.upper() == "GET":
                _response_cache[cache_key] = response_str
                _response_cache.move_to_end(cache_key)
                _manage_cache_size()
            
            # Apply pagination
            paginated = _paginate_response(response_str, page)
            
            # If response is small enough (fits in one page), return original format
            if paginated["pagination"]["total_pages"] == 1:
                return response_str
            
            # Return paginated response with metadata
            return json.dumps({
                "data": json.loads(paginated["content"]) if paginated["content"].strip().startswith(('{', '[')) else paginated["content"],
                "pagination": paginated["pagination"]
            }, ensure_ascii=False, indent=2)
                
    except httpx.HTTPStatusError as e:
        return f"Error: HTTP {e.response.status_code}: {e.response.text}"
    except httpx.TimeoutException:
        return "Error: Request timeout - JADX server may be processing a large operation"
    except httpx.ConnectError:
        return "Error: Connection failed - JADX server may not be running"
    except Exception as e:
        return f"Error: {str(e)}"

# Backward compatibility alias
async def post_to_jadx(endpoint: str, json_data: Optional[Dict[str, Any]] = None, api_type: str = "jadx", page: int = 1, use_cache: bool = True) -> str:
    """Backward compatibility wrapper for request_to_jadx with POST method"""
    return await request_to_jadx(endpoint, json_data, api_type, "POST", page, use_cache)

# File Management and Processing Tools

@mcp.tool(name="process_remote_file", description="Downloads and processes a file from a remote URL for decompilation analysis. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def process_remote_file(url: str, page: int = 1) -> str:
    return await request_to_jadx("remote_handle", {"url": url}, api_type="file", page=page)

@mcp.tool(name="list_files", description="Retrieves a list of all uploaded and initialized decompilers with their file IDs and names. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def list_files(page: int = 1) -> str:
    return await request_to_jadx("list", api_type="file", method="GET", page=page)

@mcp.tool(name="delete_file", description="Deletes an uploaded file and removes its decompiler instance by file ID.")
async def delete_file(file_id: str) -> str:
    return await request_to_jadx("delete", {"id": file_id}, api_type="file", method="DELETE", use_cache=False)

@mcp.tool(name="remove_decompiler", description="Removes a specific JADX decompiler instance and releases memory for the given fileId.")
async def remove_decompiler(file_id: str) -> str:
    return await request_to_jadx("remove_decompiler", {"id": file_id}, use_cache=False)

@mcp.tool(name="remove_all_decompilers", description="Removes all JADX decompiler instances and releases all memory.")
async def remove_all_decompilers() -> str:
    return await request_to_jadx("remove_all_decompilers", use_cache=False)


# JADX Decompiler Core Analysis Tools

@mcp.tool(name="get_all_classes", description="Retrieves all available classes from the decompiled APK/JAR/DEX/AAR file. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_all_classes(file_id: str, page: int = 1) -> str:
    return await request_to_jadx("get_all_classes", {"id": file_id}, page=page)


@mcp.tool(name="search_class_by_name", description="Searches for a specific class by its full name in the decompiled project, eg: com.example.myclass. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def search_class_by_name(file_id: str, class_name: str, page: int = 1) -> str:
    return await request_to_jadx("search_class_by_name", {"id": file_id, "class": class_name}, page=page)


@mcp.tool(name="search_method_by_name", description="Searches for methods by name across all classes in the project. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def search_method_by_name(file_id: str, method_name: str, page: int = 1) -> str:
    return await request_to_jadx("search_method_by_name", {"id": file_id, "method": method_name}, page=page)


@mcp.tool(name="list_methods_of_class", description="Lists all methods within a specific class, eg: com.example.myclass. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def list_methods_of_class(file_id: str, class_name: str, page: int = 1) -> str:
    return await request_to_jadx("list_methods_of_class", {"id": file_id, "class": class_name}, page=page)


@mcp.tool(name="list_fields_of_class", description="Lists all fields within a specific class, eg: com.example.myclass. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def list_fields_of_class(file_id: str, class_name: str, page: int = 1) -> str:
    return await request_to_jadx("list_fields_of_class", {"id": file_id, "class": class_name}, page=page)


@mcp.tool(name="get_class_source", description="Retrieves the source code of a specific class (eg: com.example.myclass.) in Smali or Java format (true or false). Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_class_source(file_id: str, class_name: str, smali: bool = False, page: int = 1) -> str:
    return await request_to_jadx("get_class_source", {"id": file_id, "class": class_name, "smali": smali}, page=page)


@mcp.tool(name="get_method_source", description="Retrieves the source code of a specific method in Java or Smali format. Provide method_info as 'className.methodName(paramType):returnType'. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_method_source(file_id: str, method_info: str, smali: bool = False, page: int = 1) -> str:
    return await request_to_jadx("get_method_source", {"id": file_id, "method": method_info, "smali": smali}, page=page)


@mcp.tool(name="get_method_xref", description="Retrieves cross-references (usage locations) for a specific method. Provide method_info as 'className.methodName(paramType):returnType'. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_method_xref(file_id: str, method_info: str, page: int = 1) -> str:
    return await request_to_jadx("get_method_xref", {"id": file_id, "method": method_info}, page=page)


@mcp.tool(name="get_class_xref", description="Retrieves cross-references (usage locations) for a specific class. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_class_xref(file_id: str, class_name: str, page: int = 1) -> str:
    return await request_to_jadx("get_class_xref", {"id": file_id, "class": class_name}, page=page)


@mcp.tool(name="get_field_xref", description="Retrieves cross-references (usage locations) for a specific field. Provide field_info as 'className.fieldName: type'. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_field_xref(file_id: str, field_info: str, page: int = 1) -> str:
    return await request_to_jadx("get_field_xref", {"id": file_id, "field": field_info}, page=page)


@mcp.tool(name="get_interface_impl", description="Retrieves implementing classes for a specific interface, eg: com.example.IInterface. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_interface_impl(file_id: str, interface_name: str, page: int = 1) -> str:
    return await request_to_jadx("get_interface_impl", {"id": file_id, "interface": interface_name}, page=page)


@mcp.tool(name="get_subclasses", description="Retrieves subclasses for a specific superclass. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_subclasses(file_id: str, class_name: str, page: int = 1) -> str:
    return await request_to_jadx("get_subclass", {"id": file_id, "class": class_name}, page=page)


# Inheritance and Interface Analysis Tools


@mcp.tool(name="get_system_service_impl", description="Retrieves system service implementation details of an interfaceDescriptor, eg: android.os.IMyService. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_system_service_impl(file_id: str, class_name: str, page: int = 1) -> str:
    return await request_to_jadx("get_system_service_impl", {"id": file_id, "class": class_name}, page=page)


@mcp.tool(name="get_app_manifest", description="Retrieves the Android application manifest (AndroidManifest.xml) content. Use page parameter to control pagination (default: 1, max 100 lines per page).")
async def get_app_manifest(file_id: str, page: int = 1) -> str:
    return await request_to_jadx("get_app_manifest", {"id": file_id}, page=page)


# Testing and Debugging Tools

@mcp.tool(name="test_endpoint", description="Tests the JADX decompiler service connectivity and validates that the server is working correctly.")
async def test_endpoint() -> str:
    return await request_to_jadx("test", use_cache=False)


if __name__ == "__main__":
    logger.info("Java Intelligence Analysis Platform mcp server")
    mcp.run(transport="stdio")
