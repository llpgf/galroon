/**
 * Electron Main Process - Galroon Launcher
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
const { registerIpcHandlers } = require('./ipc');

let mainWindow = null;
let backendProcess = null;
let logStream = null;

// ============================================================
// PHASE 26.0: PORTABLE PATHS
// ============================================================

// Determine portable app root
const isDev = !app.isPackaged;
// In Dev: Use current folder. In Prod: Use folder containing Galroon.exe
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
  const logFile = path.join(LOG_DIR, `galroon-${Date.now()}.log`);

  logStream = fs.createWriteStream(logFile, { flags: 'a' });

  logStream.write(`\n${'='.repeat(70)}\n`);
  logStream.write(`GALROON LAUNCHER STARTED: ${new Date().toISOString()}\n`);
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

function startBackend() {
  const { command, args, name } = getBackendPath();

  console.log(`Starting ${name}...`);
  console.log(`Command: ${command}`);
  console.log(`Args: ${args.join(' ')}`);

  // PHASE 26.0: Pass portable paths to backend via environment variables
  const backendEnv = {
    ...process.env,
    GALGAME_ENV: process.env.GALGAME_ENV || 'production',
    PYTHONUNBUFFERED: '1', // Force unbuffered output
    // PHASE 26.0: Portable logging and data paths
    VNITE_LOG_PATH: LOG_DIR,        // Tell Python where to write logs
    VNITE_DATA_PATH: APP_ROOT       // Tell Python where to store DB/config
  };

  console.log(`[PHASE 26.0] Backend Log Path: ${backendEnv.VNITE_LOG_PATH}`);
  console.log(`[PHASE 26.0] Backend Data Path: ${backendEnv.VNITE_DATA_PATH}`);

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
      // Try graceful shutdown first
      if (process.platform === 'win32') {
        spawn('taskkill', ['/pid', backendProcess.pid.toString(), '/f']);
      } else {
        backendProcess.kill('SIGTERM');
      }

      // Force kill after 5 seconds
      setTimeout(() => {
        if (backendProcess && !backendProcess.killed) {
          console.log('Force killing backend...');
          backendProcess.kill('SIGKILL');
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

app.on('ready', () => {
  console.log('App ready');
  setupLogging();

  // PHASE 26.0: Register IPC handlers
  registerIpcHandlers();

  createWindow();
  startBackend();
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
