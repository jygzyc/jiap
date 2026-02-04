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
