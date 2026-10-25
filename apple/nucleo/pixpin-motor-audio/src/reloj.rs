//! **El reloj de cada fuente**: que la linea de tiempo no se pare nunca.
//!
//! WASAPI fecha cada paquete con el contador de rendimiento del sistema
//! (QPC, en unidades de 100 ns): es el reloj comun a todas las tarjetas y el
//! mismo que usa el video. De ahi salen dos cosas:
//!
//! - **Los huecos.** El sonido del equipo (`AUDCLNT_STREAMFLAGS_LOOPBACK`)
//!   **no manda nada mientras no suena nada**: ni ceros, ni paquetes. Un
//!   aparato que se desenchufa y se vuelve a abrir deja otro hueco. Si no se
//!   rellenan, lo que viene despues se pega a lo de antes y esa fuente se
//!   adelanta para siempre. [`Relleno`] compara la fecha de cada paquete con
//!   donde tendria que empezar y mete el silencio que falta; y mientras no
//!   llega nada, lo va metiendo al paso del reloj.
//! - **No acumular.** Cada paquete se coloca contra **su propia fecha**, no
//!   sumando duraciones: una tarjeta cuyo cristal va un 0,02 % rapido daria,
//!   sumando, 0,7 s de mas en una hora y el relleno empezaria a meter
//!   silencios falsos. La diferencia lenta de relojes es cosa de
//!   [`crate::deriva`], no de esto.
//!
//! Puro: las fechas entran como numeros.

/// Unidades de 100 ns en un segundo.
pub const CIEN_NS_POR_SEGUNDO: u64 = 10_000_000;

/// Un paquete que empieza mas tarde que esto de donde acabo el anterior es
/// un hueco de verdad. 20 ms: por encima del temblor de las fechas (un par de
/// ms) y por debajo de lo que se nota. Es el del **sonido del equipo**, que
/// calla de verdad (no manda paquetes) y cuyo hueco hay que poner entero.
pub const UMBRAL_HUECO: u64 = 200_000;

/// El del **microfono**, que no calla nunca: entrega sin parar y un hueco
/// sin marca de discontinuidad casi siempre es temblor de las fechas (un
/// microfono USB o Bluetooth las da con 10-30 ms de baile). 70 ms, el
/// `TS_SMOOTHING_THRESHOLD` de OBS (`libobs/obs-source.c`): por debajo, se
/// pega a lo anterior como si fuera seguido. Lo que se equivoque aqui lo
/// absorbe la deriva, que mantiene el anillo en su nivel.
pub const UMBRAL_HUECO_MICROFONO: u64 = 700_000;

/// Con `AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY` (Windows tiro algo porque no
/// se leyo a tiempo) el hueco es seguro: se pone desde 1 ms.
pub const UMBRAL_DISCONTINUIDAD: u64 = 10_000;

/// **Un salto del reloj**: un hueco mayor que esto no es silencio sino que
/// el equipo se suspendio (la tapa del portatil) o el proceso estuvo
/// congelado. 2 s, el `MAX_TS_VAR` de OBS. No se rellena: se empieza de
/// nuevo desde ahi, como si hubiera sido una pausa (la mezcla hace lo mismo,
/// ver [`avance_de_mezcla`]), en vez de meter una hora de silencio en la nota.
pub const SALTO_MAXIMO: u64 = 20_000_000;

/// Sin paquetes durante esto, se empieza a poner silencio al paso del
/// reloj. 60 ms: mas que el periodo del aparato mas lento que se ve en la
/// practica (unos cascos Bluetooth entregan cada 20-30 ms).
pub const UMBRAL_ESPERA: u64 = 600_000;

/// Y se pone hasta esto antes de ahora: lo que puede estar aun de camino
/// dentro de la tarjeta (el primer paquete despues de un silencio llega con
/// unos 10-20 ms de retraso sobre su fecha).
pub const MARGEN_ESPERA: u64 = 300_000;

/// Marcos que dura `cien_ns` a `tasa` marcos por segundo (hacia abajo).
pub fn marcos_en(cien_ns: u64, tasa: u32) -> u64 {
    (u128::from(cien_ns) * u128::from(tasa) / u128::from(CIEN_NS_POR_SEGUNDO)) as u64
}

