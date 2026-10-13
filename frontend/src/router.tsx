/* [174A-103] Router SPA placeholder.
 *
 * Define las rutas mínimas que la SPA usará una vez se vayan migrando las
 * features (174A-104..107). Por ahora todas las páginas son stubs que
 * solo confirman que la navegación client-side funciona; cada feature
 * irá reemplazando su página al sacar su carpeta del exclude del tsconfig.
 *
 * Las rutas espejean los paths que conoce el SeoKamples del backend
 * (/sample/{slug}, /perfil/{username}, /blog/{slug}) y `seo::metadata`. */

import { lazy, Suspense, useEffect } from 'react';
import { NavLink, createBrowserRouter, Outlet, RouterProvider } from 'react-router-dom';
import Boton from './components/ui/Boton';
import ModalAuth from './components/auth/ModalAuth';
import NavPublico from './components/layout/NavPublico';
import { useAuthModalStore } from './app/stores/authModalStore';
import { useAuth } from './hooks/useAuth';
import { cuerpoDocumento } from './glory-core/platform/documento';

const HomePage = lazy(() => import('./pages/HomePage'));
const DiscoverPage = lazy(() => import('./pages/DiscoverPage'));
const DashboardPage = lazy(() => import('./pages/DashboardPage'));
const LoginPage = lazy(() => import('./pages/LoginPage'));
const RegisterPage = lazy(() => import('./pages/RegisterPage'));
const SamplePage = lazy(() => import('./pages/SamplePage'));
const PerfilPage = lazy(() => import('./pages/PerfilPage'));
/* [254A-B] BlogPage removida de produccion: el blog/articulos ya no es accesible desde la UI. */
const NotFoundPage = lazy(() => import('./pages/NotFoundPage'));

function Layout() {
  const auth = useAuth();
  const abrirAuth = useAuthModalStore((state) => state.abrir);

  useEffect(() => {
    const layoutMode = !auth.isAuthenticated && !auth.isLoading ? 'public' : 'app';
    cuerpoDocumento().dataset.layout = layoutMode;

    return () => {
      delete cuerpoDocumento().dataset.layout;
    };
  }, [auth.isAuthenticated, auth.isLoading]);

  const linkClassName = ({ isActive }: { isActive: boolean }) => (
    isActive ? 'enlace enlaceActivo' : 'enlace'
  );

  if (!auth.isAuthenticated && !auth.isLoading) {
    return (
      <div className="layoutPublico">
        <NavPublico />
        <main className="areaContenidoPublico">
          <Suspense fallback={<div className="cargando">Cargando…</div>}>
            <Outlet />
          </Suspense>
        </main>
        <ModalAuth />
      </div>
    );
  }

  return (
    <div className="aplicacion">
      <header className="cabecera">
        <NavLink to="/" className="logo">Kamples</NavLink>
        <nav className="navegacion">
          <NavLink to="/" className={linkClassName}>Inicio</NavLink>
          <NavLink to="/descubrir" className={linkClassName}>Descubrir</NavLink>
          <NavLink to="/dashboard" className={linkClassName}>Dashboard</NavLink>
          {/* [254A-B] Tab "Blog" removida de produccion. */}
          <a className="enlace" href="/swagger-ui/" target="_blank" rel="noopener noreferrer">API</a>
        </nav>
        <div className="accionesCabecera">
          {auth.isLoading ? (
            <span className="estadoNeutral">Sesión…</span>
          ) : auth.isAuthenticated && auth.user ? (
            <>
              <NavLink to={`/perfil/${auth.user.username}`} className="accionSecundaria">
                @{auth.user.username}
              </NavLink>
              <Boton className="accionFantasma" onClick={() => { void auth.logout(); }} type="button">
                Salir
              </Boton>
            </>
          ) : (
            <>
              <Boton className="accionSecundaria" onClick={() => abrirAuth('login')} type="button">Entrar</Boton>
              <Boton className="accionPrimaria" onClick={() => abrirAuth('registro')} type="button">Crear cuenta</Boton>
            </>
          )}
        </div>
      </header>
      <main className="contenido">
        <Suspense fallback={<div className="cargando">Cargando…</div>}>
          <Outlet />
        </Suspense>
      </main>
      <ModalAuth />
    </div>
  );
}

const router = createBrowserRouter([
  {
    path: '/',
    element: <Layout />,
    children: [
      { index: true, element: <HomePage /> },
      { path: 'descubrir', element: <DiscoverPage /> },
      { path: 'colecciones', element: <DiscoverPage /> },
      { path: 'musica', element: <DiscoverPage /> },
      { path: 'auth/login', element: <LoginPage /> },
      { path: 'auth/registro', element: <RegisterPage /> },
      { path: 'dashboard', element: <DashboardPage /> },
      { path: 'sample/:slug', element: <SamplePage /> },
      { path: 'perfil/:username', element: <PerfilPage /> },
      /* [254A-B] Rutas /blog y /blog/:slug removidas de produccion. */
      { path: '*', element: <NotFoundPage /> },
    ],
  },
]);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
