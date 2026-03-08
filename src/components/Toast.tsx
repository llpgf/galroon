// Toast — lightweight notification component for user-facing messages.

import { useState, useEffect, useCallback, createContext, useContext } from 'react';
import './Toast.css';

interface ToastMessage {
      id: number;
      text: string;
      type: 'success' | 'error' | 'info';
}

interface ToastContextType {
      showToast: (text: string, type?: 'success' | 'error' | 'info') => void;
}

const ToastContext = createContext<ToastContextType>({ showToast: () => { } });

export function useToast() {
      return useContext(ToastContext);
}

let nextId = 0;

export function ToastProvider({ children }: { children: React.ReactNode }) {
      const [toasts, setToasts] = useState<ToastMessage[]>([]);

      const showToast = useCallback((text: string, type: 'success' | 'error' | 'info' = 'info') => {
            const id = nextId++;
            setToasts(prev => [...prev, { id, text, type }]);
      }, []);

      const removeToast = useCallback((id: number) => {
            setToasts(prev => prev.filter(t => t.id !== id));
      }, []);

      return (
            <ToastContext.Provider value={{ showToast }}>
                  {children}
                  <div className="toast-container">
                        {toasts.map(toast => (
                              <ToastItem key={toast.id} toast={toast} onDismiss={removeToast} />
                        ))}
                  </div>
            </ToastContext.Provider>
      );
}

function ToastItem({ toast, onDismiss }: { toast: ToastMessage; onDismiss: (id: number) => void }) {
      useEffect(() => {
            const timer = setTimeout(() => onDismiss(toast.id), 4000);
            return () => clearTimeout(timer);
      }, [toast.id, onDismiss]);

      const icons = { success: '✓', error: '✕', info: 'ℹ' };

      return (
            <div className={`toast toast-${toast.type}`} onClick={() => onDismiss(toast.id)}>
                  <span className="toast-icon">{icons[toast.type]}</span>
                  <span className="toast-text">{toast.text}</span>
            </div>
      );
}
