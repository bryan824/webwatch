import { QueryClient, QueryClientProvider } from '@tanstack/solid-query';
import { RouterProvider, createRouter, createRoute, createRootRoute } from '@tanstack/solid-router';
import { Tooltip } from '@msviderok/base-ui-solid';
import { Shell } from './components/Shell';
import { Home } from './pages/Home';
import { Detail } from './pages/Detail';
import { NewWatch } from './pages/NewWatch';
import { EditWatch } from './pages/EditWatch';
import { ToastContainer } from './components/Toast';

const queryClient = new QueryClient();

const rootRoute = createRootRoute({
  component: Shell,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: Home,
});

const detailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/watches/$id',
  component: Detail,
});

const newWatchRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/watches/new',
  component: NewWatch,
});

const editWatchRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/watches/$id/edit',
  component: EditWatch,
});

const routeTree = rootRoute.addChildren([indexRoute, newWatchRoute, editWatchRoute, detailRoute]);
const router = createRouter({ routeTree });

declare module '@tanstack/solid-router' {
  interface Register {
    router: typeof router;
  }
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Tooltip.Provider>
        <RouterProvider router={router} />
        <ToastContainer />
      </Tooltip.Provider>
    </QueryClientProvider>
  );
}
