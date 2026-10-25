//! **Lo que entrega cada tarjeta**, leido a mano del `WAVEFORMATEX`.
//!
//! El motor abre cada aparato con el formato que ya tiene puesto la mesa de
//! mezclas de Windows (`GetMixFormat`) y **no le pide que convierta nada**:
//! convertir lo hace mejor y mas barato este modulo, una sola vez, ya bajando
//! a los canales de la salida. Casi siempre llega coma flotante de 32 bits en
//! estereo, pero un microfono USB puede dar enteros de 16 o de 24 bits, unos
//! altavoces 5.1 dan seis canales, y la cabecera puede venir en su forma
//! corta (`WAVE_FORMAT_PCM`, `WAVE_FORMAT_IEEE_FLOAT`) o en la extensible
//! (`WAVE_FORMAT_EXTENSIBLE`, con su `SubFormat` y su mascara de canales).
//!
//! Se lee de **bytes** y no de la estructura de `windows` a proposito: asi se
//! prueba en Linux con cabeceras de verdad copiadas a mano, y una tarjeta
//! rara no se descubre en casa de un usuario.

/// `WAVE_FORMAT_PCM`: enteros con signo (o sin signo si son de 8 bits).
pub const ETIQUETA_PCM: u16 = 1;
/// `WAVE_FORMAT_IEEE_FLOAT`: coma flotante.
pub const ETIQUETA_FLOTANTE: u16 = 3;
/// `WAVE_FORMAT_EXTENSIBLE`: lo de verdad esta en el `SubFormat`.
pub const ETIQUETA_EXTENSIBLE: u16 = 0xFFFE;

/// Los 12 bytes finales comunes a los `KSDATAFORMAT_SUBTYPE_*` que vienen de
/// una etiqueta corta: `XXXXXXXX-0000-0010-8000-00AA00389B71`, con la
/// etiqueta en los cuatro primeros. En memoria (little endian) son estos.
const COLA_DEL_SUBTIPO: [u8; 12] = [
    0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71,
];

// Los altavoces de `dwChannelMask` (ksmedia.h), en el orden en que van
// intercalados los canales.
const ALTAVOZ_FL: u32 = 0x1;
const ALTAVOZ_FR: u32 = 0x2;
const ALTAVOZ_FC: u32 = 0x4;
const ALTAVOZ_LFE: u32 = 0x8;
const ALTAVOZ_BL: u32 = 0x10;
const ALTAVOZ_BR: u32 = 0x20;
const ALTAVOZ_FLC: u32 = 0x40;
const ALTAVOZ_FRC: u32 = 0x80;
const ALTAVOZ_BC: u32 = 0x100;
const ALTAVOZ_SL: u32 = 0x200;
const ALTAVOZ_SR: u32 = 0x400;
const ALTAVOZ_TC: u32 = 0x800;
const ALTAVOZ_TFL: u32 = 0x1000;
const ALTAVOZ_TFC: u32 = 0x2000;
const ALTAVOZ_TFR: u32 = 0x4000;
const ALTAVOZ_TBL: u32 = 0x8000;
const ALTAVOZ_TBC: u32 = 0x10000;
const ALTAVOZ_TBR: u32 = 0x20000;

/// -3 dB: lo que aporta a cada lado un altavoz que esta en medio.
const MEDIO: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Como viene cada muestra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Muestra {
    /// Coma flotante de 32 bits: lo normal en la mesa de Windows.
    F32,
    /// Coma flotante de 64 bits (rarisimo, pero la cabecera lo permite).
    F64,
    /// Sin signo de 8 bits, centrado en 128.
    U8,
    /// Con signo de 16 bits.
    I16,
    /// Con signo de 24 bits **empaquetados** en tres bytes.
    I24,
    /// Con signo de 32 bits. Si solo valen 24 (`wValidBitsPerSample`), van
    /// arrimados arriba y se leen igual: dividir entre 2^31 sirve para los
    /// dos casos.
    I32,
}

