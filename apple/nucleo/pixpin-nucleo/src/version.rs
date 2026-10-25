//! **Negociar la version del protocolo** (fase 0.3 del plan, §4.2.4):
//! preparado, **apagado** y sin tocar el cable.
//!
//! Hoy el `hola` lleva `version: 4` y Windows y Android exigen que sea
//! **exactamente** 4 (`hola.version == VERSION`); si no, cortan con «el otro
//! aparato tiene otra version». Con tres plataformas que se actualizan a
//! ritmos distintos eso obliga a actualizar todos a la vez. El diseno
//! (detalle en `apple/docs/sincro.md`):
//!
//! 1. El `hola` sigue llevando `version` = **la mas alta que este aparato
//!    sabe hablar y que los de antes aceptan**: mientras haya aparatos de
//!    antes, 4. Asi un Windows o un Android de hoy no nota nada.
//! 2. Se anaden dos campos nuevos y opcionales, `versionMin` y `versionMax`
//!    (`skip_serializing_if` cuando son iguales a `version`, y
//!    `ignoreUnknownKeys` en Android / `serde` sin `deny_unknown_fields` en
//!    Rust: los de antes los ignoran sin mas).
//! 3. Quien los entiende elige **la mayor comun**
//!    ([`version_comun`]); quien no los trae habla solo `version`.
//! 4. Quien responde contesta en su `hola` la elegida en `version` (y su
//!    rango). Desde ahi, los dos hablan la elegida.
//! 5. Para subir a 5 sin romper a nadie: los aparatos nuevos saben 4 y 5;
//!    con uno de antes eligen 4; entre nuevos, 5. Solo cuando ya no quede
//!    ninguno de antes se sube `version` a 5 en el cable.
//!
//! Esto **no esta encendido** ([`NEGOCIAR`] es `false`): el `hola` del Mac
//! es el de `pixpin_sincro` tal cual. Encenderlo exige cambiar a la vez
//! `pixpin_sincro::mensajes::Hola` aqui y en `crates/`, y `Hola` de Android.

use pixpin_sincro::VERSION;

/// Si el Mac negocia la version. **Apagado**: no cambia nada del cable.
pub const NEGOCIAR: bool = false;

/// La version mas baja que este aparato sabe hablar.
pub const VERSION_MINIMA: u32 = VERSION;
/// La mas alta.
pub const VERSION_MAXIMA: u32 = VERSION;

/// El rango que anuncia un aparato: `(min, max)`. Uno de antes (sin
/// `versionMin`/`versionMax`) solo sabe la suya: `(v, v)`.
pub fn rango(version: u32, min: Option<u32>, max: Option<u32>) -> (u32, u32) {
    let min = min.unwrap_or(version).min(version);
    let max = max.unwrap_or(version).max(version);
    (min, max)
}

/// **La mayor version comun** de dos rangos, o `None` si no se solapan
/// (entonces no se habla: «actualiza los dos», como hoy).
pub fn version_comun(mio: (u32, u32), suyo: (u32, u32)) -> Option<u32> {
    let alta = mio.1.min(suyo.1);
    let baja = mio.0.max(suyo.0);
    (alta >= baja).then_some(alta)
}

/// La version con la que se habla con quien saludo con `version` (y quiza
/// su rango). Con [`NEGOCIAR`] apagado es la regla de hoy: la misma o nada.
pub fn elegir(version: u32, min: Option<u32>, max: Option<u32>) -> Option<u32> {
    if !NEGOCIAR {
        return (version == VERSION).then_some(VERSION);
    }
    version_comun((VERSION_MINIMA, VERSION_MAXIMA), rango(version, min, max))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn apagado_es_la_regla_de_hoy() {
        const { assert!(!NEGOCIAR) };
        assert_eq!(elegir(VERSION, None, None), Some(VERSION));
        // Un aparato de otra version no habla, aunque traiga un rango.
        assert_eq!(elegir(VERSION + 1, Some(VERSION), Some(VERSION + 1)), None);
        assert_eq!(elegir(VERSION - 1, None, None), None);
    }

    #[test]
    fn se_elige_la_mayor_comun() {
        // Uno nuevo (4..=5) con uno de antes (solo 4): 4.
        assert_eq!(version_comun((4, 5), rango(4, None, None)), Some(4));
        // Dos nuevos: 5.
        assert_eq!(version_comun((4, 5), rango(4, Some(4), Some(5))), Some(5));
        // Uno que ya no sabe 4 (5..=6) con uno nuevo: 5.
        assert_eq!(version_comun((4, 5), (5, 6)), Some(5));
        // Caso negativo: sin solape no se habla.
        assert_eq!(version_comun((4, 4), (5, 6)), None);
        // Un rango mal escrito (min > version) no deja fuera la suya.
        assert_eq!(rango(4, Some(9), None), (4, 4));
    }
}
