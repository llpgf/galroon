import { useState, useEffect } from 'react';

interface ScanStatus {
  mode: 'REALTIME' | 'MANUAL' | 'SCHEDULED';
  is_running: boolean;
  detected_directories: number;
  detected_files: number;
}

interface HealthStatus {
  status: string;
  env: string;
  sandbox: boolean;
}

export default function StatusCard() {
  const [status, setStatus] = useState<ScanStatus | null>(null);
  const [health, setHealth] = useState<HealthStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [toggling, setToggling] = useState(false);

  const fetchStatus = async () => {
    try {
      const response = await fetch('/api/scan/status');
      if (response.ok) {
        const data = await response.json();
        setStatus(data);
      }

      // Also fetch health status for sandbox detection
      const healthResponse = await fetch('/api/health');
      if (healthResponse.ok) {
        const healthData = await healthResponse.json();
        setHealth(healthData);
      }
    } catch (error) {
      console.error('Failed to fetch scan status:', error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 2000); // Poll every 2 seconds
    return () => clearInterval(interval);
  }, []);

  const toggleMode = async () => {
    if (!status || toggling) return;

    setToggling(true);
    try {
      const newMode = status.mode === 'REALTIME' ? 'MANUAL' : 'REALTIME';
      const response = await fetch('/api/scan/mode', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode: newMode }),
      });

      if (response.ok) {
        await fetchStatus();
      } else {
        console.error('Failed to toggle mode');
      }
    } catch (error) {
      console.error('Failed to toggle mode:', error);
    } finally {
      setToggling(false);
    }
  };

  if (loading) {
    return (
      <div className="bg-gray-800 rounded-lg p-6 animate-pulse">
        <div className="h-20 bg-gray-700 rounded"></div>
      </div>
    );
  }

  if (!status) {
    return (
      <div className="glass-strong rounded-2xl p-6 border-destructive/50">
        <p className="text-lg font-semibold text-destructive">Failed to load scan status</p>
      </div>
    );
  }

  const isRealtime = status.mode === 'REALTIME';
  const isRunning = status.is_running;
  const isSandbox = health?.sandbox || false;

  return (
    <div className="relative">
      {/* Sandbox Warning Banner */}
      {isSandbox && (
        <div className="absolute -top-3 left-0 right-0 z-10">
          <div className="glass-strong border-yellow-500/50 bg-yellow-500/20 text-yellow-400 dark:text-yellow-300 font-semibold text-center py-2 px-4 rounded-xl shadow-lg animate-pulse flex items-center justify-center gap-2">
            <span>⚠️</span>
            <span>SANDBOX MODE - TESTING ENVIRONMENT</span>
            <span>⚠️</span>
          </div>
        </div>
      )}

      <div
        onClick={toggleMode}
        className={`
          glass-strong rounded-2xl p-6 cursor-pointer transition-all duration-300
          hover:scale-[1.02] hover:-translate-y-1 hover:shadow-2xl hover:shadow-primary/20
          ${toggling ? 'opacity-50 cursor-not-allowed' : ''}
          ${isSandbox ? 'mt-8' : ''}
        `}
      >
      <div className="flex items-center justify-between">
        <div className="flex-1">
          <h2 className="text-2xl font-bold text-foreground mb-2">Sentinel Scanner</h2>
          <div className="space-y-1">
            <p className="text-foreground/90 text-lg">
              Mode: <span className="font-mono font-semibold text-primary">{status.mode}</span>
            </p>
            <p className="text-muted-foreground">
              Status: {isRunning ? (
                <span className="flex items-center gap-2">
                  <span className={`w-2 h-2 rounded-full ${isRealtime ? 'bg-emerald-400 animate-pulse' : 'bg-muted-foreground'}`}></span>
                  <span className="text-emerald-400 font-medium">Running</span>
                </span>
              ) : (
                <span className="text-muted-foreground">Idle</span>
              )}
            </p>
          </div>
        </div>

        <div className="text-right">
          <div className={`
            w-24 h-24 rounded-full flex items-center justify-center
            ${isRealtime && isRunning ? 'bg-primary/20' : 'bg-muted/20'}
          `}>
            {isRealtime && isRunning ? (
              <div className="w-16 h-16 rounded-full bg-primary animate-pulse"></div>
            ) : (
              <div className="w-16 h-16 rounded-full bg-muted-foreground/30"></div>
            )}
          </div>
          <p className="text-muted-foreground mt-2 text-sm">Click to toggle</p>
        </div>
      </div>

      <div className="mt-4 pt-4 border-t border-border/50 grid grid-cols-2 gap-4">
        <div className="text-center">
          <p className="text-muted-foreground text-sm">Directories</p>
          <p className="text-foreground text-2xl font-bold">{status.detected_directories}</p>
        </div>
        <div className="text-center">
          <p className="text-muted-foreground text-sm">Files</p>
          <p className="text-foreground text-2xl font-bold">{status.detected_files}</p>
        </div>
      </div>
    </div>
    </div>
  );
}
