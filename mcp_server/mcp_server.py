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
from collections import OrderedDict
from typing import List, Optional, Dict, Any, Union
from mcp.server.fastmcp import FastMCP

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S'
)
logger = logging.getLogger(__name__)

# JIAP server base address (fixed port 25419)
JIAP_BASE_SERVER = "http://127.0.0.1:25419"

# Initialize the MCP server with a descriptive name
mcp = FastMCP("JIAP MCP Server")

class Cache:

    def __init__(self, data: Dict[str, Any]):
        self._data: Optional[Union[List[Any], List[str]]] = []
        self._data_type: Optional[str] = None
        self.total_size: int = 0
        self._is_cleared: bool = False

        if not isinstance(data, dict) or 'type' not in data:
            raise ValueError("Response data must be a dictionary with 'type' field")

        response_type = data['type']

        if response_type == 'code':
            # Extract code from response structure
            if 'code' in data:
                self._data_type = 'string'
                code_content = data['code']
                self._data = code_content.splitlines(True) if isinstance(code_content, str) else [str(code_content)]
                self.total_size = len(self._data)
                self._current_offset = 0
                logger.info(f"Code cache (type: code), size: {self.total_size}")
            else:
                raise ValueError("Code type response missing 'code' field")

        elif response_type == 'list':
            # Extract list data from response structure by finding fields ending with '-list'
            list_data = None
            list_field_name = None

            for field_name, field_value in data.items():
                if field_name.endswith('-list') and isinstance(field_value, list):
                    list_data = field_value
                    list_field_name = field_name
                    break

            if list_data is not None:
                self._data_type = 'list'
                self._data = list_data
                self.total_size = len(list_data)
                self._current_offset = 0
                logger.info(f"List cache (type: list, field: {list_field_name}), size: {self.total_size}")
            else:
                raise ValueError("List type response missing fields ending with '-list'")

        else:
            raise TypeError(f"Unsupported response type: {response_type}")

    def get_slice(
        self, 
        slice_number: Optional[int] = None,
        list_slice_size: int = 100, 
        code_max_chars: int = 60000
    ) -> Union[List[Any], str, None]:
        
        if self._is_cleared:
            logger.warning("Cache has been cleared")
            return None

        if slice_number is not None:
            if slice_number < 1:
                raise ValueError("Slice number error")
            if self._data_type == 'list':
                start = (slice_number - 1) * list_slice_size
                if start >= self.total_size: return []
                return self._data[start : start + list_slice_size]
            else: # string
                temp_offset, target_chunk = 0, ""
                for _ in range(slice_number):
                    if temp_offset >= self.total_size: target_chunk = ""; break
                    lines, chars = [], 0
                    while temp_offset < self.total_size:
                        line = self._data[temp_offset]
                        if (chars + len(line) > code_max_chars) and lines: break
                        lines.append(line); chars += len(line); temp_offset += 1
                    target_chunk = "".join(lines)
                return target_chunk
        else:
            if self.is_finished(): return None
            if self._data_type == 'list':
                chunk = self._data[self._current_offset : self._current_offset + list_slice_size]
                self._current_offset += list_slice_size
                return chunk
            else: # string
                lines, chars = [], 0
                while self._current_offset < self.total_size:
                    line = self._data[self._current_offset]
                    if (chars + len(line) > code_max_chars) and lines: break
                    lines.append(line); chars += len(line); self._current_offset += 1
                return "".join(lines)
    
    def is_finished(self) -> bool:
        if self._is_cleared:
            return True
        return self._current_offset >= self.total_size

    def clear(self):
        if not self._is_cleared:
            self._data = []
            self._is_cleared = True
            logger.info(f"Cache Clear")
    
    def __enter__(self):
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.clear()


