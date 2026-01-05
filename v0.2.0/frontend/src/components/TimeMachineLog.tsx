import { useState, useEffect } from 'react';

interface HistoryEntry {
  transaction_id: string;
  operation: string;
  details: string;
  timestamp: string;
}

interface UndoResponse {
  success: boolean;
  message: string;
}

export default function TimeMachineLog() {
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [undoing, setUndoing] = useState<string | null>(null);
  const [result, setResult] = useState<{ success: boolean; message: string } | null>(null);

  const fetchHistory = async () => {
    try {
      const response = await fetch('/api/history');
      if (response.ok) {
        const data = await response.json();
        setHistory(data.history || []);
      }
    } catch (error) {
      console.error('Failed to fetch history:', error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchHistory();
    const interval = setInterval(fetchHistory, 3000); // Poll every 3 seconds
    return () => clearInterval(interval);
  }, []);

  const undoOperation = async (transactionId: string) => {
    setUndoing(transactionId);
    setResult(null);
    try {
      const response = await fetch(`/api/history/${transactionId}/undo`, {
        method: 'POST',
      });

      if (response.ok) {
        const data = await response.json();
        setResult(data);
        await fetchHistory(); // Refresh history
      } else {
        setResult({ success: false, message: 'Failed to undo operation' });
      }
    } catch (error) {
      console.error('Failed to undo:', error);
      setResult({ success: false, message: 'Network error' });
    } finally {
      setUndoing(null);
    }
  };

  const formatTimestamp = (timestamp: string) => {
    const date = new Date(timestamp);
    return date.toLocaleString();
  };

  const getOperationIcon = (operation: string) => {
    switch (operation.toLowerCase()) {
      case 'delete':
        return '🗑️';
      case 'move':
        return '📦';
      case 'organize':
        return '🪄';
      default:
        return '⚙️';
    }
  };

  const getOperationColor = (operation: string) => {
    switch (operation.toLowerCase()) {
      case 'delete':
        return 'text-red-400';
      case 'move':
        return 'text-blue-400';
      case 'organize':
        return 'text-primary-400';
      default:
        return 'text-gray-400';
    }
  };

  const canUndo = (operation: string) => {
    return operation.toLowerCase() === 'delete';
  };

  if (loading) {
    return (
      <div className="bg-gray-800 rounded-lg p-6">
        <h2 className="text-white text-xl font-bold mb-4">Time Machine Log</h2>
        <div className="space-y-2 animate-pulse">
          <div className="h-16 bg-gray-700 rounded"></div>
          <div className="h-16 bg-gray-700 rounded"></div>
          <div className="h-16 bg-gray-700 rounded"></div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-gray-800 rounded-lg p-6">
      <h2 className="text-white text-xl font-bold mb-4 flex items-center gap-2">
        <span>⏰</span>
        Time Machine Log
      </h2>

      {result && (
        <div className={`
          p-4 rounded-lg mb-4
          ${result.success ? 'bg-green-900/50 border border-green-500' : 'bg-red-900/50 border border-red-500'}
        `}>
          <p className="text-white font-semibold">{result.message}</p>
        </div>
      )}

      <div className="bg-gray-950 rounded-lg p-4 max-h-[500px] overflow-y-auto font-mono text-sm">
        {history.length === 0 ? (
          <div className="text-center py-8">
            <p className="text-gray-400">No operations yet</p>
            <p className="text-gray-500 text-sm mt-2">Operations will appear here</p>
          </div>
        ) : (
          <div className="space-y-2">
            {history.map((entry, index) => (
              <div
                key={entry.transaction_id}
                className={`
                  bg-gray-900/50 rounded-lg p-4 border-l-4
                  ${entry.operation.toLowerCase() === 'delete' ? 'border-red-500' : 'border-gray-600'}
                  hover:bg-gray-900 transition-colors
                `}
              >
                <div className="flex items-start justify-between gap-4">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-2">
                      <span className="text-xl">{getOperationIcon(entry.operation)}</span>
                      <span className={`font-bold ${getOperationColor(entry.operation)}`}>
                        {entry.operation.toUpperCase()}
                      </span>
                      <span className="text-gray-500 text-xs ml-auto">
                        #{index + 1}
                      </span>
                    </div>
                    <p className="text-gray-300 break-words">{entry.details}</p>
                    <p className="text-gray-500 text-xs mt-2">
                      {formatTimestamp(entry.timestamp)}
                    </p>
                    <p className="text-gray-600 text-xs font-mono mt-1">
                      ID: {entry.transaction_id}
                    </p>
                  </div>

                  {canUndo(entry.operation) && (
                    <button
                      onClick={() => undoOperation(entry.transaction_id)}
                      disabled={undoing === entry.transaction_id}
                      className={`
                        px-4 py-2 rounded-lg font-semibold text-sm
                        flex items-center gap-2 whitespace-nowrap
                        transition-all duration-200
                        ${undoing === entry.transaction_id
                          ? 'bg-gray-600 cursor-not-allowed opacity-50'
                          : 'bg-primary-600 hover:bg-primary-500 active:scale-95'
                        }
                        text-white shadow-lg
                      `}
                    >
                      {undoing === entry.transaction_id ? (
                        <>
                          <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
                          Undoing...
                        </>
                      ) : (
                        <>
                          <span>↩️</span>
                          UNDO
                        </>
                      )}
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="mt-4 text-sm text-gray-400 text-center">
        {history.length} operation{history.length !== 1 ? 's' : ''} in history
      </div>
    </div>
  );
}