/// Lo que duran `marcos` marcos a `tasa`, en 100 ns (hacia abajo).
pub fn duracion_de(marcos: u64, tasa: u32) -> u64 {
    if tasa == 0 {
        return 0;
    }
    (u128::from(marcos) * u128::from(CIEN_NS_POR_SEGUNDO) / u128::from(tasa)) as u64
}

/// Lo que avanza el reloj de la mezcla en una vuelta de `dt` (100 ns):
/// `dt`, o nada si es un [salto](SALTO_MAXIMO) (el equipo se suspendio o el
/// proceso estuvo congelado). Con un salto, quien mezcla lo trata como una
/// pausa: tira lo que haya y vuelve a cebar, en vez de sacar de golpe la hora
/// que estuvo dormido.
pub fn avance_de_mezcla(dt: u64) -> Option<u64> {
    (dt <= SALTO_MAXIMO).then_some(dt)
}

/// **El relleno de silencio** de una fuente, a la tasa con la que esa fuente
/// escribe en su anillo.
#[derive(Debug, Clone)]
pub struct Relleno {
    tasa: u32,
    /// Cuando empezaria el siguiente paquete si no hubiera huecos: el final
    /// del ultimo paquete de verdad, por su fecha.
    fin: u64,
    /// Silencio ya puesto desde `fin` mientras no llegaba nada.
    puesto: u64,
    /// Silencio puesto de mas (el sonido llego antes de lo que se supuso):
    /// se le quita al principio de lo siguiente que llegue.
    exceso: u64,
    /// Desde cuanto un hueco entre paquetes es de verdad.
    umbral: u64,
    /// El siguiente paquete viene despues de una discontinuidad marcada.
    roto: bool,
    /// Saltos del reloj vistos (ver [`SALTO_MAXIMO`]).
    saltos: u64,
}

impl Relleno {
    /// Para una fuente arrancada en `inicio` (100 ns de QPC).
    pub fn nuevo(tasa: u32, inicio: u64) -> Relleno {
        Relleno::con_umbral(tasa, inicio, UMBRAL_HUECO)
    }

    /// Con otro umbral de hueco (ver [`UMBRAL_HUECO_MICROFONO`]).
    pub fn con_umbral(tasa: u32, inicio: u64, umbral: u64) -> Relleno {
        Relleno {
            tasa: tasa.max(1),
            fin: inicio,
            puesto: 0,
            exceso: 0,
            umbral,
            roto: false,
            saltos: 0,
        }
    }

    pub fn tasa(&self) -> u32 {
        self.tasa
    }

    /// Cuantos saltos del reloj se han visto.
    pub fn saltos(&self) -> u64 {
        self.saltos
    }

    /// Donde acaba lo ultimo que llego, por su fecha (100 ns).
    pub fn fin(&self) -> u64 {
        self.fin
    }

    /// Hasta donde llega lo escrito en el anillo, contando el silencio
    /// puesto al paso del reloj (100 ns): lo que se sella en el anillo.
    pub fn hasta(&self) -> u64 {
        self.fin + duracion_de(self.puesto, self.tasa)
    }

    /// El paquete siguiente viene marcado con discontinuidad (o es el
    /// primero de un aparato vuelto a abrir): su hueco se cree desde 1 ms.
    pub fn discontinuidad(&mut self) {
        self.roto = true;
    }

    /// Empieza de nuevo en `ahora`, olvidando lo puesto (un salto del reloj).
    fn saltar(&mut self, ahora: u64) {
        self.fin = ahora;
        self.puesto = 0;
        self.exceso = 0;
        self.saltos += 1;
    }

    /// Llega un paquete fechado en `inicio` que dura `duracion` (los dos en
    /// 100 ns). Devuelve los marcos de silencio que hay que escribir
    /// **antes** de el. Si se habia puesto silencio de mas, lo apunta para
    /// que [`Relleno::recortar`] se lo quite al paquete.
    pub fn paquete(&mut self, inicio: u64, duracion: u64) -> u64 {
        let hueco = inicio.saturating_sub(self.fin);
        let umbral = if std::mem::take(&mut self.roto) {
            UMBRAL_DISCONTINUIDAD
        } else {
            self.umbral
        };
        if hueco > SALTO_MAXIMO + duracion_de(self.puesto, self.tasa) {
            // Ni el silencio puesto al paso del reloj lo cubria: el hilo no
            // corrio (suspension). Se sigue desde aqui.
            self.saltar(inicio);
            self.fin = inicio + duracion;
            return 0;
        }
        let hace_falta = if hueco > umbral {
            marcos_en(hueco, self.tasa)
        } else {
            0
        };
        let silencio = if hace_falta >= self.puesto {
            hace_falta - self.puesto
        } else {
            self.exceso += self.puesto - hace_falta;
            0
        };
        self.puesto = 0;
        // Contra la fecha del paquete, no sumando: ver el comentario del
        // modulo. `max` por si un paquete viene fechado antes que el
        // anterior (pasa al cambiar de aparato): el reloj no va hacia atras.
        self.fin = self.fin.max(inicio + duracion);
        silencio
    }

