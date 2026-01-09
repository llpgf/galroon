import React, { createContext, useContext, useState } from 'react';
import type { ReactNode } from 'react';

/**
 * Navigation Context - Phase 19
 *
 * Provides routing functionality for the application.
 * Simple state-based routing until React Router is fully integrated.
 */

type Route = 'library' | 'settings' | 'trash' | 'organizer' | 'details';

interface NavigationContextType {
  currentRoute: Route;
  navigate: (route: Route, params?: Record<string, unknown> | null) => void;
  goBack: () => void;
  params: Record<string, unknown> | null;
}

const NavigationContext = createContext<NavigationContextType | undefined>(undefined);

interface NavigationProviderProps {
  children: ReactNode;
}

export const NavigationProvider: React.FC<NavigationProviderProps> = ({ children }) => {
  const [currentRoute, setCurrentRoute] = useState<Route>('library');
  const [params, setParams] = useState<Record<string, unknown> | null>(null);

  const navigate = (route: Route, newParams?: Record<string, unknown> | null) => {
    setCurrentRoute(route);
    setParams(newParams || null);
    window.scrollTo(0, 0);
  };

  const goBack = () => {
    setCurrentRoute('library');
    setParams(null);
  };

  return (
    <NavigationContext.Provider value={{ currentRoute, navigate, goBack, params }}>
      {children}
    </NavigationContext.Provider>
  );
};

export const useNavigate = () => {
  const context = useContext(NavigationContext);
  if (!context) {
    throw new Error('useNavigate must be used within NavigationProvider');
  }
  return context;
};

export default NavigationContext;
