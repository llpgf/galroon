import React from 'react';
import { Sidebar } from './Sidebar';
import { NavigationBar, type NavigationBarProps } from './NavigationBar';

/**
 * AppShell - Native Desktop Client Layout
 *
 * Uses Flexbox layout to match Figma design.
 * Full viewport, no scrolling on body.
 *
 * Layout Structure (matches Figma):
 * ┌──────────┬──────────────────────────────────────────┐
 * │ Sidebar  │ NavigationBar (64px)                     │
 * │ (80-192px)├──────────────────────────────────────────┤
 * │          │ Content Area (1fr)                       │
 * │          │                                          │
 * └──────────┴──────────────────────────────────────────┘
 */

interface AppShellProps extends Omit<NavigationBarProps, 'currentPage'> {
  children: React.ReactNode;
  currentPageName?: string;
  currentView?: string;
  onViewChange?: (view: string) => void;
}

export const AppShell: React.FC<AppShellProps> = ({
  children,
  currentPageName = 'Library',
  currentView = 'library',
  onViewChange,
  ...navigationBarProps
}) => {
  return (
    <div className="flex h-screen bg-zinc-900">
      {/* Sidebar */}
      <Sidebar activeItem={currentView} onItemClick={onViewChange || (() => {})} />

      {/* Right Content Area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Navigation Bar */}
        <NavigationBar
          {...navigationBarProps}
          currentPage={currentPageName}
        />

        {/* Main Content Area */}
        <main className="flex-1 overflow-y-auto">
          {children}
        </main>
      </div>
    </div>
  );
};

