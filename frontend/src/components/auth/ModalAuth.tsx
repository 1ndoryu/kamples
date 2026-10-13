import { useCallback, useState, type FormEvent } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { BotonBase } from '../../app/components/ui/BotonBase';
import CampoTexto from '../../app/components/ui/CampoTexto';
import { useT } from '../../app/utils/i18n/useT';
import { useAuth } from '../../hooks/useAuth';
import { useModalAuth } from '../../hooks/useModalAuth';
import IconoGoogle from '../ui/IconoGoogle';
import '../../styles/componentes/authModal.css';
import { cuerpoDocumento } from '../../glory-core/platform/documento';

const authImage = '/auth/2.jpg';

function FormularioLogin({ onCambiar }: { onCambiar: () => void }): JSX.Element {
  const { t } = useT();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const { cargando, errorMessage, iniciarSesion, googleBotonRef, esGoogleNativo, loginGoogleNativo } = useAuth();
  const { cerrar } = useModalAuth();

  const manejarSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await iniciarSesion(email.trim(), password);
      cerrar();
    } catch {
      /* El feedback lo maneja useAuth vía errorMessage. */
    }
  };

  return (
    <div className="authFormContenedor">
      <h2 className="authTitulo">{t('auth.login.titulo')}</h2>

      {esGoogleNativo ? (
        <BotonBase
          variante="secundario"
          anchoCompleto
          onClick={loginGoogleNativo}
          cargando={cargando}
          className="authGoogleBtnDesktop"
          type="button"
        >
          import { useT } from '../../app/utils/i18n/useT';
          <IconoGoogle />
          {t('auth.continuarConGoogle')}
        </BotonBase>
      ) : (
        <div ref={googleBotonRef} className="authGoogleBtnContenedor" />
      )}

      <div className="authSeparador">{t('auth.o')}</div>
            const { t } = useT();

      {errorMessage && <div className="authError">{errorMessage}</div>}

      <form className="authFormulario" onSubmit={manejarSubmit}>
        <CampoTexto
          etiqueta={t('auth.emailOUsuario')}
          type="text"
          placeholder="tu@email.com"
                await iniciarSesion(email.trim(), password);
          onChange={(event) => setEmail(event.target.value)}
          autoComplete="username"
          required
        />

        <CampoTexto
          etiqueta={t('auth.contrasena')}
          type="password"
                <h2 className="authTitulo">{t('auth.login.titulo')}</h2>

        <BotonBase type="submit" variante="primario" anchoCompleto cargando={cargando}>
          {t('auth.iniciarSesion')}
        </BotonBase>
      </form>

      <p className="authFooter">
        {t('auth.noTienesCuenta')}{' '}
        <BotonBase variante="ghost" type="button" className="authEnlace" onClick={onCambiar}>
          {t('auth.registrateGratis')}
                    {t('auth.continuarConGoogle')}
      </p>
    </div>
  );
}

                <div className="authSeparador">{t('auth.o')}</div>
  const { t } = useT();
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const { cargando, errorMessage, registrar, googleBotonRef, esGoogleNativo, loginGoogleNativo } = useAuth();
                    etiqueta={t('auth.emailOUsuario')}

  const manejarSubmit = async (event: FormEvent<HTMLFormElement>) => {
                    value={email}
                    onChange={(event) => setEmail(event.target.value)}
      await registrar({
        nombreVisible: username.trim(),
        username: username.trim(),
        email: email.trim(),
        password,
                    etiqueta={t('auth.contrasena')}
      cerrar();
                    placeholder={t('auth.contrasenaPlaceholder')}
      /* El feedback lo maneja useAuth vía errorMessage. */
    }
  };

  return (
    <div className="authFormContenedor">
      <h2 className="authTitulo">{t('auth.registro.titulo')}</h2>
                    {t('auth.iniciarSesion')}
      {esGoogleNativo ? (
        <BotonBase
          variante="secundario"
          anchoCompleto
                  {t('auth.noTienesCuenta')}{' '}
          cargando={cargando}
                    {t('auth.registrateGratis')}
          type="button"
        >
          <IconoGoogle />
          {t('auth.continuarConGoogle')}
        </BotonBase>
      ) : (
        <div ref={googleBotonRef} className="authGoogleBtnContenedor" />
            const { t } = useT();
      )}

      <div className="authSeparador">{t('auth.o')}</div>

      {errorMessage && <div className="authError">{errorMessage}</div>}

      <form className="authFormulario" onSubmit={manejarSubmit}>
        <CampoTexto
          etiqueta={t('config.nombreUsuario')}
          placeholder="tu_usuario"
          value={username}
          onChange={(event) => setUsername(event.target.value)}
          autoComplete="username"
          required
        />

        <CampoTexto
          etiqueta={t('auth.email')}
          type="email"
          placeholder="tu@email.com"
          value={email}
          onChange={(event) => setEmail(event.target.value)}
          autoComplete="email"
                <h2 className="authTitulo">{t('auth.registro.titulo')}</h2>
          placeholder={t('auth.contrasenaMin')}
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          autoComplete="new-password"
          required
        />

        <BotonBase type="submit" variante="primario" anchoCompleto cargando={cargando}>
          {t('auth.registro')}
        </BotonBase>
                    {t('auth.continuarConGoogle')}

      <p className="authFooter">
        {t('auth.yaTienesCuenta')}{' '}
        <BotonBase variante="ghost" type="button" className="authEnlace" onClick={onCambiar}>
          {t('auth.iniciarSesion')}
                <div className="authSeparador">{t('auth.o')}</div>
      </p>
    </div>
  );
}

                    etiqueta={t('config.nombreUsuario')}
  const { abierto, vista, cerrar, cambiarALogin, cambiarARegistro, puedesCerrar } = useModalAuth();

  const manejarEscape = useCallback((event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      cerrar();
    }
  }, [cerrar]);

                    etiqueta={t('auth.email')}
    if (!abierto) {
      return;
    }

    document.addEventListener('keydown', manejarEscape);
    cuerpoDocumento().style.overflow = 'hidden';

    return () => {
      document.removeEventListener('keydown', manejarEscape);
                    etiqueta={t('auth.contrasena')}
    };
                    placeholder={t('auth.contrasenaMin')}

  if (!abierto) {
    return null;
  }

  return createPortal(
    <div className={`authPantallaCompleta${puedesCerrar ? '' : ' authSoloFormulario'}`} role="dialog" aria-modal="true">
                    {t('auth.registro')}
        <aside className="authPanelImagen">
          <img src={authImage} alt="Kamples" className="authImagen" loading="lazy" />
        </aside>
      )}
                  {t('auth.yaTienesCuenta')}{' '}
        {puedesCerrar && (
                    {t('auth.iniciarSesion')}
            variante="ghost"
            className="authBtnCerrar"
            onClick={cerrar}
            aria-label="Cerrar"
            type="button"
          >
            <X size={20} />
          </BotonBase>
        )}
        {vista === 'login'
          ? <FormularioLogin onCambiar={cambiarARegistro} />
          : <FormularioRegistro onCambiar={cambiarALogin} />}
      </section>
    </div>,
    cuerpoDocumento(),
  );
}