    /// De los `marcos` de un paquete que se va a escribir, cuantos hay que
    /// saltarse del principio porque ya se puso silencio en su lugar.
    pub fn recortar(&mut self, marcos: u64) -> u64 {
        let n = self.exceso.min(marcos);
        self.exceso -= n;
        n
    }

    /// Sin paquete nuevo, a las `ahora`: los marcos de silencio que hay que
    /// escribir ya para que la fuente no se quede atras.
    pub fn espera(&mut self, ahora: u64) -> u64 {
        let atraso = ahora.saturating_sub(self.fin);
        if atraso <= UMBRAL_ESPERA {
            return 0;
        }
        if atraso > SALTO_MAXIMO + duracion_de(self.puesto, self.tasa) + MARGEN_ESPERA {
            // Llevaba mas de dos segundos sin poner nada, y se pone cada
            // 10 ms: este hilo no corrio. No es silencio que rellenar.
            self.saltar(ahora);
            return 0;
        }
        let hasta = marcos_en(atraso - MARGEN_ESPERA, self.tasa);
        let n = hasta.saturating_sub(self.puesto);
        self.puesto += n;
        n
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const FS: u32 = 48_000;
    const MS: u64 = 10_000;

    #[test]
    fn las_cuentas_de_tiempo_y_marcos_son_inversas() {
        assert_eq!(marcos_en(CIEN_NS_POR_SEGUNDO, FS), 48_000);
        assert_eq!(duracion_de(480, FS), 10 * MS);
        assert_eq!(marcos_en(duracion_de(441, 44_100), 44_100), 441);
        assert_eq!(duracion_de(5, 0), 0);
    }

    #[test]
    fn paquetes_seguidos_no_meten_nada() {
        let mut r = Relleno::nuevo(FS, 0);
        let mut t = 2 * MS;
        for i in 0..1000 {
            // Fechas con temblor de +-1 ms.
            let temblor = if i % 2 == 0 { MS } else { 0 };
            assert_eq!(r.paquete(t + temblor, 10 * MS), 0);
            assert_eq!(r.espera(t + 15 * MS), 0);
            t += 10 * MS;
        }
    }

    #[test]
    fn un_hueco_entre_paquetes_se_rellena_entero() {
        let mut r = Relleno::nuevo(FS, 0);
        assert_eq!(r.paquete(0, 10 * MS), 0);
        // El siguiente llega fechado 500 ms despues de donde acabo este.
        assert_eq!(r.paquete(510 * MS, 10 * MS), 24_000);
        assert_eq!(r.recortar(480), 0);
    }

    #[test]
    fn el_sonido_del_equipo_callado_avanza_al_paso_del_reloj() {
        // Ni un paquete en dos segundos: a cada vuelta se pone lo que toca,
        // y al final hay dos segundos de silencio menos el margen.
        let mut r = Relleno::nuevo(FS, 0);
        let mut puesto = 0;
        let mut t = 0;
        while t <= 2_000 * MS {
            puesto += r.espera(t);
            t += 10 * MS;
        }
        let esperado = marcos_en(2_000 * MS - MARGEN_ESPERA, FS);
        assert_eq!(puesto, esperado);
        // Vuelve a sonar justo donde tocaba: se completa lo que faltaba y no
        // se recorta nada.
        let falta = r.paquete(2_000 * MS, 10 * MS);
        assert_eq!(puesto + falta, 96_000);
        assert_eq!(r.recortar(480), 0);
    }

    #[test]
    fn si_se_puso_silencio_de_mas_se_recorta_de_lo_que_llega() {
        let mut r = Relleno::nuevo(FS, 0);
        r.paquete(0, 10 * MS);
        // Un aparato que se retrasa 100 ms sin hueco de verdad.
        let puesto = r.espera(110 * MS);
        assert!(puesto > 0);
        // Llega fechado justo donde tocaba: todo lo puesto sobra.
        assert_eq!(r.paquete(10 * MS, 10 * MS), 0);
        assert_eq!(r.recortar(100), 100);
        assert_eq!(r.recortar(10_000), puesto - 100);
        assert_eq!(r.recortar(10_000), 0);
    }

    #[test]
    fn una_tarjeta_con_el_reloj_rapido_no_acumula_silencios_falsos() {
        // Un cristal un 0,1 % rapido durante una hora: las fechas dicen que
        // cada paquete de 480 marcos dura 9,99 ms de pared.
        let mut r = Relleno::nuevo(FS, 0);
        let mut t = 0u64;
        let mut silencio = 0;
        for _ in 0..360_000 {
            silencio += r.paquete(t, 10 * MS);
            t += 10 * MS - 10;
        }
        assert_eq!(silencio, 0);
        // Y uno lento tampoco.
        let mut r = Relleno::nuevo(FS, 0);
        let mut t = 0u64;
        for _ in 0..360_000 {
            silencio += r.paquete(t, 10 * MS);
            t += 10 * MS + 10;
        }
        assert_eq!(silencio, 0);
    }

    #[test]
    fn el_microfono_aguanta_el_baile_de_las_fechas_y_el_equipo_no() {
        // Un paquete que llega fechado 40 ms tarde sin marca: en el
        // microfono es temblor (se pega); en el equipo, un hueco de verdad.
        let mut m = Relleno::con_umbral(FS, 0, UMBRAL_HUECO_MICROFONO);
        let mut e = Relleno::nuevo(FS, 0);
        m.paquete(0, 10 * MS);
        e.paquete(0, 10 * MS);
        assert_eq!(m.paquete(50 * MS, 10 * MS), 0);
        assert_eq!(e.paquete(50 * MS, 10 * MS), 1_920);
        // Con la marca de discontinuidad, el microfono tambien lo pone.
        m.discontinuidad();
        assert_eq!(m.paquete(100 * MS, 10 * MS), 1_920);
        // Y la marca vale para un solo paquete.
        assert_eq!(m.paquete(150 * MS, 10 * MS), 0);
    }

    #[test]
    fn una_suspension_no_mete_una_hora_de_silencio() {
        let mut r = Relleno::nuevo(FS, 0);
        let mut t = 0;
        let mut puesto = 0;
        // Un segundo normal, esperando cada 10 ms.
        while t < 1_000 * MS {
            r.paquete(t, 10 * MS);
            t += 10 * MS;
            puesto += r.espera(t);
        }
        assert_eq!(puesto, 0);
        // El equipo duerme una hora: ni una vuelta del hilo.
        t += 3_600 * 1_000 * MS;
        assert_eq!(r.espera(t), 0, "no se rellena la suspension");
        assert_eq!(r.saltos(), 1);
        // Al volver, lo que llega cae seguido, sin hueco.
        assert_eq!(r.paquete(t, 10 * MS), 0);
        assert_eq!(r.espera(t + 15 * MS), 0);
        // Un paquete fechado una hora despues sin que el hilo esperase
        // (la otra forma de verlo) tambien es un salto.
        let mut r = Relleno::nuevo(FS, 0);
        r.paquete(0, 10 * MS);
        assert_eq!(r.paquete(3_600 * 1_000 * MS, 10 * MS), 0);
        assert_eq!(r.saltos(), 1);
    }

    #[test]
    fn un_silencio_largo_del_equipo_no_es_un_salto() {
        // Diez segundos callado con el hilo dando vueltas: se rellena todo,
        // sin confundirlo con una suspension.
        let mut r = Relleno::nuevo(FS, 0);
        r.paquete(0, 10 * MS);
        let mut puesto = 0;
        let mut t = 10 * MS;
        while t < 10_000 * MS {
            t += 10 * MS;
            puesto += r.espera(t);
        }
        puesto += r.paquete(t, 10 * MS);
        assert_eq!(r.saltos(), 0);
        assert_eq!(puesto, marcos_en(t - 10 * MS, FS));
    }

    #[test]
    fn la_mezcla_no_cuenta_los_saltos() {
        assert_eq!(avance_de_mezcla(156_000), Some(156_000));
        assert_eq!(avance_de_mezcla(SALTO_MAXIMO), Some(SALTO_MAXIMO));
        assert_eq!(avance_de_mezcla(SALTO_MAXIMO + 1), None);
    }
}
