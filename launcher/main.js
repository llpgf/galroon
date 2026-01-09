/**
 * Electron Main Process - Vnite Launcher
 *
 * Phase 25.0: The Green Release - Portable application launcher
 *
 * Lifecycle:
 * 1. Development: Spawns Python backend process (backend/run.py)
 * 2. Production: Spawns frozen backend (resources/backend/backend.exe)
 * 3. Cleanup: Kills subprocess on app quit
 */

const { app, BrowserWindow, shell, ipcMain } = require('electron');
const path = require('path');
const { spawn } = require('child_process');
const fs = require('fs');
const kill = require('tree-kill');
const { randomUUID } = require('crypto');
const { registerIpcHandlers } = require('./ipc');
const portfinder = require('portfinder');

let mainWindow = null;
let backendProcess = null;
let logStream = null;
let sessionToken = null;  // PHASE 27.0: Store session token for frontend
let apiPort = null;  // PHASE 28.0: Dynamically allocated API port

// Make sessionToken globally accessible for IPC handlers
global.sessionToken = null;

// ============================================================
// PHASE 26.0: PORTABLE PATHS
// ============================================================

// Determine portable app root
const isDev = !app.isPackaged;
// In Dev: Use current folder. In Prod: Use folder containing Vnite.exe
const APP_ROOT = isDev ? path.join(__dirname, '..') : path.dirname(process.execPath);
const LOG_DIR = path.join(APP_ROOT, 'logs');

console.log('============================================================');
console.log('PHASE 26.0: PORTABLE TELEMETRY');
console.log('============================================================');
console.log(`Mode: ${isDev ? 'DEVELOPMENT' : 'PRODUCTION'}`);
console.log(`App Root: ${APP_ROOT}`);
console.log(`Log Directory: ${LOG_DIR}`);
console.log('============================================================');

// Ensure log directory exists
if (!fs.existsSync(LOG_DIR)) {
  fs.mkdirSync(LOG_DIR, { recursive: true });
  console.log('[OK] Created log directory:', LOG_DIR);
}

// Configure crash dumps path (Electron built-in)
app.setPath('crashDumps', path.join(LOG_DIR, 'crashes'));

// ============================================================
// LOGGING SETUP
// ============================================================

function setupLogging() {
  const logFile = path.join(LOG_DIR, `vnite-${Date.now()}.log`);

  logStream = fs.createWriteStream(logFile, { flags: 'a' });

  logStream.write(`\n${'='.repeat(70)}\n`);
  logStream.write(`VNITE LAUNCHER STARTED: ${new Date().toISOString()}\n`);
  logStream.write(`${'='.repeat(70)}\n`);

  // Redirect console to log file
  const originalLog = console.log;
  const originalError = console.error;

  console.log = (...args) => {
    originalLog(...args);
    const message = args.map(arg =>
      typeof arg === 'object' ? JSON.stringify(arg, null, 2) : String(arg)
    ).join(' ');
    logStream.write(`[LOG] ${message}\n`);
  };

  console.error = (...args) => {
    originalError(...args);
    const message = args.map(arg =>
      typeof arg === 'object' ? JSON.stringify(arg, null, 2) : String(arg)
    ).join(' ');
    logStream.write(`[ERROR] ${message}\n`);
  };
}

// ============================================================
// BACKEND PROCESS MANAGEMENT
// ============================================================

// PHASE 28.0: Dynamic Port Allocation
// =======================================
function getAvailablePort() {
  const DEFAULT_PORT = 8000;
  const PORT_RANGE_START = 8000;
  const PORT_RANGE_END = 8999;

  return new Promise((resolve, reject) => {
    portfinder.getPort({
      port: DEFAULT_PORT,
      stopPort: false
    }, (err, port) => {
      if (err) {
        console.error('[PHASE 28.0] Failed to find available port:', err);
        // Fallback: try sequential ports
        const fallbackPort = Math.floor(Math.random() * (PORT_RANGE_END - PORT_RANGE_START + 1)) + PORT_RANGE_START;
        console.warn(`[PHASE 28.0] Using fallback port: ${fallbackPort}`);
        resolve(fallbackPort);
      } else {
        console.log(`[PHASE 28.0] Found available port: ${port}`);
        resolve(port);
      }
    });
  });
}

function getBackendPath() {
  if (process.env.NODE_ENV === 'development') {
    // Development: Use Python script
    return {
      command: 'python',
      args: [path.join(__dirname, '..', 'backend', 'run.py')],
      name: 'Python Backend'
    };
  } else {
    // Production: Use frozen executable
    const backendExe = process.platform === 'win32'
      ? 'backend.exe'
      : 'backend';

    const backendPath = path.join(process.resourcesPath, 'backend', backendExe);

    return {
      command: backendPath,
      args: [],
      name: 'Frozen Backend'
    };
  }
}

