package me.yvesz.server.model

import jadx.api.JavaClass
import jadx.api.JavaNode
import org.springframework.stereotype.Component
import org.slf4j.LoggerFactory
import java.util.concurrent.ConcurrentHashMap

/**
 * Data cache for JADX decompiler instances.
 * Each file ID has its own independent cache instance to support multiple concurrent decompilation processes.
 */
@Component
class JadxDataCache {
    
    private val log = LoggerFactory.getLogger(JadxDataCache::class.java)
    private val classAliasMap: MutableMap<String, JavaClass> = ConcurrentHashMap()
    private val methodToClassMap: MutableMap<String, JavaClass> = ConcurrentHashMap()
    private val xrefMap: MutableMap<String, MutableList<JavaNode>> = ConcurrentHashMap()
    /**
     * Map the system service interface to the service implement
     */
    private val systemServiceMap: MutableMap<String, JavaClass> = ConcurrentHashMap()

    /**
     * Initialize/clear all cache maps for this instance.
     */
    @Synchronized
    fun initCache() {
        classAliasMap.clear()
        methodToClassMap.clear()
        xrefMap.clear()
        systemServiceMap.clear()
        log.debug("Cache initialized for instance")
    }

    /**
     * Get the JavaClass associated with the given method information.
     * 
     * @param methodInfo The method information string
     * @return The JavaClass containing the method, or null if not found
     */
    @Synchronized
    fun getClassOfMethodInfo(methodInfo: String): JavaClass? {
        log.debug("getClassOfMethodInfo: $methodInfo")
        return methodToClassMap[methodInfo]
    }

    /**
     * Map a method information string to its containing JavaClass.
     * 
     * @param methodInfo The method information string
     * @param clazz The JavaClass containing the method
     */
    @Synchronized
    fun setMethodToClassMap(methodInfo: String, clazz: JavaClass) {
        log.debug("setMethodToClassMap: $methodInfo to ${clazz.fullName}")
        methodToClassMap[methodInfo] = clazz
    }

    /**
     * Get the JavaClass associated with the given class full name.
     * 
     * @param classFullName The full name of the class
     * @return The JavaClass, or null if not found
     */
    @Synchronized
    fun getClassOfAliasName(classFullName: String): JavaClass? {
        log.debug("getClassOfAliasName: $classFullName")
        return classAliasMap[classFullName]
    }

    /**
     * Map a class full name to its JavaClass instance.
     * 
     * @param classFullName The full name of the class
     * @param javaClass The JavaClass instance
     */
    @Synchronized
    fun setClassFullNameToAliasMap(classFullName: String, javaClass: JavaClass) {
        log.debug("setClassFullNameToAliasMap: $classFullName")
        classAliasMap[classFullName] = javaClass
    }

    /**
     * Get the cross-reference nodes for the given parameter name.
     * 
     * @param javaNodeName The parameter name
     * @return List of JavaNode references, or null if not found
     */
    @Synchronized
    fun getXrefNodes(javaNodeName: String): MutableList<JavaNode>? {
        log.debug("getXrefNodes: $javaNodeName")
        return xrefMap[javaNodeName]
    }

    /**
     * Set the cross-reference nodes for the given parameter name.
     * 
     * @param javaNodeName The parameter name
     * @param nodeList List of JavaNode references
     */
    @Synchronized
    fun setXrefMap(javaNodeName: String, nodeList: MutableList<JavaNode>) {
        log.debug("setXrefMap: $javaNodeName for ${nodeList.size}")
        xrefMap[javaNodeName] = nodeList
    }

    /**
     * Get the service implementation class for the given interface name.
     * 
     * @param interfaceName The interface name
     * @return The service implementation JavaClass, or null if not found
     */
    @Synchronized
    fun getServiceImpl(interfaceName: String): JavaClass? {
        log.debug("getServiceImpl: $interfaceName")
        return systemServiceMap[interfaceName]
    }

    /**
     * Map a service interface to its implementation class.
     * 
     * @param interfaceName The interface name
     * @param serviceImpl The service implementation JavaClass
     */
    @Synchronized
    fun setServiceMap(interfaceName: String, serviceImpl: JavaClass) {
        log.debug("setServiceMap: $interfaceName to ${serviceImpl.fullName}")
        systemServiceMap[interfaceName] = serviceImpl
    }

    /**
     * Clear all cached data for this instance.
     */
    @Synchronized
    fun clearCache() {
        classAliasMap.clear()
        methodToClassMap.clear()
        xrefMap.clear()
        systemServiceMap.clear()
        log.debug("Cache cleared for instance")
    }

    /**
     * Get cache statistics for monitoring purposes.
     * 
     * @return Map containing cache size information
     */
    fun getCacheStats(): Map<String, Int> {
        return mapOf(
            "classAliasMap" to classAliasMap.size,
            "methodToClassMap" to methodToClassMap.size,
            "xrefMap" to xrefMap.size,
            "systemServiceMap" to systemServiceMap.size
        )
    }
}