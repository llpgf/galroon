// Sidebar — grouped navigation + SFW toggle.

import { NavLink } from 'react-router-dom';
import { useSfwStore } from '../stores/sfwStore';
import './Sidebar.css';

interface NavSection {
      title?: string;
      items: { path: string; label: string; icon: string }[];
}

const NAV_SECTIONS: NavSection[] = [
      {
            items: [
                  { path: '/', label: 'Library', icon: '📚' },
                  { path: '/dashboard', label: 'Dashboard', icon: '📊' },
            ],
      },
      {
            title: 'Browse',
            items: [
                  { path: '/characters', label: 'Characters', icon: '👤' },
                  { path: '/creators', label: 'Creators', icon: '🎭' },
                  { path: '/collections', label: 'Collections', icon: '📂' },
            ],
      },
      {
            title: 'Tools',
            items: [
                  { path: '/workshop', label: 'Workshop', icon: '🔧' },
                  { path: '/enrichment', label: 'Enrichment', icon: '🔗' },
                  { path: '/year-in-review', label: 'Year Review', icon: '🏆' },
            ],
      },
      {
            items: [
                  { path: '/settings', label: 'Settings', icon: '⚙️' },
            ],
      },
];

export function Sidebar() {
      const { sfwMode, toggleSfw } = useSfwStore();

      return (
            <aside className="sidebar">
                  <div className="sidebar-brand">
                        <span className="brand-icon">🎮</span>
                        <span className="brand-name">Galroon</span>
                        <span className="brand-version">v0.5</span>
                  </div>

                  <nav className="sidebar-nav">
                        {NAV_SECTIONS.map((section, si) => (
                              <div key={si} className="nav-section">
                                    {section.title && <div className="nav-section-title">{section.title}</div>}
                                    {section.items.map(item => (
                                          <NavLink
                                                key={item.path}
                                                to={item.path}
                                                end={item.path === '/'}
                                                className={({ isActive }) => `nav-item ${isActive ? 'active' : ''}`}
                                          >
                                                <span className="nav-icon">{item.icon}</span>
                                                <span className="nav-label">{item.label}</span>
                                          </NavLink>
                                    ))}
                              </div>
                        ))}
                  </nav>

                  <div className="sidebar-footer">
                        <button
                              className={`sfw-toggle ${sfwMode ? 'active' : ''}`}
                              onClick={toggleSfw}
                              title={sfwMode ? 'SFW mode ON (covers blurred)' : 'SFW mode OFF'}
                        >
                              {sfwMode ? '🔒' : '🔓'} SFW
                        </button>
                        <div className="sidebar-status">
                              <span className="status-dot online" />
                              <span className="status-text">Ready</span>
                        </div>
                  </div>
            </aside>
      );
}