class CacheManager:

    _instance = None

    def __new__(cls, *args, **kwargs):
        if cls._instance is None:
            cls._instance = super(CacheManager, cls).__new__(cls)
        return cls._instance

    def __init__(self, max_size: int = 5):
        if not hasattr(self, '_initialized'):
            self._caches: OrderedDict[str, 'Cache'] = OrderedDict()
            self.max_size = max_size
            self._initialized = True
            logger.debug(f"LRU CacheManager initialized with max size: {self.max_size}")

    def _generate_key(self, endpoint: str, json_data: Optional[Dict[str, Any]]) -> str:
        data_str = json.dumps(json_data, sort_keys=True) if json_data else ""
        return f"{endpoint}::{data_str}"

    def get_cache(self, key: str) -> Optional['Cache']:
        if key not in self._caches:
            return None
        self._caches.move_to_end(key)
        logger.debug(f"Cache hit for key '{key}'. Marked as most recently used.")
        return self._caches[key]

    def set_cache(self, key: str, cache_instance: 'Cache'):
        self._caches[key] = cache_instance
        
        if len(self._caches) > self.max_size:
            oldest_key, oldest_cache = self._caches.popitem(last=False)
            logger.debug(f"Cache full. Evicting least recently used item with key: '{oldest_key}'")
            oldest_cache.clear()

    def clear(self, key: str):
        if key in self._caches:
            self._caches[key].clear()
            del self._caches[key]
            logger.debug(f"Cache for key '{key}' has been explicitly cleared.")

cache_manager = CacheManager(max_size=5)

async def request_to_jiap(
    endpoint: str,
    json_data: Optional[Dict[str, Any]] = None,
    slice_number: int = 1,
    api_type: str = "jiap",
    list_slice_size: int = 100,
    code_max_chars: int = 60000,
    force_refresh: bool = False,
    auto_cleanup_threshold: int = 10
) -> Union[List[Any], str, None]:

    cache_key = cache_manager._generate_key(endpoint=endpoint, json_data=json_data)

    if force_refresh:
        cache_manager.clear(cache_key)

    cache = cache_manager.get_cache(cache_key)

    if cache is None:
        try:
            url = f"{JIAP_BASE_SERVER}/api/{api_type}/{endpoint.lstrip('/')}"
            resp = requests.post(url, json=json_data or {}, timeout=60)
            resp.raise_for_status()
            json_response = resp.json()

            if isinstance(json_response, dict) and "error" in json_response:
                logger.error(f"JIAP API error: {json_response.get('error', 'Unknown error')}")
                return None

            data = json_response.get('data', json_response)

            with Cache(data) as cache_instance:
                cache_manager.set_cache(cache_key, cache_instance)

                if len(cache_manager._caches) > auto_cleanup_threshold:
                    oldest_keys = list(cache_manager._caches.keys())[:3]
                    for old_key in oldest_keys:
                        cache_manager.clear(old_key)

                return cache_instance.get_slice(
                    slice_number=slice_number,
                    list_slice_size=list_slice_size,
                    code_max_chars=code_max_chars
                )

        except Exception as e:
            logger.error(f"Request failed for key '{cache_key}': {e}")
            return None

    return cache.get_slice(
        slice_number=slice_number,
        list_slice_size=list_slice_size,
        code_max_chars=code_max_chars
    )

# Basic JADX endpoints

@mcp.tool(name="get_all_classes", description="Retrieves all available classes.")
async def get_all_classes(page: int = 1) -> Union[List[Any], str, None]:
    return await request_to_jiap("get_all_classes", slice_number=page)

