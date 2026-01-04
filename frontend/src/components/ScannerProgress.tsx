import React, { useState, useEffect, useRef } from 'react';
import { X, Minus2, Square } from 'lucide-react';

/**
 * WebSocket connection status types
 */
type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

/**
 * Scan progress data structure from backend
 */
interface ScanProgressData {
  current: number;
  total: number;
  percentage: number;
  message: string;
  is_complete: boolean;
}

/**
 * WebSocket message structure
 */
interface WebSocketMessage {
  type: 'scan_progress' | 'notification';
  data: ScanProgressData | Record<string, any>;
}

/**
 * useWebSocket - Hook for managing WebSocket connections
 * 
 * Provides real-time updates for:
 * - Scan progress
 * - Task notifications
 * - System events
 */
export const useWebSocket = (url: string = 'ws://localhost:8000/ws') => {
  const [status, setStatus] = useState<ConnectionStatus>('disconnected');
  const [scanProgress, setScanProgress] = useState<ScanProgressData | null>(null);
  const [lastMessage, setLastMessage] = useState<WebSocketMessage | null>(null);
  const [error, setError] = useState<string | null>(null);
  
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const reconnectAttempts = useRef(0);
  const maxReconnectAttempts = 5;

  /**
   * Connect to WebSocket
   */
  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      return;
    }

    setStatus('connecting');
    setError(null);

    try {
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        setStatus('connected');
        reconnectAttempts.current = 0;
        console.log('[WebSocket] Connected to', url);
      };

      ws.onmessage = (event) => {
        try {
          const message: WebSocketMessage = JSON.parse(event.data);
          setLastMessage(message);

          if (message.type === 'scan_progress') {
            setScanProgress(message.data as ScanProgressData);
          }

          console.log('[WebSocket] Received:', message);
        } catch (err) {
          console.error('[WebSocket] Failed to parse message:', err);
        }
      };

      ws.onclose = (event) => {
        setStatus('disconnected');
        wsRef.current = null;

        if (!event.wasClean && reconnectAttempts.current < maxReconnectAttempts) {
          const delay = Math.min(1000 * Math.pow(2, reconnectAttempts.current), 30000);
          console.log(`[WebSocket] Reconnecting in ${delay}ms... (attempt ${reconnectAttempts.current + 1})`);
          
          reconnectTimeoutRef.current = setTimeout(() => {
            reconnectAttempts.current++;
            connect();
          }, delay);
        } else {
          console.log('[WebSocket] Connection closed');
        }
      };

      ws.onerror = (event) => {
        setStatus('error');
        setError('WebSocket connection error');
        console.error('[WebSocket] Error:', event);
      };

    } catch (err) {
      setStatus('error');
      setError('Failed to create WebSocket connection');
      console.error('[WebSocket] Connection error:', err);
    }
  }, [url]);

  /**
   * Disconnect from WebSocket
   */
  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }

    setStatus('disconnected');
    setScanProgress(null);
    setLastMessage(null);
    setError(null);
  }, []);

  /**
   * Send message to WebSocket
   */
  const sendMessage = useCallback((message: Record<string, any>) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message));
    } else {
      console.warn('[WebSocket] Cannot send message - not connected');
    }
  }, []);

  /**
   * Auto-connect on mount
   */
  useEffect(() => {
    connect();

    return () => {
      disconnect();
    };
  }, [connect, disconnect]);

  return {
    status,
    scanProgress,
    lastMessage,
    error,
    connect,
    disconnect,
    sendMessage,
    isConnected: status === 'connected',
  };
};

/**
 * ScannerProgress - Real-time scan progress component
 * 
 * Displays scan progress with real-time WebSocket updates
 */
export const ScannerProgress: React.FC = () => {
  const { status, scanProgress, error, isConnected } = useWebSocket();
  const [isMinimized, setIsMinimized] = useState(false);

  /**
   * Get status color
   */
  const getStatusColor = () => {
    switch (status) {
      case 'connected': return 'text-green-400';
      case 'connecting': return 'text-yellow-400';
      case 'error': return 'text-red-400';
      default: return 'text-zinc-400';
    }
  };

  /**
   * Get status text
   */
  const getStatusText = () => {
    switch (status) {
      case 'connected': return '已连接';
      case 'connecting': return '连接中...';
      case 'error': return '连接错误';
      default: return '未连接';
    }
  };

  /**
   * Close/minimize handlers
   */
  const handleClose = () => {
    // TODO: Implement close logic
    console.log('Close scanner progress');
  };

  const handleMinimize = () => {
    setIsMinimized(!isMinimized);
  };

  /**
   * Don't render if no scan progress and not connected
   */
  if (!scanProgress && !isConnected && status === 'disconnected') {
    return null;
  }

  return (
    <div className="fixed bottom-4 right-4 bg-zinc-900 border border-zinc-700 rounded-lg shadow-lg min-w-80 max-w-96">
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b border-zinc-700">
        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${isConnected ? 'bg-green-400' : 'bg-zinc-400'}`} />
          <span className="text-sm font-medium text-white">扫描进度</span>
          <span className={`text-xs ${getStatusColor()}`}>{getStatusText()}</span>
        </div>
        
        <div className="flex items-center gap-1">
          <button
            onClick={handleMinimize}
            className="p-1 hover:bg-zinc-800 rounded transition-colors"
            title={isMinimized ? '展开' : '最小化'}
          >
            <Minus2 size={14} className="text-zinc-400" />
          </button>
          <button
            onClick={handleClose}
            className="p-1 hover:bg-zinc-800 rounded transition-colors"
            title="关闭"
          >
            <X size={14} className="text-zinc-400" />
          </button>
        </div>
      </div>

      {/* Content */}
      {!isMinimized && (
        <div className="p-4">
          {error ? (
            <div className="text-red-400 text-sm">{error}</div>
          ) : scanProgress ? (
            <div className="space-y-3">
              {/* Progress Bar */}
              <div className="space-y-1">
                <div className="flex justify-between text-sm">
                  <span className="text-zinc-300">扫描进度</span>
                  <span className="text-zinc-400">
                    {scanProgress.current} / {scanProgress.total}
                  </span>
                </div>
                <div className="w-full bg-zinc-800 rounded-full h-2">
                  <div
                    className="bg-blue-500 h-2 rounded-full transition-all duration-300"
                    style={{ width: `${scanProgress.percentage}%` }}
                  />
                </div>
                <div className="text-right text-xs text-zinc-400">
                  {scanProgress.percentage.toFixed(1)}%
                </div>
              </div>

              {/* Status Message */}
              <div className="text-sm text-zinc-300">
                {scanProgress.message}
              </div>

              {/* Complete indicator */}
              {scanProgress.is_complete && (
                <div className="text-green-400 text-sm font-medium">
                  ✅ 扫描完成
                </div>
              )}
            </div>
          ) : (
            <div className="text-zinc-400 text-sm">
              {status === 'connecting' ? '正在连接到扫描服务...' : '等待扫描开始...'}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default ScannerProgress;