# 🚀 JIAP v0.3.2 Release Notes

## ✨ New Features

### Advanced Android Analysis Tools

#### Code Analysis Enhancements
- **Field Cross-Reference Analysis** (`get_field_xref`): Find field usage locations across the codebase
- **Enhanced Method Tracking**: Extended method xref capabilities for better code flow analysis

#### Android Security Analysis
- **Exported Components Detection** (`get_exported_components`): Identify exported activities, services, receivers, and providers with their permissions
- **Deep Link Discovery** (`get_deep_links`): Extract URL schemes and intent filters for attack surface analysis
- **Dynamic Broadcast Receivers** (`get_dynamic_receivers`): Find dynamically registered BroadcastReceivers for vulnerability assessment

#### Architecture Improvements
- **UI Service Extraction**: Separated UI-related functionality into dedicated `UIService`
- **Vulnerability Mining Service**: New `VulnMiningService` for security-specific analysis features
- **Modular Service Architecture**: Refactored service layer for better maintainability and extensibility

---

# 🚀 JIAP v0.3.1 Release Notes

## ✨ New Features

### Built-in Security Analysis Skills
- **JIAP Analyst Skill**: Main entry point for Android reverse engineering and security analysis

---

## 🚀 JIAP v0.3.0 Release Notes

## ✨ Major Features

### Automatic MCP Companion Process Management
- **Automatic Startup**: MCP server now launches automatically as a companion process when JIAP plugin starts
- **Sidecar Architecture**: Built-in sidecar process manager handles MCP server lifecycle
- **Zero Configuration**: No manual MCP server startup or dependency installation required
- **Auto-Extraction**: MCP scripts automatically extracted to `~/.jiap/mcp/` on first launch
- **Health Monitoring**: Continuous monitoring of companion process with auto-restart capability

### Structured Error System
- **Error Codes**: Added 19 structured error codes (E001-E019) for clear diagnostics
- **Error Messages**: Human-readable error messages with context-specific details
- **Consistent Format**: Standardized JSON error response format across all endpoints
- **Enhanced Logging**: Improved error logging with code-based categorization

### Enhanced User Experience
- **GUI Integration**: New JIAP Server Status menu with real-time monitoring
- **Live Configuration**: Port and script path changes via GUI with automatic restart
- **Visual Feedback**: Status indicators for both JIAP server and MCP companion process
- **Health Check**: Built-in connection verification tool

## ⚡ Improvements

### Architecture & Performance
- **Companion Process Lifecycle**: Managed process lifecycle synchronized with plugin state
- **Resource Management**: Proper cleanup of companion processes on plugin unload
- **Port Flexibility**: Dynamic port assignment with automatic MCP port calculation (JIAP_PORT + 1)
- **Dependency Detection**: Automatic Python/uv detection and dependency validation

### Developer Experience
- **Error Handling**: Comprehensive error coverage with actionable error messages
- **Logging**: Enhanced logging with prefix-based categorization for companion process output
- **Configuration**: Simplified configuration via plugin options and GUI

## 🐛 Bug Fixes

- **Process Management**: Fixed companion process not stopping on plugin unload
- **Port Conflicts**: Better handling of port allocation and conflict detection
- **Script Extraction**: Fixed resource extraction path issues on different platforms
- **Health Check**: Improved health check reliability and timeout handling

## 📋 Technical Details

### Companion Process Flow
```
JADX Startup → JIAP Plugin Init → HTTP Server Start → Script Extraction → 
Companion Launch → Health Monitoring → Ready for MCP Connections
```

### Error Code Categories
- **E001-E005**: Server lifecycle errors
- **E006-E007**: JADX integration errors
- **E008-E012**: Companion process errors
- **E013-E019**: API and connection errors

## 🔄 Migration Notes

**Breaking Changes**: None - fully backward compatible

**Recommended Actions**:
1. Remove manual MCP server startup scripts
2. Update MCP client configurations to use auto-managed ports
3. Review error handling to use new error codes

---

## Previous Releases

See v0.2.x release notes for historical changes.
