// Router — app routes with lazy loading.

import { lazy, Suspense } from 'react';
import {
      createBrowserRouter,
      Navigate,
      RouterProvider,
} from 'react-router-dom';
import App from './App';

const Dashboard = lazy(() => import('./pages/Dashboard/Dashboard'));
const GalleryPage = lazy(() => import('./pages/Gallery/GalleryPage'));
const SettingsPage = lazy(() => import('./pages/Settings/SettingsPage'));
const WorkDetail = lazy(() => import('./pages/WorkDetail/WorkDetail'));
const EnrichmentReview = lazy(() => import('./pages/Enrichment/EnrichmentReview'));
const Characters = lazy(() => import('./pages/Characters/Characters'));
const Creators = lazy(() => import('./pages/Creators/Creators'));
const Collections = lazy(() => import('./pages/Collections/Collections'));
const BrandDetail = lazy(() => import('./pages/Brands/BrandDetail'));
const PersonDetail = lazy(() => import('./pages/Persons/PersonDetail'));
const YearInReview = lazy(() => import('./pages/YearInReview/YearInReview'));
const Workshop = lazy(() => import('./pages/Workshop/Workshop'));

function LoadingFallback() {
      return (
            <div style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  height: '100%',
                  color: 'var(--text-muted)',
                  fontSize: '0.9rem',
            }}>
                  Loading...
            </div>
      );
}

function LazyPage({ children }: { children: React.ReactNode }) {
      return <Suspense fallback={<LoadingFallback />}>{children}</Suspense>;
}

const router = createBrowserRouter([
      {
            path: '/',
            element: <App />,
            children: [
                  {
                        index: true,
                        element: <Navigate to="/library" replace />,
                  },
                  {
                        path: 'dashboard',
                        element: <LazyPage><Dashboard /></LazyPage>,
                  },
                  {
                        path: 'library',
                        element: <LazyPage><GalleryPage /></LazyPage>,
                  },
                  {
                        path: 'work/:id',
                        element: <LazyPage><WorkDetail /></LazyPage>,
                  },
                  {
                        path: 'characters',
                        element: <LazyPage><Characters /></LazyPage>,
                  },
                  {
                        path: 'creators',
                        element: <LazyPage><Creators /></LazyPage>,
                  },
                  {
                        path: 'brand/:name',
                        element: <LazyPage><BrandDetail /></LazyPage>,
                  },
                  {
                        path: 'person/:id',
                        element: <LazyPage><PersonDetail /></LazyPage>,
                  },
                  {
                        path: 'collections',
                        element: <LazyPage><Collections /></LazyPage>,
                  },
                  {
                        path: 'enrichment',
                        element: <LazyPage><EnrichmentReview /></LazyPage>,
                  },
                  {
                        path: 'settings',
                        element: <LazyPage><SettingsPage /></LazyPage>,
                  },
                  {
                        path: 'year-in-review',
                        element: <LazyPage><YearInReview /></LazyPage>,
                  },
                  {
                        path: 'workshop',
                        element: <LazyPage><Workshop /></LazyPage>,
                  },
            ],
      },
]);

export function AppRouter() {
      return <RouterProvider router={router} />;
}
