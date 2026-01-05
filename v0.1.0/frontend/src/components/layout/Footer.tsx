import React, { useState, useEffect } from 'react';
import axios from 'axios';

/**
 * Footer - App Shell Footer
 *
 * Fixed height: 32px
 * Spans full width (sidebar + content)
 * Shows connection status, item count, etc.
 */

export const Footer: React.FC = () => {
  const [connectionStatus, setConnectionStatus] = useState<'checking' | 'connected' | 'disconnected'>('checking');
  const [itemCount, setItemCount] = useState<number>(0);

  useEffect(() => {
    // Check backend connection
    const checkConnection = async () => {
      try {
        const response = await axios.get('http://localhost:8000/api/analytics/dashboard', {
          timeout: 2000
        });
        if (response.status === 200) {
          setConnectionStatus('connected');
          setItemCount(response.data.total_works || 0);
        }
      } catch (error) {
        setConnectionStatus('disconnected');
      }
    };

    checkConnection();
    const interval = setInterval(checkConnection, 5000); // Check every 5s

    return () => clearInterval(interval);
  }, []);

  const getStatusColor = () => {
    switch (connectionStatus) {
      case 'connected':
        return '#10b981'; // green
      case 'disconnected':
        return '#ef4444'; // red
      default:
        return '#f59e0b'; // yellow
    }
  };

  return (
    <div className="app-footer">
      {/* Connection Status */}
      <div className="app-footer-left">
        <div className="connection-indicator">
          <span
            className="connection-dot"
            style={{ backgroundColor: getStatusColor() }}
          />
          <span className="connection-text">
            {connectionStatus === 'connected' && 'Backend Connected'}
            {connectionStatus === 'disconnected' && 'Backend Disconnected'}
            {connectionStatus === 'checking' && 'Connecting...'}
          </span>
        </div>
      </div>

      {/* Item Count */}
      <div className="app-footer-center">
        <span className="item-count">
          {itemCount} works in library
        </span>
      </div>

      {/* Version/Info */}
      <div className="app-footer-right">
        <span className="version-info">v0.1.0</span>
      </div>
    </div>
  );
};

/**
 * Footer Styles
 */
export const footerStyles = `
  .app-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 100%;
    padding: 0 24px;
    background: linear-gradient(
      180deg,
      rgba(18, 18, 20, 0.8) 0%,
      rgba(18, 18, 20, 0.95) 100%
    );
    backdrop-filter: blur(20px);
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
  }

  .app-footer-left,
  .app-footer-center,
  .app-footer-right {
    display: flex;
    align-items: center;
  }

  .connection-indicator {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .connection-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background-color: #f59e0b;
    animation: pulse 2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }

  .connection-text {
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .item-count {
    font-size: 12px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.6);
  }

  .version-info {
    font-size: 11px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.4);
    letter-spacing: 0.05em;
  }
`;
