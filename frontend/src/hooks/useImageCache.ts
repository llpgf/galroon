import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

/**
 * Image cache status types
 */
type CacheStatus = 'loading' | 'cached' | 'downloading' | 'error' | 'not_cached';

/**
 * Image cache data structure
 */
interface ImageCacheData {
  url: string;
  local_path?: string;
  is_cached: boolean;
  file_size?: number;
  cache_size?: number;
}

/**
 * useImageCache - Hook for managing image cache operations
 * 
 * Provides:
 * - Check if image is cached
 * - Download and cache images
 * - Get cache statistics
 * - Clear cache
 */
export const useImageCache = () => {
  const [cacheStatus, setCacheStatus] = useState<Record<string, CacheStatus>>({});
  const [cacheStats, setCacheStats] = useState<{
    total_size: number;
    max_size: number;
    cached_count: number;
  } | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /**
   * Check if image is cached
   */
  const checkCacheStatus = useCallback(async (imageUrl: string): Promise<CacheStatus> => {
    try {
      setCacheStatus(prev => ({ ...prev, [imageUrl]: 'loading' }));
      
      const response = await api.checkImageCache(imageUrl);
      const isCached = response.data.is_cached;
      
      const status: CacheStatus = isCached ? 'cached' : 'not_cached';
      setCacheStatus(prev => ({ ...prev, [imageUrl]: status }));
      
      return status;
    } catch (err) {
      console.error('[useImageCache] Failed to check cache status:', err);
      setCacheStatus(prev => ({ ...prev, [imageUrl]: 'error' }));
      return 'error';
    }
  }, []);

  /**
   * Download and cache image
   */
  const downloadImage = useCallback(async (imageUrl: string): Promise<string | null> => {
    try {
      setCacheStatus(prev => ({ ...prev, [imageUrl]: 'downloading' }));
      setError(null);

      const response = await api.downloadImageCache(imageUrl);
      const localPath = response.data.local_path;

      if (localPath) {
        setCacheStatus(prev => ({ ...prev, [imageUrl]: 'cached' }));
        console.log(`[useImageCache] ✅ Cached image: ${imageUrl} -> ${localPath}`);
        return localPath;
      } else {
        setCacheStatus(prev => ({ ...prev, [imageUrl]: 'error' }));
        setError('Failed to cache image');
        return null;
      }
    } catch (err) {
      console.error('[useImageCache] Failed to download image:', err);
      setCacheStatus(prev => ({ ...prev, [imageUrl]: 'error' }));
      setError((err as any)?.response?.data?.detail || 'Failed to download image');
      return null;
    }
  }, []);

  /**
   * Get image (cached or download)
   */
  const getImage = useCallback(async (imageUrl: string): Promise<string | null> => {
    // First check if cached
    const status = await checkCacheStatus(imageUrl);
    
    if (status === 'cached') {
      return imageUrl; // Return original URL, backend will serve cached version
    }
    
    if (status === 'not_cached') {
      // Download and cache
      return await downloadImage(imageUrl);
    }
    
    return null;
  }, [checkCacheStatus, downloadImage]);

  /**
   * Get cache statistics
   */
  const getCacheStats = useCallback(async () => {
    try {
      setIsLoading(true);
      const response = await api.getImageCacheStats();
      setCacheStats(response.data);
    } catch (err) {
      console.error('[useImageCache] Failed to get cache stats:', err);
      setError('Failed to get cache statistics');
    } finally {
      setIsLoading(false);
    }
  }, []);

  /**
   * Clear cache
   */
  const clearCache = useCallback(async () => {
    try {
      setIsLoading(true);
      await api.clearImageCache();
      
      // Reset all cache statuses
      setCacheStatus({});
      setCacheStats(null);
      
      console.log('[useImageCache] ✅ Cache cleared');
    } catch (err) {
      console.error('[useImageCache] Failed to clear cache:', err);
      setError('Failed to clear cache');
    } finally {
      setIsLoading(false);
    }
  }, []);

  /**
   * Get cached image URL
   * Returns the appropriate URL to use for displaying the image
   */
  const getCachedImageUrl = useCallback((originalUrl: string): string => {
    const status = cacheStatus[originalUrl];
    
    if (status === 'cached') {
      // Use cached version endpoint
      return `/api/cache/image?url=${encodeURIComponent(originalUrl)}`;
    }
    
    // Use original URL
    return originalUrl;
  }, [cacheStatus]);

  /**
   * Auto-load cache stats on mount
   */
  useEffect(() => {
    getCacheStats();
  }, [getCacheStats]);

  return {
    cacheStatus,
    cacheStats,
    isLoading,
    error,
    checkCacheStatus,
    downloadImage,
    getImage,
    getCacheStats,
    clearCache,
    getCachedImageUrl,
  };
};

/**
 * ImageCacheIndicator - Small indicator showing cache status
 */
export const ImageCacheIndicator: React.FC<{
  imageUrl: string;
  className?: string;
}> = ({ imageUrl, className = '' }) => {
  const { cacheStatus } = useImageCache();
  const status = cacheStatus[imageUrl] || 'not_cached';

  const getStatusColor = () => {
    switch (status) {
      case 'cached': return 'bg-green-500';
      case 'downloading': return 'bg-yellow-500 animate-pulse';
      case 'error': return 'bg-red-500';
      case 'loading': return 'bg-blue-500 animate-pulse';
      default: return 'bg-zinc-500';
    }
  };

  const getStatusText = () => {
    switch (status) {
      case 'cached': return 'Cached';
      case 'downloading': return 'Downloading...';
      case 'error': return 'Error';
      case 'loading': return 'Checking...';
      default: return 'Not cached';
    }
  };

  return (
    <div className={`flex items-center gap-1 ${className}`}>
      <div className={`w-2 h-2 rounded-full ${getStatusColor()}`} />
      <span className="text-xs text-zinc-400">{getStatusText()}</span>
    </div>
  );
};

export default useImageCache;