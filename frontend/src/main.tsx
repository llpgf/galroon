import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { Toaster } from 'react-hot-toast'
import { NavigationProvider } from './context/NavigationContext'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <NavigationProvider>
      <App />
      <Toaster
        position="bottom-right"
        reverseOrder={false}
        toastOptions={{
          className: 'bg-zinc-800 text-white border border-zinc-700',
          success: {
            iconTheme: {
              primary: '#22c55e',
              secondary: '#fff',
            },
          },
          error: {
            iconTheme: {
              primary: '#ef4444',
              secondary: '#fff',
            },
          },
          loading: {
            iconTheme: {
              primary: '#3b82f6',
              secondary: '#fff',
            },
          },
        }}
      />
    </NavigationProvider>
  </StrictMode>,
)
