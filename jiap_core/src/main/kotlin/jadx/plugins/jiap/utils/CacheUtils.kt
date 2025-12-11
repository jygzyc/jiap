package jadx.plugins.jiap.utils

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write

class LRUCacheWithTTL<K, V>(
    private val maxCapacity: Int,
    private val ttlMillis: Long
) {
    private data class CacheEntry<V>(
        val value: V,
        val expiryTime: Long
    )

    private val cache = ConcurrentHashMap<K, CacheEntry<V>>()
    private val accessOrder = LinkedHashSet<K>()
    private val lock = ReentrantReadWriteLock()

    /**
     * Retrieve a value from cache
     */
    fun get(key: K): V? {
        return lock.read {
            val entry = cache[key]
            if (entry == null) {
                null
            } else {
                // Check if entry has expired
                if (System.currentTimeMillis() > entry.expiryTime) {
                    // Entry expired, remove it
                    lock.write {
                        cache.remove(key)
                        accessOrder.remove(key)
                    }
                    null
                } else {
                    // Move to end (mark as recently used)
                    lock.write {
                        accessOrder.remove(key)
                        accessOrder.add(key)
                    }
                    entry.value
                }
            }
        }
    }

    /**
     * Put a value into cache
     */
    fun put(key: K, value: V) {
        val expiryTime = if (ttlMillis > 0) {
            System.currentTimeMillis() + ttlMillis
        } else {
            Long.MAX_VALUE // No expiration
        }

        lock.write {
            // Remove existing key if present
            cache.remove(key)
            accessOrder.remove(key)

            // Add new entry
            cache[key] = CacheEntry(value, expiryTime)
            accessOrder.add(key)

            // Evict oldest entries if over capacity
            while (cache.size > maxCapacity) {
                val oldestKey = accessOrder.firstOrNull()
                if (oldestKey != null) {
                    cache.remove(oldestKey)
                    accessOrder.remove(oldestKey)
                } else {
                    break
                }
            }
        }
    }

    /**
     * Clear all entries from cache
     */
    fun clear() {
        lock.write {
            cache.clear()
            accessOrder.clear()
        }
    }

    /**
     * Get current cache size
     */
    fun size(): Int = cache.size
}

object CacheUtils {
    private const val CACHE_MAX_SIZE = 7
    private const val CACHE_TTL_MINUTES = 10

    private var responseCache: LRUCacheWithTTL<String, Any>? = null

    init {
        initializeCache()
    }

    private fun initializeCache() {
        responseCache = LRUCacheWithTTL(
            maxCapacity = CACHE_MAX_SIZE,
            ttlMillis = CACHE_TTL_MINUTES * 60 * 1000L
        )
    }

    /**
     * Get cached response
     * @param endpoint API endpoint
     * @param parameters Request parameters as map
     * @param page Page number for pagination
     * @return Cached response or null
     */
    fun getCachedResponse(endpoint: String, parameters: Map<String, Any>?, page: Int = 1): Any? {
        val cache = responseCache ?: return null
        val cacheKey = buildCacheKey(endpoint, parameters, page)
        return cache.get(cacheKey)
    }

    /**
     * Cache API response
     * @param endpoint API endpoint
     * @param parameters Request parameters as map
     * @param page Page number for pagination
     * @param response Response to cache
     */
    fun cacheResponse(endpoint: String, parameters: Map<String, Any>?, page: Int, response: Any) {
        val cache = responseCache ?: return
        val cacheKey = buildCacheKey(endpoint, parameters, page)
        cache.put(cacheKey, response)
    }

    /**
     * Clear all cached responses
     */
    fun clearCache() {
        responseCache?.clear()
    }

    /**
     * Reinitialize cache
     */
    fun reinitializeCache() {
        clearCache()
        initializeCache()
    }

    private fun buildCacheKey(endpoint: String, parameters: Map<String, Any>?, page: Int): String {
        val paramsString = if (parameters != null) {
            parameters.entries
                .sortedBy { it.key }
                .joinToString(",") { "${it.key}=${it.value}" }
        } else {
            ""
        }
        return "${endpoint}::${paramsString}::page_${page}"
    }
}