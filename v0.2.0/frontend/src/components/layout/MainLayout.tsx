/**
 * Main Layout Component
 *
 * PHASE 13: The Framework
 *
 * Container with Sidebar + Content Area.
 * Includes backend connection test indicator.
 */

import React, { useEffect, useState } from 'react';
import { Sidebar } from './Sidebar';

interface MainLayoutProps {
  children: React.ReactNode;
}

const MainLayout: React.FC<MainLayoutProps> = ({ children }) => {
  const [isConnected, setIsConnected] = useState(false);
  const [gameCount, setGameCount] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    // Test backend connection
    const testConnection = async () => {
      try {
        const response = await fetch('http://localhost:8000/api/analytics/dashboard');
        if (response.ok) {
          const data = await response.json();
          setIsConnected(true);
          setGameCount(data.total_games || 0);
        } else {
          setIsConnected(false);
        }
      } catch (error) {
        console.error('Backend connection failed:', error);
        setIsConnected(false);
      } finally {
        setIsLoading(false);
      }
    };

    testConnection();
  }, []);

  return (
    <div className="flex h-screen bg-[#121214] text-white">
      {/* Sidebar */}
      <Sidebar activeItem="home" onItemClick={() => {}} />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Top Bar with Connection Indicator */}
        <div className="h-14 border-b border-gray-800 flex items-center justify-between px-6">
          <h1 className="text-xl font-semibold text-gray-200">Galgame Library</h1>

          {/* Connection Test Indicator */}
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-900/50 border border-gray-800">
            {isLoading ? (
              <>
                <div className="w-2 h-2 rounded-full bg-yellow-500 animate-pulse"></div>
                <span className="text-sm text-gray-400">Connecting...</span>
              </>
            ) : isConnected ? (
              <>
                <div className="w-2 h-2 rounded-full bg-green-500 shadow-lg shadow-green-500/50"></div>
                <span className="text-sm text-gray-300">
                  Connected to Backend | {gameCount} Games
                </span>
              </>
            ) : (
              <>
                <div className="w-2 h-2 rounded-full bg-red-500"></div>
                <span className="text-sm text-red-400">Backend Disconnected</span>
              </>
            )}
          </div>
        </div>

        {/* Content Area */}
        <div className="flex-1 overflow-auto">
          {children}
        </div>
      </div>
    </div>
  );
};

export default MainLayout;
