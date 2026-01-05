/**
 * useAnalytics Hook
 *
 * PHASE 13: The Framework
 *
 * Custom hook for fetching analytics data from backend.
 */

import { useState, useEffect } from 'react';
import { api } from '../api/client';

export interface DashboardStats {
  total_games: number;
  timeline: Record<string, number>;
  engines: Record<string, number>;
  play_time: Record<string, number>;
  tags: Array<{
    tag: string;
    count: number;
    weight: number;
  }>;
}

export const useAnalytics = () => {
  const [data, setData] = useState<DashboardStats | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchDashboardStats = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const response = await api.getDashboardStats();
      setData(response.data as DashboardStats);
    } catch (err: any) {
      console.error('Failed to fetch dashboard stats:', err);
      setError(err.message || 'Failed to fetch analytics');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchDashboardStats();
  }, []);

  return {
    data,
    isLoading,
    error,
    refetch: fetchDashboardStats,
  };
};