impl Muestra {
    /// Bytes que ocupa una muestra en el bloque.
    pub fn bytes(self) -> usize {
        match self {
            Muestra::U8 => 1,
            Muestra::I16 => 2,
            Muestra::I24 => 3,
            Muestra::F32 | Muestra::I32 => 4,
            Muestra::F64 => 8,
        }
    }
}

/// El formato de un aparato, ya entendido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Formato {
    pub muestra: Muestra,
    pub canales: u16,
    pub tasa: u32,
    /// Bytes por marco (`nBlockAlign`).
    pub alineado: u16,
    /// Que altavoz es cada canal. Si la cabecera no lo dice (forma corta) o
    /// dice algo que no cuadra con los canales, se pone el habitual.
    pub mascara: u32,
}

/// Por que no se entiende una cabecera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ErrorFormato {
    #[error("la cabecera del formato es mas corta de lo que dice")]
    Corta,
    #[error("formato de sonido desconocido (etiqueta {etiqueta:#06x}, {bits} bits)")]
    Desconocido { etiqueta: u16, bits: u16 },
    #[error("la cabecera dice cero canales o cero muestras por segundo")]
    Vacia,
    #[error("el tamano de bloque ({alineado}) no cuadra con {canales} canales de {bits} bits")]
    Desalineada {
        alineado: u16,
        canales: u16,
        bits: u16,
    },
}

