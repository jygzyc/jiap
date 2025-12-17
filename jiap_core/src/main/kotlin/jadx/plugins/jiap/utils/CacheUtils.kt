package jadx.plugins.jiap.utils

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write
import kotlin.math.min

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
}

object CacheUtils {
    private const val CACHE_MAX_SIZE = 7
    private const val CACHE_TTL_MINUTES = 10

    private var responseCache: LRUCacheWithTTL<String, Any>? = null
    private var hits: Long = 0
    private var misses: Long = 0

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

    private fun buildBaseCacheKey(endpoint: String, parameters: Map<String, Any>?): String {
        val paramsString = if (parameters != null) {
            parameters.entries
                .sortedBy { it.key }
                .joinToString(",") { "${it.key}=${it.value}" }
        } else {
            ""
        }
        return "${endpoint}::${paramsString}"
    }

    /**
     * Get cached response
     */
    fun get(endpoint: String, parameters: Map<String, Any>?): Any? {
        val cache = responseCache ?: return null
        val cacheKey = buildBaseCacheKey(endpoint, parameters)

        val result = cache.get(cacheKey)
        if (result != null) {
            hits++
        } else {
            misses++
        }
        return result
    }

    /**
     * Cache response data
     */
    fun put(endpoint: String, parameters: Map<String, Any>?, response: Any) {
        val cache = responseCache ?: return
        val cacheKey = buildBaseCacheKey(endpoint, parameters)
        cache.put(cacheKey, response)
    }
}