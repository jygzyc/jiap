# JIAP v1.1.0

## Performance Improvements

- **JADX Disk Cache**: Enable native JADX code caching to `~/.jiap/cache/code/` for 10-50x faster repeated queries
- **Optimized Warmup**: Filter SDK packages and randomly sample app code during warmup, with full decompilation fallback for small apps
- **CodeInfo Caching**: Cache decompiled code info to reduce redundant decompilation

## Project Structure

- **Directory Restructure**: Rename `jiap_core/` to `jiap/` and move `mcp_server/` into `jiap/` directory for cleaner organization
- **Gradle Build**: Update build configuration for the new directory structure

## Documentation Enhancements

- **Refactored Audit Guides**: Reorganize security analysis documentation by component type (Activity, Service, ContentProvider)
- **Vulnerability Categorization**: Separate App vulnerabilities (IPC, WebView, SQL injection) from Framework vulnerabilities (Permission bypass, Identity confusion)
- **Standardized Tool APIs**: Update all tool signatures to use complete method signatures with named parameters
- **Taint Tracking Methodology**: Add comprehensive taint analysis workflow documentation

## Agent Skill Updates

**jiap-analyst**: 
- Component-specific analysis workflows for Activity, Service, and ContentProvider
- Framework-level vulnerability detection (Permission bypass, UID spoofing)
- Standardized tool API signatures matching MCP protocol

---

# JIAP v1.0.0

**JIAP (Java Intelligence Analysis Platform)** is a JADX plugin that brings AI-powered code analysis to your Android reverse engineering workflow through MCP (Model Context Protocol).

## Purpose

JIAP bridges JADX decompiler with AI assistants, enabling intelligent code analysis, security auditing, and vulnerability discovery in Android applications.

## Key Features

### Code Analysis
- **Class Exploration**: Browse all classes, search by keyword, retrieve source code and detailed class information
- **Method Tracking**: Search methods, extract source code, analyze cross-references and call graphs
- **Field Analysis**: Track field definitions and locate field usage across the codebase
- **Inheritance**: Find implementations and subclasses for interface/class hierarchy analysis

### Android Security
- **Manifest Analysis**: Parse AndroidManifest.xml for application structure
- **Component Audit**: Identify exported activities, services, and broadcast receivers
- **Deep Link Discovery**: Extract URL schemes and intent filters for attack surface mapping
- **Dynamic Receivers**: Find runtime-registered BroadcastReceivers for vulnerability assessment

### UI Integration
- **Context-Aware**: Access currently selected text or class in JADX
- **Seamless Workflow**: Analyze code directly from the IDE

### MCP Server
- **Auto-Managed**: Built-in companion process starts automatically on port 25420
- **Zero Configuration**: No manual setup required
- **Health Monitoring**: Continuous process health checks with auto-restart

## Agent Skill

**jiap-analyst**: Provides intelligent Android reverse engineering and security analysis capabilities for AI assistants.

---
*First stable release. Ready for production use.*
