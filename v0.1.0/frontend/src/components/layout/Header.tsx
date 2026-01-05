import React from 'react';
import { useLibraryStore } from '../../store/libraryStore';
import { useNavigate } from '../../context/NavigationContext';

/**
 * Header - App Shell Header
 *
 * Fixed height: 64px
 * Spans full width (sidebar + content)
 *
 * Phase 19: Navigation implemented ✅
 */

export const Header: React.FC = () => {
  const { viewMode, setViewMode } = useLibraryStore();
  const { navigate } = useNavigate();

  return (
    <div className="app-header">
      {/* Logo/Title */}
      <div className="app-header-left">
        <h1 className="app-header-title">Galgame Library</h1>
      </div>

      {/* View Mode Toggle */}
      <div className="app-header-center">
        <div className="view-toggle">
          <button
            className={`view-toggle-button ${(viewMode as any) === 'grid' ? 'active' : ''}`}
            onClick={() => setViewMode('grid' as any)}
            aria-label="Grid view"
          >
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <rect x="2" y="2" width="7" height="7" rx="1" stroke="currentColor" strokeWidth="2"/>
              <rect x="11" y="2" width="7" height="7" rx="1" stroke="currentColor" strokeWidth="2"/>
              <rect x="2" y="11" width="7" height="7" rx="1" stroke="currentColor" strokeWidth="2"/>
              <rect x="11" y="11" width="7" height="7" rx="1" stroke="currentColor" strokeWidth="2"/>
            </svg>
          </button>
          <button
            className={`view-toggle-button ${(viewMode as any) === 'list' ? 'active' : ''}`}
            onClick={() => setViewMode('list' as any)}
            aria-label="List view"
          >
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <rect x="2" y="3" width="16" height="3" rx="1" stroke="currentColor" strokeWidth="2"/>
              <rect x="2" y="8.5" width="16" height="3" rx="1" stroke="currentColor" strokeWidth="2"/>
              <rect x="2" y="14" width="16" height="3" rx="1" stroke="currentColor" strokeWidth="2"/>
            </svg>
          </button>
        </div>
      </div>

      {/* Search/Actions */}
      <div className="app-header-right">
        <button
          className="icon-button"
          aria-label="Search"
          onClick={() => {
            // Phase 19.15: Trigger search focus
            const searchInput = document.querySelector('[data-search-input]') as HTMLInputElement;
            searchInput?.focus();
          }}
        >
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
            <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="2"/>
            <path d="M14 14L12 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
          </svg>
        </button>
        <button
          className="icon-button"
          aria-label="Settings"
          onClick={() => navigate('settings')}
        >
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
            <circle cx="10" cy="10" r="3" stroke="currentColor" strokeWidth="2"/>
            <path d="M10 2V4M10 16V18M18 10H16M4 10H2M15.66 4.34L14.24 5.76M5.76 14.24L4.34 15.66M15.66 15.66L14.24 14.24M5.76 5.76L4.34 4.34" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
          </svg>
        </button>
      </div>
    </div>
  );
};

/**
 * Header Styles
 */
export const headerStyles = `
  .app-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 100%;
    padding: 0 24px;
    background: linear-gradient(
      180deg,
      rgba(18, 18, 20, 0.95) 0%,
      rgba(18, 18, 20, 0.8) 100%
    );
    backdrop-filter: blur(20px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .app-header-left {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .app-header-title {
    font-size: 18px;
    font-weight: 600;
    color: #ffffff;
    margin: 0;
    letter-spacing: -0.02em;
  }

  .app-header-center {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .view-toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .view-toggle-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.5);
    border-radius: 6px;
    cursor: pointer;
    transition: all 150ms ease;
  }

  .view-toggle-button:hover {
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.8);
  }

  .view-toggle-button.active {
    background: rgba(255, 255, 255, 0.1);
    color: #ffffff;
  }

  .app-header-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .icon-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
    border-radius: 8px;
    cursor: pointer;
    transition: all 150ms ease;
  }

  .icon-button:hover {
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.9);
  }

  .icon-button:active {
    transform: scale(0.95);
  }
`;
