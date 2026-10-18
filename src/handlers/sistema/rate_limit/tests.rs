/* [16A-8] Pruebas del limitador. Ninguna duerme: el paso del tiempo se
 * simula envejeciendo el cubo a mano, que es determinista y no depende de la
 * carga de la máquina. */

use super::*;
use axum::http::HeaderValue;
use std::net::Ipv4Addr;

/* Un instante del pasado. `checked_sub` y no `-` porque restar de más
 * desborda, y aquí el valor lo elige cada prueba. */
fn hace(segundos: f64) -> Instant {
    Instant::now()
        .checked_sub(Duration::from_secs_f64(segundos))
        .expect("el instante pedido cabe en un Instant")
}

fn cabeceras(pares: &[(&'static str, &'static str)]) -> HeaderMap {
    let mut mapa = HeaderMap::new();
    for (nombre, valor) in pares {
        mapa.insert(*nombre, HeaderValue::from_static(valor));
    }
    mapa
}

/* Retrasa el reloj del cubo `segundos` para simular que la clave lleva ese
 * tiempo sin gastar. */
fn envejecer(limitador: &Limitador, clave: &str, segundos: f64) {
    let mut cubos = limitador.cubos.lock().unwrap();
    let cubo = cubos.get_mut(clave).expect("el cubo deberia existir");
    cubo.ultima = hace(segundos);
}

#[test]
fn la_rafaga_se_agota_y_devuelve_la_espera() {
    let limitador = Limitador::nuevo(1.0, 3.0);

    for intento in 0..3 {
        assert!(limitador.consumir("1.2.3.4").is_ok(), "falla el intento {intento}");
    }

    let espera = limitador
        .consumir("1.2.3.4")
        .expect_err("la cuarta peticion deberia pasarse del tope");
    assert!(espera >= 1, "la espera se redondea a segundos enteros: {espera}");
}

#[test]
fn el_cubo_se_recarga_con_el_tiempo() {
    let limitador = Limitador::nuevo(2.0, 1.0);

    assert!(limitador.consumir("ip").is_ok());
    assert!(limitador.consumir("ip").is_err(), "la rafaga es de una ficha");

    envejecer(&limitador, "ip", 1.0);
    assert!(limitador.consumir("ip").is_ok(), "1 s a 2 rps son 2 fichas");
}

#[test]
fn cada_ip_tiene_su_propio_cubo() {
    let limitador = Limitador::nuevo(1.0, 1.0);

    assert!(limitador.consumir("a").is_ok());
    assert!(limitador.consumir("a").is_err());
    assert!(limitador.consumir("b").is_ok(), "otra IP no hereda el gasto");
}

#[test]
fn la_rafaga_nunca_queda_por_debajo_de_una_ficha() {
    /* Una ráfaga de 0 dejaría el cubo permanentemente vacío: toda petición
     * mutante respondería 429. El constructor la sube a 1. */
    let limitador = Limitador::nuevo(1.0, 0.0);
    assert!(limitador.consumir("ip").is_ok());
}

#[test]
fn solo_los_metodos_mutantes_se_limitan() {
    for metodo in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
        assert!(es_mutante(&metodo), "{metodo} deberia limitarse");
    }
    for metodo in [Method::GET, Method::HEAD, Method::OPTIONS] {
        assert!(!es_mutante(&metodo), "{metodo} no deberia limitarse");
    }
}

#[test]
fn la_ip_sale_de_la_cabecera_del_proxy() {
    let peer: IpAddr = "10.0.0.9".parse().unwrap();

    let mapa = cabeceras(&[
        ("x-forwarded-for", "1.2.3.4, 10.0.0.1"),
        ("x-real-ip", "5.6.7.8"),
    ]);
    assert_eq!(
        ip_cliente(&mapa, peer),
        "5.6.7.8",
        "x-real-ip trae una sola IP y gana a la cadena"
    );

    let mapa = cabeceras(&[("x-forwarded-for", "1.2.3.4, 10.0.0.1")]);
    assert_eq!(
        ip_cliente(&mapa, peer),
        "1.2.3.4",
        "de la cadena se toma la primera entrada"
    );
}

#[test]
fn sin_cabecera_util_cae_al_peer() {
    let peer: IpAddr = "10.0.0.9".parse().unwrap();

    assert_eq!(ip_cliente(&HeaderMap::new(), peer), "10.0.0.9");

    let mapa = cabeceras(&[("x-forwarded-for", "no-es-una-ip")]);
    assert_eq!(
        ip_cliente(&mapa, peer),
        "10.0.0.9",
        "una cabecera que no es una IP se ignora"
    );
}

#[test]
fn sin_connect_info_el_peer_no_revienta() {
    /* Un router al que se llama sin `into_make_service_with_connect_info`
     * (pruebas con `oneshot`) no trae el peer. Debe caer a la clave fija, no
     * a un 500 por extractor que falla. */
    assert_eq!(peer_de(&Extensions::new()), IpAddr::V4(Ipv4Addr::UNSPECIFIED));

    let mut extensiones = Extensions::new();
    extensiones.insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 9], 443))));
    assert_eq!(peer_de(&extensiones), "10.0.0.9".parse::<IpAddr>().unwrap());
}

#[test]
fn la_ip_ipv4_mapeada_no_cuenta_como_dos_clientes() {
    let peer: IpAddr = "10.0.0.9".parse().unwrap();
    let mapa = cabeceras(&[("x-real-ip", "::ffff:1.2.3.4")]);
    assert_eq!(ip_cliente(&mapa, peer), "1.2.3.4");
}

/* Coloca un cubo con un estado concreto sin pasar por `consumir`. */
fn sembrar(limitador: &Limitador, clave: &str, fichas: f64, hace_segundos: f64) {
    let mut cubos = limitador.cubos.lock().unwrap();
    cubos.insert(
        clave.to_string(),
        Cubo {
            fichas,
            ultima: hace(hace_segundos),
        },
    );
}

#[test]
fn la_poda_solo_corre_cuando_toca() {
    let limitador = Limitador::nuevo(1.0, 2.0);
    sembrar(&limitador, "parado", 2.0, 30.0);

    /* Recién creado el limitador, la poda todavía no toca: el cubo sobrevive
     * aunque cumpla las condiciones para caer. */
    let mut cubos = limitador.cubos.lock().unwrap();
    limitador.podar(&mut cubos);
    assert!(cubos.contains_key("parado"), "la poda se adelantó");
}

#[test]
fn la_poda_tira_los_cubos_llenos_y_quietos() {
    let limitador = Limitador::nuevo(1.0, 2.0);
    sembrar(&limitador, "a-medio-gastar", 1.0, 30.0);
    sembrar(&limitador, "parado", 2.0, 30.0);

    {
        let mut ultima = limitador.ultima_poda.lock().unwrap();
        *ultima = hace(INTERVALO_PODA.as_secs_f64());
    }

    let mut cubos = limitador.cubos.lock().unwrap();
    limitador.podar(&mut cubos);
    assert!(
        cubos.contains_key("a-medio-gastar"),
        "un cubo con fichas gastadas sigue vivo: al cliente le quedan"
    );
    assert!(
        !cubos.contains_key("parado"),
        "un cubo lleno y quieto 30 s se descarta"
    );
}
