import { useCallback, useEffect, useRef, useState } from 'react';
import { cabezaDocumento, crearElemento, consultarElemento } from '../glory-core/platform/documento';

const GSI_SCRIPT_SRC = 'https://accounts.google.com/gsi/client';

interface GoogleCredentialResponse {
  credential: string;
}

interface GoogleAccounts {
  id: {
    initialize: (config: {
      client_id: string;
      callback: (response: GoogleCredentialResponse) => void;
      auto_select?: boolean;
      cancel_on_tap_outside?: boolean;
    }) => void;
    prompt: (notification?: (notification: { isNotDisplayed: () => boolean }) => void) => void;
    renderButton: (parent: HTMLElement, options: {
      type?: 'standard' | 'icon';
      theme?: 'outline' | 'filled_blue' | 'filled_black';
      size?: 'large' | 'medium' | 'small';
      text?: 'signin_with' | 'signup_with' | 'continue_with';
      logo_alignment?: 'left' | 'center';
      width?: number;
    }) => void;
  };
}

declare global {
  interface Window {
    GLORY_CONTEXT?: {
      googleClientId?: string;
    };
    google?: {
      accounts: GoogleAccounts;
    };
  }
}

function obtenerClientId(): string | null {
  return window.GLORY_CONTEXT?.googleClientId ?? null;
}

export function useGoogleAuth(onCredential: (credential: string) => void, skip = false) {
  const initializedRef = useRef(false);
  const callbackRef = useRef(onCredential);
  const [isReady, setIsReady] = useState(false);

  callbackRef.current = onCredential;

  useEffect(() => {
    if (skip) {
      return;
    }

    const clientId = obtenerClientId();
    if (!clientId || initializedRef.current) {
      return;
    }

    const initializeGoogle = () => {
      if (!window.google?.accounts?.id) {
        return;
      }

      window.google.accounts.id.initialize({
        client_id: clientId,
        callback: (response: GoogleCredentialResponse) => {
          callbackRef.current(response.credential);
        },
        auto_select: false,
        cancel_on_tap_outside: true,
      });

      initializedRef.current = true;
      setIsReady(true);
    };

    if (window.google?.accounts?.id) {
      initializeGoogle();
      return;
    }

    const existingScript = consultarElemento(`script[src="${GSI_SCRIPT_SRC}"]`);
    if (existingScript) {
      existingScript.addEventListener('load', initializeGoogle);
      return () => {
        existingScript.removeEventListener('load', initializeGoogle);
      };
    }

    const script = crearElemento('script');
    script.src = GSI_SCRIPT_SRC;
    script.async = true;
    script.defer = true;
    script.onload = initializeGoogle;
    cabezaDocumento().appendChild(script);

    return () => {
      script.onload = null;
    };
  }, [skip]);

  const buttonContainerRef = useCallback((node: HTMLDivElement | null) => {
    if (skip || !node || !isReady || !window.google?.accounts?.id) {
      return;
    }

    node.innerHTML = '';
    window.google.accounts.id.renderButton(node, {
      type: 'standard',
      theme: 'outline',
      size: 'large',
      text: 'continue_with',
      logo_alignment: 'left',
      width: 300,
    });
  }, [isReady, skip]);

  const triggerGoogle = useCallback(() => {
    if (skip || !window.google?.accounts?.id) {
      return;
    }

    window.google.accounts.id.prompt();
  }, [skip]);

  return {
    buttonContainerRef,
    triggerGoogle,
  };
}
