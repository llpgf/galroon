/**
 * Tests for useLibrary hook.
 *
 * Phase 22.0: Pagination support
 * Phase 19.6: Semantic Sanitization
 * Phase 18.5: Custom User Tags
 */

import { renderHook, waitFor, act } from '@testing-library/react'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import useLibrary from '../useLibrary'
import { api } from '../../api/client'

// Mock the api module
vi.mock('../../api/client', () => ({
  api: {
    getAllGames: vi.fn()
  }
}))

// Mock the store
vi.mock('../../store/libraryStore', () => ({
  useLibraryStore: vi.fn((selector) => {
    const state = {
      filters: { tags: [] },
      viewMode: { type: 'grid' }
    }
    return selector(state)
  })
}))

describe('useLibrary', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('Initial State', () => {
    it('should have empty initial assets', () => {
      const { result } = renderHook(() => useLibrary())

      expect(result.current.assets).toEqual([])
    })

    it('should have loading state true on mount', () => {
      const { result } = renderHook(() => useLibrary())

      expect(result.current.isLoading).toBe(true)
    })

    it('should have null error initially', () => {
      const { result } = renderHook(() => useLibrary())

      expect(result.current.error).toBe(null)
    })

    it('should start at page 0', () => {
      const { result } = renderHook(() => useLibrary())

      expect(result.current.currentPage).toBe(0)
    })

    it('should have default sort by "最近添加"', () => {
      const { result } = renderHook(() => useLibrary())

      expect(result.current.sortBy).toBe('最近添加')
    })
  })

  describe('Fetching Games', () => {
    it('should fetch games on mount', async () => {
      const mockGames = [
        {
          folder_path: 'Game1',
          title: 'Game One',
          developer: 'Dev1',
          cover_image: '/cover1.jpg',
          badges: ['ISO'],
          library_status: 'unstarted',
          rating: 8.0,
          release_date: '2024-01-01',
          tags: ['vn'],
          user_tags: []
        }
      ]

      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: mockGames,
          total: 1,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      expect(api.getAllGames).toHaveBeenCalledTimes(1)
      expect(result.current.assets).toEqual(mockGames)
      expect(result.current.totalAssets).toBe(1)
    })

    it('should set error on fetch failure', async () => {
      const mockError: any = {
        response: {
          data: {
            message: 'Network error'
          }
        },
        message: 'Failed to fetch'
      }

      vi.mocked(api.getAllGames).mockRejectedValueOnce({
        apiError: {
          status: 500,
          message: 'Internal Server Error'
        }
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      expect(result.current.error).toBe('Internal Server Error')
      expect(result.current.assets).toEqual([])
    })

    it('should handle 404 endpoint error gracefully', async () => {
      vi.mocked(api.getAllGames).mockRejectedValueOnce({
        apiError: {
          status: 404,
          message: 'Not Found'
        }
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      expect(result.current.error).toContain('CRITICAL:')
      expect(result.current.error).toContain('/api/games')
    })
  })

  describe('Search Functionality', () => {
    it('should update search query', async () => {
      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [],
          total: 0,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      act(() => {
        result.current.setSearchQuery('test game')
      })

      expect(result.current.searchQuery).toBe('test game')
    })

    it('should fetch with search parameter', async () => {
      const mockGames = [
        {
          folder_path: 'TestGame',
          title: 'Test Game',
          developer: 'Dev',
          cover_image: '/cover.jpg',
          badges: [],
          library_status: 'unstarted',
          rating: null,
          release_date: null,
          tags: [],
          user_tags: []
        }
      ]

      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: mockGames,
          total: 1,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      act(() => {
        result.current.setSearchQuery('test')
      })

      await waitFor(() => {
        expect(api.getAllGames).toHaveBeenCalledWith(
          expect.objectContaining({
            search: 'test'
          })
        )
      })
    })
  })

  describe('Pagination', () => {
    it('should calculate total pages correctly', async () => {
      const mockGames = Array.from({ length: 50 }, (_, i) => ({
        folder_path: `Game${i}`,
        title: `Game ${i}`,
        developer: 'Dev',
        cover_image: '/cover.jpg',
        badges: [],
        library_status: 'unstarted',
        rating: null,
        release_date: null,
        tags: [],
        user_tags: []
      }))

      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: mockGames,
          total: 150, // 3 pages with 50 items per page
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      expect(result.current.totalPages).toBe(3)
    })

    it('should navigate to next page', async () => {
      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [],
          total: 150,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      act(() => {
        result.current.goToPage(1) // Go to page 2 (0-indexed)
      })

      expect(result.current.currentPage).toBe(1)
    })

    it('should fetch with correct skip parameter on page change', async () => {
      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [],
          total: 150,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      act(() => {
        result.current.goToPage(1)
      })

      await waitFor(() => {
        expect(api.getAllGames).toHaveBeenCalledWith(
          expect.objectContaining({
            skip: 50 // page 1 (0-indexed) * 50 = 50
          })
        )
      })
    })
  })

  describe('Sorting', () => {
    it('should change sort by parameter', async () => {
      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [],
          total: 0,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      act(() => {
        result.current.setSortBy('名称')
      })

      expect(result.current.sortBy).toBe('名称')
    })

    it('should fetch with sort_by parameter', async () => {
      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [],
          total: 0,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      act(() => {
        result.current.setSortBy('评分')
      })

      await waitFor(() => {
        expect(api.getAllGames).toHaveBeenCalledWith(
          expect.objectContaining({
            sort_by: '评分'
          })
        )
      })
    })
  })

  describe('Refresh', () => {
    it('should call fetchGames when refresh is called', async () => {
      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [],
          total: 0,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      const callCount = vi.mocked(api.getAllGames).mock.calls.length

      act(() => {
        result.current.refresh()
      })

      await waitFor(() => {
        expect(vi.mocked(api.getAllGames).mock.calls.length).toBe(callCount + 1)
      })
    })
  })

  describe('Metadata Transformation', () => {
    it('should transform title with multilingual fallback', async () => {
      const mockGame = {
        folder_path: 'Game1',
        title: {
          value: {
            zh_hant: '繁體中文標題',
            zh_hans: '简体中文标题',
            en: 'English Title',
            ja: '日本語タイトル',
            original: 'Original Title'
          },
          locked: false
        },
        developer: { value: 'Dev', locked: false },
        cover_url: { value: '/cover.jpg', locked: false },
        assets_detected: [],
        library_status: { value: 'unstarted' },
        tags: [],
        user_tags: []
      }

      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [mockGame],
          total: 1,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      // Should use zh_hant (highest priority)
      expect(result.current.assets[0].title).toBe('繁體中文標題')
    })

    it('should fallback to next language if primary is missing', async () => {
      const mockGame = {
        folder_path: 'Game1',
        title: {
          value: {
            en: 'English Title',
            ja: '日本語タイトル'
          },
          locked: false
        },
        developer: { value: 'Dev', locked: false },
        cover_url: { value: '/cover.jpg', locked: false },
        assets_detected: [],
        library_status: { value: 'unstarted' },
        tags: [],
        user_tags: []
      }

      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [mockGame],
          total: 1,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      // Should fallback to en
      expect(result.current.assets[0].title).toBe('English Title')
    })

    it('should transform assets_detected to badges', async () => {
      const mockGame = {
        folder_path: 'Game1',
        title: { value: { en: 'Game' }, locked: false },
        developer: { value: 'Dev', locked: false },
        cover_url: { value: '/cover.jpg', locked: false },
        assets_detected: ['ISO', 'DLC', 'Patch', 'OST', 'Chinese'],
        library_status: { value: 'unstarted' },
        tags: [],
        user_tags: []
      }

      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [mockGame],
          total: 1,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      // Should only include ISO, DLC, Patch (filter out OST, Chinese)
      expect(result.current.assets[0].badges).toEqual(['ISO', 'DLC', 'Patch'])
    })

    it('should use cover_path if available', async () => {
      const mockGame = {
        folder_path: 'Game1',
        title: { value: { en: 'Game' }, locked: false },
        developer: { value: 'Dev', locked: false },
        cover_path: '/local/path/cover.jpg',
        cover_url: { value: '/remote/path/cover.jpg', locked: false },
        assets_detected: [],
        library_status: { value: 'unstarted' },
        tags: [],
        user_tags: []
      }

      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: [mockGame],
          total: 1,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      // Should prefer local cover_path
      expect(result.current.assets[0].coverImage).toBe('/local/path/cover.jpg')
    })
  })

  describe('Filtered Assets', () => {
    it('should provide filtered assets based on search query', async () => {
      const mockGames = [
        {
          folder_path: 'TestGame',
          title: 'Test Game',
          developer: 'Dev',
          cover_image: '/cover.jpg',
          badges: [],
          library_status: 'unstarted',
          rating: null,
          release_date: null,
          tags: [],
          user_tags: []
        },
        {
          folder_path: 'AnotherGame',
          title: 'Another Game',
          developer: 'Dev',
          cover_image: '/cover2.jpg',
          badges: [],
          library_status: 'unstarted',
          rating: null,
          release_date: null,
          tags: [],
          user_tags: []
        }
      ]

      vi.mocked(api.getAllGames).mockResolvedValueOnce({
        data: {
          data: mockGames,
          total: 2,
          page: 1,
          size: 50,
          strategy: 'sqlite'
        },
        success: true
      })

      const { result } = renderHook(() => useLibrary())

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false)
      })

      act(() => {
        result.current.setSearchQuery('Test')
      })

      // filteredAssets should match search
      expect(result.current.filteredAssets.length).toBe(1)
      expect(result.current.filteredAssets[0].title).toBe('Test Game')
    })
  })
})