async function startBackend() {
  const { command, args, name } = getBackendPath();

  console.log(`Starting ${name}...`);
  console.log(`Command: ${command}`);
  console.log(`Args: ${args.join(' ')}`);

  // PHASE 28.0: Get available port dynamically
  apiPort = await getAvailablePort();
  console.log(`[PHASE 28.0] API will run on port: ${apiPort}`);

  // PHASE 27.0: Generate session token for API authentication
  sessionToken = randomUUID();
  global.sessionToken = sessionToken;  // Make available globally for IPC
  console.log(`[PHASE 27.0] Generated SESSION_TOKEN: ${sessionToken}`);

  // PHASE 26.0: Pass portable paths to backend via environment variables
  const backendEnv = {
    ...process.env,
    GALGAME_ENV: process.env.GALGAME_ENV || 'production',
    PYTHONUNBUFFERED: '1', // Force unbuffered output
    // PHASE 26.0: Portable logging and data paths
    VNITE_LOG_PATH: LOG_DIR,        // Tell Python where to write logs
    VNITE_DATA_PATH: APP_ROOT,      // Tell Python where to store DB/config
    // PHASE 27.0: Session token for API authentication
    SESSION_TOKEN: sessionToken,       // Pass token to backend for validation
    // PHASE 28.0: Dynamic API port
    VNITE_API_PORT: apiPort.toString()  // Pass dynamically allocated port
  };

  console.log(`[PHASE 26.0] Backend Log Path: ${backendEnv.VNITE_LOG_PATH}`);
  console.log(`[PHASE 26.0] Backend Data Path: ${backendEnv.VNITE_DATA_PATH}`);
  console.log(`[PHASE 27.0] Session Token sent to backend`);
  console.log(`[PHASE 28.0] API Port sent to backend: ${apiPort}`);

  // Spawn backend process
  backendProcess = spawn(command, args, {
    cwd: APP_ROOT,  // PHASE 26.0: Use portable app root as working directory
    stdio: ['ignore', 'pipe', 'pipe'],
    env: backendEnv
  });

  // Pipe stdout to log
  backendProcess.stdout.on('data', (data) => {
    const message = data.toString().trim();
    if (message) {
      console.log(`[BACKEND] ${message}`);
    }
  });

  // Pipe stderr to log
  backendProcess.stderr.on('data', (data) => {
    const message = data.toString().trim();
    if (message) {
      console.error(`[BACKEND ERROR] ${message}`);
    }
  });

  // Handle backend exit
  backendProcess.on('close', (code) => {
    console.log(`${name} exited with code ${code}`);
    if (code !== 0 && code !== null) {
      console.error(`${name} crashed! Check logs for details.`);
    }
  });

  // Handle backend error
  backendProcess.on('error', (err) => {
    console.error(`Failed to start ${name}:`, err);
  });

  console.log(`${name} started (PID: ${backendProcess.pid})`);
}

function stopBackend() {
  if (backendProcess) {
    console.log('Stopping backend...');

    try {
      // Use tree-kill to kill Python process and all its children
      // This prevents zombie processes on shutdown
      kill(backendProcess.pid, 'SIGTERM', (err) => {
        if (err) {
          console.error('Error killing backend process tree:', err);
        } else {
          console.log(`Successfully killed backend process tree (PID: ${backendProcess.pid})`);
        }
      });

      // Force kill after 5 seconds if tree-kill doesn't work
      setTimeout(() => {
        if (backendProcess && !backendProcess.killed) {
          console.log('Force killing backend...');
          kill(backendProcess.pid, 'SIGKILL');
        }
      }, 5000);

    } catch (err) {
      console.error('Error stopping backend:', err);
    }

    backendProcess = null;
  }
}

// ============================================================
// WINDOW MANAGEMENT
// ============================================================

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    minWidth: 1024,
    minHeight: 768,
    frame: true,
    titleBarStyle: 'default',
    backgroundColor: '#18181b', // Match zinc-900 background
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js'),
      devTools: true // Enable DevTools for debugging
    }
  });

  // Load app
  if (process.env.NODE_ENV === 'development') {
    // Development: Load from dev server
    mainWindow.loadURL('http://localhost:5173');
    mainWindow.webContents.openDevTools();
  } else {
    // Production: Load from built files
    // PHASE 26.1: Simplified path logic - no more trial and error
    const isProd = app.isPackaged;

    // Determine frontend file location
    // Packaged: resources/frontend/index.html
    // Development fallback: ../frontend/dist/index.html
    const frontendPath = isProd
      ? path.join(process.resourcesPath, 'frontend', 'index.html')
      : path.join(__dirname, '../frontend/dist/index.html');

    console.log('[PHASE 26.1] Loading frontend from:', frontendPath);

    // Load frontend directly
    mainWindow.loadFile(frontendPath).catch(err => {
      console.error('[PHASE 26.1] Failed to load frontend:', err.message);
    });
  }

  mainWindow.on('closed', () => {
    mainWindow = null;
  });

  // Open DevTools automatically in production for debugging (remove later)
  if (process.env.NODE_ENV !== 'development') {
    mainWindow.webContents.openDevTools();
  }

  console.log('Main window created');
}

// ============================================================
// APP LIFECYCLE
// ============================================================

app.on('ready', async () => {
  console.log('App ready');
  setupLogging();

  // PHASE 26.0: Register IPC handlers
  registerIpcHandlers();

  createWindow();
  // PHASE 28.0: Start backend with async port allocation
  await startBackend();
});

app.on('window-all-closed', () => {
  console.log('All windows closed');
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('will-quit', (event) => {
  console.log('App will quit');
  stopBackend();

  // Close log stream
  if (logStream) {
    logStream.write(`\n${'='.repeat(70)}\n`);
    logStream.write(`VNITE LAUNCHER STOPPED: ${new Date().toISOString()}\n`);
    logStream.write(`${'='.repeat(70)}\n\n`);
    logStream.end();
  }
});

app.on('second-instance', (event, commandLine, workingDirectory) => {
  // Focus existing window when opening second instance
  if (mainWindow) {
    if (mainWindow.isMinimized()) mainWindow.restore();
    mainWindow.focus();
  }
});

// ============================================================
// ERROR HANDLING
// ============================================================

process.on('uncaughtException', (err) => {
  console.error('Uncaught Exception:', err);
});

process.on('unhandledRejection', (err, promise) => {
  console.error('Unhandled Rejection at:', promise, 'reason:', err);
});
 
// Audit Trigger 