fn u16_en(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_en(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// La mascara de siempre para `canales` canales, cuando la cabecera no la da.
pub fn mascara_habitual(canales: u16) -> u32 {
    match canales {
        1 => ALTAVOZ_FC,
        2 => ALTAVOZ_FL | ALTAVOZ_FR,
        3 => ALTAVOZ_FL | ALTAVOZ_FR | ALTAVOZ_FC,
        4 => ALTAVOZ_FL | ALTAVOZ_FR | ALTAVOZ_BL | ALTAVOZ_BR,
        6 => ALTAVOZ_FL | ALTAVOZ_FR | ALTAVOZ_FC | ALTAVOZ_LFE | ALTAVOZ_BL | ALTAVOZ_BR,
        8 => {
            ALTAVOZ_FL
                | ALTAVOZ_FR
                | ALTAVOZ_FC
                | ALTAVOZ_LFE
                | ALTAVOZ_BL
                | ALTAVOZ_BR
                | ALTAVOZ_SL
                | ALTAVOZ_SR
        }
        // Otra cosa: los primeros altavoces en orden, que es lo que hace
        // Windows con una mascara que falta.
        n => ((1u64 << n.min(18)) - 1) as u32,
    }
}

impl Formato {
    /// Lee un `WAVEFORMATEX` (o `WAVEFORMATEXTENSIBLE`) de sus bytes crudos:
    /// los 18 de la cabecera y los `cbSize` que la siguen.
    pub fn de_bytes(b: &[u8]) -> Result<Formato, ErrorFormato> {
        if b.len() < 16 {
            return Err(ErrorFormato::Corta);
        }
        let etiqueta = u16_en(b, 0);
        let canales = u16_en(b, 2);
        let tasa = u32_en(b, 4);
        let alineado = u16_en(b, 12);
        let bits = u16_en(b, 14);
        if canales == 0 || tasa == 0 {
            return Err(ErrorFormato::Vacia);
        }
        let desconocido = ErrorFormato::Desconocido { etiqueta, bits };

        let (subtipo, validos, mascara) = if etiqueta == ETIQUETA_EXTENSIBLE {
            // 18 de cabecera + 22 de extension: Samples, dwChannelMask y el
            // GUID del SubFormat.
            if b.len() < 40 || u16_en(b, 16) < 22 {
                return Err(ErrorFormato::Corta);
            }
            let validos = u16_en(b, 18);
            let mascara = u32_en(b, 20);
            if b[28..40] != COLA_DEL_SUBTIPO {
                return Err(desconocido);
            }
            // El primer campo del GUID es la etiqueta corta del subtipo.
            let sub = u32_en(b, 24);
            let sub = u16::try_from(sub).map_err(|_| desconocido)?;
            (sub, validos, mascara)
        } else {
            (etiqueta, bits, 0)
        };

        let muestra = match (subtipo, bits) {
            (ETIQUETA_FLOTANTE, 32) => Muestra::F32,
            (ETIQUETA_FLOTANTE, 64) => Muestra::F64,
            (ETIQUETA_PCM, 8) => Muestra::U8,
            (ETIQUETA_PCM, 16) => Muestra::I16,
            (ETIQUETA_PCM, 24) => Muestra::I24,
            (ETIQUETA_PCM, 32) => Muestra::I32,
            _ => return Err(desconocido),
        };
        // Los bits validos no pueden ser mas que el contenedor; cero es «no lo
        // dice» y vale.
        if validos > bits {
            return Err(desconocido);
        }
        if alineado as usize != canales as usize * muestra.bytes() {
            return Err(ErrorFormato::Desalineada {
                alineado,
                canales,
                bits,
            });
        }
        let mascara = if mascara.count_ones() == u32::from(canales) {
            mascara
        } else {
            mascara_habitual(canales)
        };
        Ok(Formato {
            muestra,
            canales,
            tasa,
            alineado,
            mascara,
        })
    }

    /// Bytes por marco.
    pub fn bytes_por_marco(&self) -> usize {
        self.alineado as usize
    }
}

/// Lo que aporta un altavoz a la izquierda y a la derecha al bajarlo a
/// estereo. Las proporciones de siempre (ITU-R BS.775): lo de en medio entra
/// a -3 dB en los dos lados, lo de atras y los lados entra a -3 dB en el
/// suyo, y el subgrave no entra (en unos altavoces de portatil no hay donde
/// ponerlo y solo restaria margen).
fn a_estereo(altavoz: u32) -> [f32; 2] {
    match altavoz {
        ALTAVOZ_FL | ALTAVOZ_FLC => [1.0, 0.0],
        ALTAVOZ_FR | ALTAVOZ_FRC => [0.0, 1.0],
        ALTAVOZ_FC | ALTAVOZ_TC | ALTAVOZ_TFC => [MEDIO, MEDIO],
        ALTAVOZ_BC | ALTAVOZ_TBC => [0.5, 0.5],
        ALTAVOZ_BL | ALTAVOZ_SL | ALTAVOZ_TFL => [MEDIO, 0.0],
        ALTAVOZ_BR | ALTAVOZ_SR | ALTAVOZ_TFR => [0.0, MEDIO],
        ALTAVOZ_TBL => [0.5, 0.0],
        ALTAVOZ_TBR => [0.0, 0.5],
        ALTAVOZ_LFE => [0.0, 0.0],
        // Un bit que no conocemos: a los dos lados, bajito, mejor que perderlo.
        _ => [0.5, 0.5],
    }
}

/// **Convierte** marcos crudos de un aparato a coma flotante, ya en los
/// canales de la salida (1 o 2).
///
/// La matriz se calcula una vez al abrir el aparato; convertir no reserva
/// memoria. Sin normalizar la suma: un 5.1 por el que solo suena musica en
/// estereo (lo normal en un PC) tiene que salir al mismo volumen que unos
/// cascos, y los picos que se pasen los dobla el limitador del final.
#[derive(Debug, Clone)]
pub struct Conversor {
    formato: Formato,
    salida: usize,
    /// `[izquierda, derecha]` de cada canal de entrada (o `[mono, _]`).
    matriz: Vec<[f32; 2]>,
}

impl Conversor {
    /// Para `formato` hacia `canales_salida` canales (1 o 2).
    pub fn nuevo(formato: Formato, canales_salida: u16) -> Conversor {
        let salida = canales_salida.clamp(1, 2) as usize;
        let n = formato.canales as usize;
        let mut matriz = Vec::with_capacity(n);
        if n == 1 {
            // Un microfono mono va entero a los dos lados (o al mono): bajarlo
            // 3 dB por ser «el centro» dejaria la voz baja sin motivo.
            matriz.push([1.0, 1.0]);
        } else {
            // Cada bit puesto de la mascara, de menor a mayor, es un canal.
            let mut restantes = formato.mascara;
            for _ in 0..n {
                let bit = if restantes == 0 {
                    0
                } else {
                    restantes & restantes.wrapping_neg()
                };
                restantes &= !bit;
                matriz.push(a_estereo(bit));
            }
        }
        if salida == 1 && n > 1 {
            // Mono: la media de los dos lados del estereo. Asi un microfono
            // que deja un canal callado no sale a la mitad de volumen... y
            // tampoco al doble si los dos traen lo mismo.
            for c in &mut matriz {
                let m = (c[0] + c[1]) * 0.5;
                *c = [m, m];
            }
        }
        Conversor {
            formato,
            salida,
            matriz,
        }
    }

    pub fn formato(&self) -> &Formato {
        &self.formato
    }

    pub fn canales_salida(&self) -> usize {
        self.salida
    }

    /// Lee una muestra cruda en `[-1, 1]`.
    #[inline]
    fn leer(&self, b: &[u8]) -> f32 {
        match self.formato.muestra {
            Muestra::F32 => {
                let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                // Un NaN o un infinito de un controlador roto no puede
                // envenenar la mezcla entera.
                if v.is_finite() { v } else { 0.0 }
            }
            Muestra::F64 => {
                let v = f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
                if v.is_finite() { v as f32 } else { 0.0 }
            }
            Muestra::U8 => (f32::from(b[0]) - 128.0) / 128.0,
            Muestra::I16 => f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0,
            Muestra::I24 => {
                // Se mete en los 24 bits altos de un i32 y se divide igual que
                // uno de 32: el signo sale solo.
                let v = i32::from_le_bytes([0, b[0], b[1], b[2]]);
                v as f32 / 2_147_483_648.0
            }
            Muestra::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
        }
    }

    /// Convierte los marcos enteros de `datos` y los deja intercalados en
    /// `salida`. Devuelve cuantos marcos escribio: los que quepan en los dos.
    pub fn convertir(&self, datos: &[u8], salida: &mut [f32]) -> usize {
        let por_marco = self.formato.bytes_por_marco();
        let n = (datos.len() / por_marco).min(salida.len() / self.salida);
        let ancho = self.formato.muestra.bytes();
        // El caso de casi siempre, sin matriz: estereo flotante a estereo.
        if self.formato.muestra == Muestra::F32
            && self.formato.canales == 2
            && self.salida == 2
            && self.matriz[0] == [1.0, 0.0]
            && self.matriz[1] == [0.0, 1.0]
        {
            for (marco, fuera) in datos[..n * 8]
                .chunks_exact(8)
                .zip(salida.chunks_exact_mut(2))
            {
                fuera[0] = self.leer(&marco[0..4]);
                fuera[1] = self.leer(&marco[4..8]);
            }
            return n;
        }
        for (marco, fuera) in datos[..n * por_marco]
            .chunks_exact(por_marco)
            .zip(salida.chunks_exact_mut(self.salida))
        {
            let (mut i, mut d) = (0.0f32, 0.0f32);
            for (c, coef) in self.matriz.iter().enumerate() {
                let v = self.leer(&marco[c * ancho..]);
                i += v * coef[0];
                d += v * coef[1];
            }
            fuera[0] = i;
            if self.salida == 2 {
                fuera[1] = d;
            }
        }
        n
    }
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;

    /// Una cabecera corta como la que daria Windows.
    pub(crate) fn corta(etiqueta: u16, canales: u16, tasa: u32, bits: u16) -> Vec<u8> {
        let alineado = canales * bits / 8;
        let mut b = Vec::new();
        b.extend(etiqueta.to_le_bytes());
        b.extend(canales.to_le_bytes());
        b.extend(tasa.to_le_bytes());
        b.extend((tasa * alineado as u32).to_le_bytes());
        b.extend(alineado.to_le_bytes());
        b.extend(bits.to_le_bytes());
        b.extend(0u16.to_le_bytes());
        b
    }

    /// Una `WAVEFORMATEXTENSIBLE`, como la que da `GetMixFormat` casi siempre.
    pub(crate) fn extensible(
        sub: u16,
        canales: u16,
        tasa: u32,
        bits: u16,
        validos: u16,
        mascara: u32,
    ) -> Vec<u8> {
        let mut b = corta(ETIQUETA_EXTENSIBLE, canales, tasa, bits);
        b[16..18].copy_from_slice(&22u16.to_le_bytes());
        b.extend(validos.to_le_bytes());
        b.extend(mascara.to_le_bytes());
        b.extend(u32::from(sub).to_le_bytes());
        b.extend(COLA_DEL_SUBTIPO);
        b
    }

    #[test]
    fn la_cabecera_de_siempre_es_flotante_estereo_a_48000() {
        let f = Formato::de_bytes(&extensible(3, 2, 48_000, 32, 32, 3)).unwrap();
        assert_eq!(f.muestra, Muestra::F32);
        assert_eq!(
            (f.canales, f.tasa, f.alineado, f.mascara),
            (2, 48_000, 8, 3)
        );
    }

    #[test]
    fn se_entienden_las_formas_cortas_y_los_enteros() {
        let f = Formato::de_bytes(&corta(1, 1, 44_100, 16)).unwrap();
        assert_eq!(f.muestra, Muestra::I16);
        assert_eq!(f.mascara, ALTAVOZ_FC, "mono sin mascara: el centro");
        let f = Formato::de_bytes(&corta(3, 2, 48_000, 32)).unwrap();
        assert_eq!(f.muestra, Muestra::F32);
        let f = Formato::de_bytes(&extensible(1, 2, 48_000, 24, 24, 3)).unwrap();
        assert_eq!(f.muestra, Muestra::I24);
        // 24 validos en un contenedor de 32: se lee como de 32.
        let f = Formato::de_bytes(&extensible(1, 2, 96_000, 32, 24, 3)).unwrap();
        assert_eq!(f.muestra, Muestra::I32);
    }

    #[test]
    fn lo_que_no_se_entiende_se_rechaza_con_su_motivo() {
        assert_eq!(Formato::de_bytes(&[0; 10]), Err(ErrorFormato::Corta));
        // A-law (etiqueta 6): no.
        assert!(matches!(
            Formato::de_bytes(&corta(6, 1, 8_000, 8)),
            Err(ErrorFormato::Desconocido { etiqueta: 6, .. })
        ));
        // Extensible con un SubFormat que no es de los nuestros.
        let mut raro = extensible(3, 2, 48_000, 32, 32, 3);
        raro[39] ^= 0xFF;
        assert!(matches!(
            Formato::de_bytes(&raro),
            Err(ErrorFormato::Desconocido { .. })
        ));
        // Extensible cortada.
        let e = extensible(3, 2, 48_000, 32, 32, 3);
        assert_eq!(Formato::de_bytes(&e[..30]), Err(ErrorFormato::Corta));
        // Bloque que no cuadra.
        let mut mal = corta(1, 2, 48_000, 16);
        mal[12..14].copy_from_slice(&3u16.to_le_bytes());
        assert!(matches!(
            Formato::de_bytes(&mal),
            Err(ErrorFormato::Desalineada { .. })
        ));
        assert_eq!(
            Formato::de_bytes(&corta(1, 0, 48_000, 16)),
            Err(ErrorFormato::Vacia)
        );
    }

    #[test]
    fn una_mascara_que_no_cuadra_con_los_canales_se_cambia_por_la_habitual() {
        // Seis canales pero la mascara dice dos: se supone un 5.1.
        let f = Formato::de_bytes(&extensible(3, 6, 48_000, 32, 32, 3)).unwrap();
        assert_eq!(f.mascara, 0x3F);
    }

    fn bytes_f32(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    #[test]
    fn estereo_a_mono_promedia_y_a_estereo_no_toca() {
        let f = Formato::de_bytes(&extensible(3, 2, 48_000, 32, 32, 3)).unwrap();
        let datos = bytes_f32(&[0.0, 1.0, 0.5, 0.5]);
        let mut mono = [0.0; 2];
        assert_eq!(Conversor::nuevo(f, 1).convertir(&datos, &mut mono), 2);
        assert_eq!(mono, [0.5, 0.5]);
        let mut est = [0.0; 4];
        Conversor::nuevo(f, 2).convertir(&datos, &mut est);
        assert_eq!(est, [0.0, 1.0, 0.5, 0.5]);
    }

    #[test]
    fn un_microfono_mono_va_entero_a_los_dos_lados() {
        let f = Formato::de_bytes(&corta(1, 1, 48_000, 16)).unwrap();
        let datos: Vec<u8> = [16_384i16, -32_768]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        let mut est = [0.0; 4];
        assert_eq!(Conversor::nuevo(f, 2).convertir(&datos, &mut est), 2);
        assert_eq!(est, [0.5, 0.5, -1.0, -1.0]);
    }

    #[test]
    fn cinco_punto_uno_baja_a_estereo_sin_el_subgrave() {
        let f = Formato::de_bytes(&extensible(3, 6, 48_000, 32, 32, 0x3F)).unwrap();
        // FL FR FC LFE BL BR
        let datos = bytes_f32(&[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        let mut est = [0.0; 2];
        Conversor::nuevo(f, 2).convertir(&datos, &mut est);
        assert_eq!(est, [1.0, 0.0], "el subgrave no entra");
        let datos = bytes_f32(&[0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
        Conversor::nuevo(f, 2).convertir(&datos, &mut est);
        assert!((est[0] - MEDIO).abs() < 1e-6 && (est[1] - MEDIO).abs() < 1e-6);
    }

    #[test]
    fn veinticuatro_bits_empaquetados_conservan_el_signo() {
        let f = Formato::de_bytes(&corta(1, 1, 48_000, 24)).unwrap();
        // -1 en 24 bits: FF FF FF. El maximo: FF FF 7F.
        let datos = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F];
        let mut m = [0.0; 2];
        Conversor::nuevo(f, 1).convertir(&datos, &mut m);
        assert!(m[0] < 0.0 && m[0] > -1e-6, "{}", m[0]);
        assert!((m[1] - 1.0).abs() < 1e-6, "{}", m[1]);
    }

    #[test]
    fn un_nan_del_controlador_sale_como_silencio() {
        let f = Formato::de_bytes(&corta(3, 1, 48_000, 32)).unwrap();
        let mut m = [1.0; 1];
        Conversor::nuevo(f, 1).convertir(&bytes_f32(&[f32::NAN]), &mut m);
        assert_eq!(m, [0.0]);
    }

    #[test]
    fn medio_marco_al_final_no_se_inventa() {
        let f = Formato::de_bytes(&corta(1, 2, 48_000, 16)).unwrap();
        let mut m = [9.0; 4];
        assert_eq!(
            Conversor::nuevo(f, 1).convertir(&[0, 0, 0, 0, 1, 1], &mut m),
            1
        );
        assert_eq!(m[1], 9.0, "no toca mas alla de lo convertido");
    }
}
