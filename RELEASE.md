# 🚀 JIAP v0.2.3 Release Notes

## 🐛 Bug Fixes
- **Fixed Runtime Parameter Name Resolution**: Resolved issue where parameter names were not preserved at runtime, causing "Parameter not found" errors
- **Enhanced Parameter Mapping**: Updated parameter mapping logic to use configuration-defined parameter names instead of relying on reflection
- **Improved Error Messages**: Added clearer error messages indicating which parameter is missing in request payload

---

# 🚀 JIAP v0.2.2 Release Notes

## ⚡ Improvements
- **Fixed Parameter Mapping Issue**: Resolved parameter confusion when methods have multiple parameters of the same type
- **Enhanced Parameter Validation**: Added proper parameter name matching between configuration and service methods
- **Improved Null Safety**: Enhanced null pointer exception handling in search functions
- **Better Error Handling**: Added input validation for blank/null parameters

---

# 🚀 JIAP v0.2.1 Release Notes

## ✨ Features
- **New Class Keyword Search**: Added `search_class_key` functionality to search for classes whose source code contains specified keywords (case-insensitive)
- **Enhanced Code Analysis**: Improved search capabilities for more efficient code navigation and discovery

## ⚡ Improvements
- **Fixed Error Message Format**: Corrected error message naming conventions in method handlers
- **Updated Documentation**: Refreshed all README files with accurate tool descriptions

---

# 🚀 JIAP v0.2.0 Release Notes

## ✨ Features
- **Plugin Architecture Refactor**: Introduced JiapConfig with centralized service management and routing
- **Enhanced Cross-Reference Analysis**: Improved method/class cross-references with better line number tracking
- **UI Integration**: Added selected text/class endpoints for real-time JADX GUI interaction
- **Android System Service Analysis**: Enhanced system service implementation discovery

## ⚡ Improvements
- **Server-Side Caching**: Moved caching logic from client to core server for better performance
- **Simplified MCP Server**: Removed client-side slicing and complex cache management
- **Enhanced Error Handling**: Better exception management with structured error responses
- **Code Quality**: Cleaned up duplicated code, improved type safety and service organization

---

**Breaking Changes (v0.2.0)**: Removed client-side caching, simplified MCP response format
**Migration**: Update MCP clients for new response structure