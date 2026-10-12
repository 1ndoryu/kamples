# 174A-75 - Web Push VAPID

## Alcance
- Se porto el canal Web Push base con VAPID para navegador, incluyendo registro, baja y envio directo a suscripciones activas del usuario.
- El backend expone los endpoints legacy `GET /api/push/vapid-key`, `POST /api/push/subscribe` y `POST /api/push/unsubscribe`.
- Se agrego un runtime opcional que solo se habilita cuando existe `VAPID_PRIVATE_KEY` o su alias legacy `KAMPLES_VAPID_PRIVATE_KEY`.

## Contrato HTTP
- `GET /api/push/vapid-key`
  - Respuesta: `{ habilitado, vapidKey? }`
  - No requiere autenticacion.
- `POST /api/push/subscribe`
  - Request: `{ endpoint, keys: { p256dh, auth }, plataforma }`
  - Requiere bearer auth.
- `POST /api/push/unsubscribe`
  - Request: `{ endpoint }`
  - Requiere bearer auth.

## Reglas de negocio portadas
- El registro conserva el upsert por `endpoint` del legado y reactiva la suscripcion si ya existia.
- La baja exige `usuario_id + endpoint` para evitar que un usuario desregistre la suscripcion de otra cuenta.
- El envio solo usa endpoints activos y marca como inactivos los expirados o invalidados por el proveedor push.
- La clave publica configurada, si existe, debe coincidir con la derivada desde la privada VAPID.

## Implementacion
- `src/repositories/push.rs`
  - upsert, delete scoped por usuario, listado activo y desactivacion por endpoint.
- `src/services/push.rs`
  - validacion de endpoint y llaves
  - runtime VAPID con derivacion de clave publica
  - envio via `web-push-native` (cifrado aes128gcm, RFC 8291) con firma VAPID ES256 propia (`p256`, RFC 8292) y cliente `reqwest`
  - desactivacion automatica cuando el proveedor devuelve endpoint expirado
- `src/handlers/push.rs`
  - endpoints HTTP y DTOs OpenAPI
- `src/config/mod.rs`, `src/main.rs`, `src/lib.rs`
  - wiring del runtime opcional dentro de `AppState`

## Decision tecnica importante
- Se descarto el crate `web-push` porque arrastra `ece -> openssl-sys`, lo que bloqueo la compilacion local en Windows sin OpenSSL instalado.
- Se sustituyo por `atomic_web_push`, que mantiene primitives equivalentes para VAPID y cliente reqwest sin ese bloqueo.
- **Actualizacion 2026-10-12:** `atomic_web_push` arrastra `rsa` 0.9.10 (RUSTSEC-2023-0071, sin parche). Se retiro: `web-push-native` (sin feature `vapid`) cifra el payload y `src/services/push.rs` firma el JWT VAPID con `p256`. Las variantes con feature `vapid` tambien arrastran `rsa`, por eso no se usan.

## Diferido explicito
- Este corte deja el canal listo para enviar, pero todavia no conecta productores concretos al fanout comun. Esa orquestacion queda para `174A-78`.
- No se implementa service worker ni UI frontend en este corte; solo contrato backend y runtime del canal.

## Validacion ejecutada
- `cargo sqlx prepare`
- `cargo check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo run -- --emit-openapi openapi.json`
- `npm run codegen`
- `npm --prefix frontend run type-check`
- `npm run self-check -- -TareaId 174A-75`