@mcp.tool(name="get_class_source", description="Retrieves the source code of a specific class (eg: com.example.Myclass$Innerclass) in Smali or Java format")
async def get_class_source(class_name: str, smali: bool = False, page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves source code of a specific class with pagination support"""
    return await request_to_jiap("get_class_source", json_data={"class": class_name, "smali": smali}, slice_number=page)

@mcp.tool(name="get_method_source", description="Retrieves the source code of a specific method in Java or Smali format. Provide method_name as 'className.methodName(paramType):returnType', eg: com.example.Myclass$Innerclass.myMethod(java.lang.String):java.lang.String.")
async def get_method_source(method_name: str, smali: bool = False, page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves source code of a specific method with pagination support"""
    return await request_to_jiap("get_method_source", json_data={"method": method_name, "smali": smali}, slice_number=page)

@mcp.tool(name="list_methods", description="Lists all methods within a specific class (eg: com.example.Myclass$Innerclass).")
async def list_methods(class_name: str, page: int = 1) -> Union[List[Any], str, None]:
    """Lists all methods within a specific class with pagination support"""
    return await request_to_jiap("list_methods", json_data={"class": class_name}, slice_number=page)

@mcp.tool(name="search_class", description="Searches for a specific class by its full name in the decompiled project, eg: com.example.myclass.")
async def search_class(class_name: str, page: int = 1) -> Union[List[Any], str, None]:
    """Searches for a specific class by its full name using page-based pagination"""
    return await request_to_jiap("search_class", json_data={"class": class_name}, slice_number=page)

@mcp.tool(name="get_method_xref", description="Retrieves cross-references (usage locations) for a specific method. Provide method_name as 'className.methodName(paramType):returnType', eg: com.example.Myclass$Innerclass.myMethod(java.lang.String):java.lang.String.")
async def get_method_xref(method_name: str, page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves cross-references for a specific method with pagination support"""
    return await request_to_jiap("get_method_xref", json_data={"method": method_name}, slice_number=page)

@mcp.tool(name="get_class_xref", description="Retrieves cross-references (usage locations) for a specific class.")
async def get_class_xref(class_name: str, page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves cross-references for a specific class with pagination support"""
    return await request_to_jiap("get_class_xref", json_data={"class": class_name}, slice_number=page)

@mcp.tool(name="get_implement", description="Retrieves implementing classes for a specific interface, eg: com.example.IInterface.")
async def get_implement(interface_name: str, page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves implementing classes for a specific interface with pagination support"""
    return await request_to_jiap("get_implement", json_data={"interface": interface_name}, slice_number=page)

@mcp.tool(name="get_sub_classes", description="Retrieves subclasses for a specific superclass.")
async def get_sub_classes(class_name: str, page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves subclasses for a specific superclass with pagination support"""
    return await request_to_jiap("get_sub_classes", json_data={"class": class_name}, slice_number=page)

# UI Integration

@mcp.tool(name="selected_text", description="Retrieves the currently selected text in the JADX GUI.")
async def selected_text(page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves the currently selected text in the JADX GUI"""
    return await request_to_jiap("selected_text", slice_number=page)

# Android App specific endpoints

@mcp.tool(name="get_app_manifest", description="Retrieves the Android application manifest (AndroidManifest.xml) content.")
async def get_app_manifest(page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves the Android application manifest content"""
    return await request_to_jiap("get_app_manifest", slice_number=page)

@mcp.tool(name="get_main_activity", description="Retrieves the main activity of the Android application.")
async def get_main_activity(page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves the main activity of the Android application"""
    return await request_to_jiap("get_main_activity", slice_number=page)

# Android Framework specific endpoints

@mcp.tool(name="get_system_service_impl", description="Retrieves system service implementation details of an interface, eg: android.os.IMyService.")
async def get_system_service_impl(interface_name: str, page: int = 1) -> Union[List[Any], str, None]:
    """Retrieves system service implementation details with pagination support"""
    return await request_to_jiap("get_system_service_impl", json_data={"interface": interface_name}, slice_number=page)


# Health check

@mcp.tool(name="health_check", description="Checks if the JIAP server is running and returns its status.")
async def health_check() -> Union[Dict[str, Any], str, None]:
    """Checks if the JIAP server is running and returns its status"""
    try:
        url = f"{JIAP_BASE_SERVER}/health"
        resp = requests.get(url, timeout=10)
        resp.raise_for_status()

        json_response = resp.json()
        if isinstance(json_response, dict) and 'status' in json_response:
            return json_response

        logger.error(f"Unexpected health response format: {json_response}")
        return None

    except Exception as e:
        logger.error(f"Health check failed: {e}")
        return {"status": "Error", "url": "N/A", "error": str(e)}

if __name__ == "__main__":
    logger.info("JIAP MCP Server")
    mcp.run(transport="stdio")