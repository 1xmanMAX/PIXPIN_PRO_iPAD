//! **La pagina que se exporta** (rehecha el 5-oct-2026, noche): el
//! timeline en un solo fichero HTML que se ve **como la ventana**, y que es
//! tambien lo que se imprime a PDF.
//!
//! El usuario: «a la hora de exportar, que se exporte con su audio, la
//! imagen, todo completo, de forma que se pueda ver tambien con una
//! interfaz de usuario muy similar y que se vea muy bien». Por eso:
//!
//! - **Todo va dentro**: fotos y audios en `data:` (el `.m4a` como
//!   `audio/mp4`). Se abre sin internet, en el PC o en el movil.
//! - Arriba, como la ventana, las pestanas **Linea · Momentos · Estado**. La
//!   linea de tiempo esta escrita en el HTML (es lo que sale al imprimir);
//!   Momentos y Estado los arma el JavaScript de la pagina con los mismos
//!   datos.
//! - Pulsar una tarjeta abre **la historia**, como en la ventana y como en
//!   Instagram: la foto llenando el fondo (o el degradado del momento,
//!   `estilo::degradado_de`), el texto en el medio, barritas si hay varias
//!   fotos, flechas y teclado, y la nota de voz con su boton. `#m=<id>` en
//!   la direccion abre la historia de ese momento.
//! - Cada foto y cada audio va **una sola vez** en el fichero: la linea de
//!   tiempo los lleva en sus `<img>` y `<audio>` con id, y el resto de la
//!   pagina los toma de ahi. Una foto no pesa el doble por salir en dos
//!   pestanas.
//! - Los datos para el JavaScript van como JSON dentro de un
//!   `<script type="application/json">`, escapado para que un titulo con
//!   comillas, `</script>` o saltos de linea no rompa nada.
//! - Al imprimir (el PDF de Edge) solo sale la linea de tiempo, en claro; el
//!   audio no suena en papel: sale su chapita con la duracion.

use std::collections::BTreeMap;

use serde_json::json;

use crate::Momento;
use crate::dias::{Dia, Mes, hora, solo_el_mes};
use crate::emoticonos::primer_emoticono;
use crate::estilo::{css, degradado_de, duracion, fecha_larga};
use crate::resumen::lo_mas_repetido;

/// Un trozo de la pagina con su rotulo: un dia, o «Ultimas 24 horas».
pub struct Seccion<'a> {
    pub rotulo: String,
    pub momentos: Vec<&'a Momento>,
}

pub struct Opciones {
    pub titulo: String,
    pub subtitulo: String,
    /// Lo que dice el pie («Exportado con PixPin Max · …»).
    pub pie: String,
    /// El rotulo de la chapa de un audio («Nota de voz»).
    pub nota_de_voz: String,
    pub desfase: i64,
    pub ingles: bool,
    /// Los rotulos de las pestanas: linea, momentos y estado.
    pub pestanas: [String; 3],
    /// «Sobre todo», en la pestana Estado.
    pub sobre_todo: String,
}

/// La pagina entera. `imagen` y `audio` dan el `data:` de un fichero por su
/// nombre, o `None` si ya no esta (se omite sin romper nada).
pub fn pagina(
    op: &Opciones,
    secciones: &[Seccion<'_>],
    imagen: &dyn Fn(&str) -> Option<String>,
    audio: &dyn Fn(&str) -> Option<String>,
) -> String {
    let mut s = String::with_capacity(64 * 1024);
    let lang = if op.ingles { "en" } else { "es" };
    s.push_str(&format!(
        "<!doctype html>\n<html lang=\"{lang}\"><head><meta charset=\"utf-8\">\n"
    ));
    s.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    s.push_str(&format!("<title>{}</title>\n<style>", esc(&op.titulo)));
    s.push_str(CSS);
    s.push_str("</style></head><body>\n");
    s.push_str(&format!(
        "<header class=\"barra\"><div class=\"tit\"><h1>{}</h1><p class=\"sub\">{}</p></div>\
         <nav class=\"tabs\"><button data-v=\"linea\" class=\"on\">{}</button>\
         <button data-v=\"momentos\">{}</button><button data-v=\"estado\">{}</button></nav></header>\n",
        esc(&op.titulo),
        esc(&op.subtitulo),
        esc(&op.pestanas[0]),
        esc(&op.pestanas[1]),
        esc(&op.pestanas[2]),
    ));
    s.push_str("<main>\n<div id=\"v-linea\" class=\"vista on\">\n");

    let mut datos = Vec::new();
    let mut audios = String::new();
    let mut n_foto = 0usize;
    let mut n_audio = 0usize;
    for sec in secciones {
        s.push_str(&format!(
            "<section><h2>{}</h2>\n<ol class=\"linea\">\n",
            esc(&sec.rotulo)
        ));
        for m in &sec.momentos {
            let id = id_html(&m.id);
            s.push_str(&format!(
                "<li class=\"momento\" data-id=\"{id}\"><time>{}</time><span class=\"punto\"></span>\
                 <div class=\"tarjeta\" tabindex=\"0\">",
                hora(m.cuando, op.desfase)
            ));
            s.push_str(&format!("<h3>{}</h3>", esc(&m.titulo)));
            if !m.descripcion.trim().is_empty() {
                s.push_str(&format!(
                    "<p>{}</p>",
                    esc(&m.descripcion).replace('\n', "<br>")
                ));
            }
            let mut fotos = Vec::new();
            for f in &m.fotos {
                if let Some(uri) = imagen(f) {
                    let fid = format!("f{n_foto}");
                    n_foto += 1;
                    if fotos.is_empty() {
                        s.push_str("<div class=\"fotos\">");
                    }
                    s.push_str(&format!("<img id=\"{fid}\" src=\"{uri}\" alt=\"\">"));
                    fotos.push(fid);
                }
            }
            if !fotos.is_empty() {
                s.push_str("</div>");
            }
            let mut aid = None;
            if let Some(a) = &m.audio {
                s.push_str(&format!(
                    "<span class=\"voz\">▶ {}{}</span>",
                    esc(&op.nota_de_voz),
                    con_punto(&duracion(m.duracion_ms))
                ));
                if let Some(uri) = audio(a) {
                    let x = format!("a{n_audio}");
                    n_audio += 1;
                    audios.push_str(&format!(
                        "<audio id=\"{x}\" preload=\"none\" src=\"{uri}\"></audio>\n"
                    ));
                    aid = Some(x);
                }
            }
            s.push_str("</div></li>\n");
            let (g1, g2) = degradado_de(&m.id);
            let d = Dia::de_instante(m.cuando, op.desfase);
            let emo = primer_emoticono(&m.titulo).or_else(|| primer_emoticono(&m.descripcion));
            datos.push(json!({
                "id": id,
                "dia": d.iso(),
                "hora": hora(m.cuando, op.desfase),
                "fecha": fecha_larga(m.cuando, op.desfase, op.ingles),
                "titulo": m.titulo,
                "texto": m.descripcion,
                "fotos": fotos,
                "audio": aid,
                "dur": duracion(m.duracion_ms),
                "g": [css(g1), css(g2)],
                "emo": emo,
                "leccion": m.leccion.is_some(),
            }));
        }
        s.push_str("</ol></section>\n");
    }
    s.push_str("</div>\n<div id=\"v-momentos\" class=\"vista\"></div>\n");
    s.push_str("<div id=\"v-estado\" class=\"vista\"></div>\n");
    s.push_str(&format!("<footer>{}</footer>\n</main>\n", esc(&op.pie)));
    s.push_str(HISTORIA);
    s.push_str("<div id=\"medios\" hidden>\n");
    s.push_str(&audios);
    s.push_str("</div>\n");

    // Lo que mas se repitio en cada mes, para «Sobre todo» de Estado.
    let mut por_mes: BTreeMap<Mes, Vec<&Momento>> = BTreeMap::new();
    for sec in secciones {
        for m in &sec.momentos {
            por_mes
                .entry(Mes::de(Dia::de_instante(m.cuando, op.desfase)))
                .or_default()
                .push(m);
        }
    }
    let sobre: serde_json::Map<String, serde_json::Value> = por_mes
        .iter()
        .filter_map(|(mes, v)| {
            lo_mas_repetido(v).map(|l| (format!("{:04}-{:02}", mes.anio, mes.mes), json!(l.texto())))
        })
        .collect();
    let meses: Vec<&str> = (1..=12).map(|n| solo_el_mes(n, op.ingles)).collect();
    let todo = json!({
        "momentos": datos,
        "meses": meses,
        "sobre": sobre,
        "rot": { "sobre": op.sobre_todo, "voz": op.nota_de_voz },
    });
    s.push_str("<script type=\"application/json\" id=\"datos\">");
    s.push_str(&json_en_html(&todo.to_string()));
    s.push_str("</script>\n<script>");
    s.push_str(JS);
    s.push_str("</script>\n</body></html>\n");
    s
}

/// « · 0:42», o nada.
fn con_punto(d: &str) -> String {
    if d.is_empty() {
        String::new()
    } else {
        format!(" · {d}")
    }
}

/// Un id que vale como atributo y en la direccion (`#m=`): solo letras,
/// numeros y guiones. Los de la app ya son asi (`tl-<ms>-<n>`).
fn id_html(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect()
}

/// JSON a salvo dentro de un `<script>`: ningun `<` (asi ni `</script>` ni
/// `<!--` cambian como lee el HTML la etiqueta) y U+2028/U+2029 escapados
/// aunque alguien lo lea como JavaScript. `<` es el mismo `<` para
/// `JSON.parse`.
pub fn json_en_html(j: &str) -> String {
    j.replace('<', "\\u003c")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// El `data:` de un fichero, con el tipo por su extension: fotos y audios.
pub fn data_uri(nombre: &str, bytes: &[u8]) -> String {
    let ext = nombre.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let tipo = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        // El dictado guarda AAC en MP4: los navegadores lo piden asi.
        "m4a" | "mp4" | "aac" => "audio/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "opus" => "audio/ogg",
        "webm" => "audio/webm",
        _ => "image/png",
    };
    format!("data:{tipo};base64,{}", base64(bytes))
}

/// Los bytes en base64 (el de siempre, con `=` de relleno). Lo usa tambien
/// la hoja de compartir para los adjuntos de su pagina web.
pub fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

/// Lo escrito, a salvo dentro del HTML.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
    }
    o
}

/// La historia: vacia, la rellena el JavaScript.
const HISTORIA: &str = r#"<div id="historia" class="historia" hidden>
<div class="h-fondo"></div><div class="h-velo"></div>
<div class="h-barras"></div>
<div class="h-arriba"><span class="h-fecha"></span><span class="h-cuenta"></span><button class="h-x" aria-label="Cerrar">&#x2715;</button></div>
<button class="h-izq" aria-label="Anterior"></button><button class="h-der" aria-label="Siguiente"></button>
<div class="h-centro"><h2 class="h-titulo"></h2><p class="h-texto"></p></div>
<div class="h-abajo"><span class="h-estado"></span>
<div class="h-voz" hidden><button class="h-play" aria-label="Play">&#9654;</button><div class="h-pista"><div class="h-lleno"></div></div><span class="h-dur"></span></div>
</div>
<button class="h-flecha h-ant" aria-label="Anterior">&#8249;</button><button class="h-flecha h-sig" aria-label="Siguiente">&#8250;</button>
</div>
"#;

const CSS: &str = r#"
@page { margin: 14mm 12mm; }
* { box-sizing: border-box; }
html { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
body { margin: 0; background: #1c1c1e; color: #f5f5f7;
  font: 15px/1.5 "Segoe UI", system-ui, -apple-system, Roboto, sans-serif; }
.barra { position: sticky; top: 0; z-index: 5; display: flex; align-items: center; gap: 16px;
  justify-content: space-between; padding: 12px 20px; background: #1c1c1e;
  border-bottom: 1px solid rgba(255,255,255,.07); flex-wrap: wrap; }
.barra h1 { margin: 0; font-size: 18px; }
.barra .sub { margin: 0; color: #98989d; font-size: 12.5px; }
.tabs { display: flex; gap: 8px; }
.tabs button { font: inherit; font-size: 14px; color: #f5f5f7; background: #2c2c2e; border: 0;
  border-radius: 10px; padding: 8px 14px; cursor: pointer; }
.tabs button.on { background: #0060df; color: #fff; }
main { max-width: 900px; margin: 0 auto; padding: 16px 20px 24px; }
.vista { display: none; } .vista.on { display: block; }
section { margin-top: 18px; }
section h2 { margin: 0 0 12px; font-size: 12.5px; font-weight: 600; color: #d1d1d6; display: inline-block;
  background: #2c2c2e; border-radius: 999px; padding: 3px 12px; }
ol.linea { list-style: none; margin: 0; padding: 0; position: relative; }
ol.linea::before { content: ""; position: absolute; left: 81px; top: 6px; bottom: 6px;
  width: 2px; background: #3a3a3d; }
.momento { display: grid; grid-template-columns: 64px 36px 1fr; align-items: start;
  margin-bottom: 12px; break-inside: avoid; page-break-inside: avoid; }
.momento time { font-variant-numeric: tabular-nums; color: #d1d1d6;
  text-align: right; padding-top: 13px; font-size: 13.5px; }
.momento .punto { width: 13px; height: 13px; border-radius: 50%; background: #1c1c1e;
  border: 2.5px solid #0a84ff; margin: 17px 0 0 11px; position: relative; z-index: 1; }
.tarjeta { background: #2a2a2d; border-radius: 12px; padding: 13px 15px; min-width: 0;
  cursor: pointer; transition: background .15s; outline: none; }
.tarjeta:hover, .tarjeta:focus-visible { background: #313134; }
.tarjeta h3 { margin: 0; font-size: 15.5px; line-height: 1.35; }
.tarjeta p { margin: 4px 0 0; color: #d1d1d6; font-size: 14px; overflow-wrap: anywhere; }
.fotos { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
.fotos img { width: 176px; height: 132px; object-fit: cover; border-radius: 10px; display: block; }
.voz { display: inline-block; margin-top: 10px; padding: 4px 12px; border-radius: 999px;
  background: #182b47; color: #64d2ff; font-size: 13px; }
footer { margin-top: 30px; color: #8e8e93; font-size: 12px; text-align: center; }
/* Momentos */
.anio { font-size: 34px; font-weight: 700; margin: 8px 0 4px; }
.mes { color: #98989d; font-size: 14px; margin: 10px 0 8px; }
.dia { display: flex; gap: 12px; margin-bottom: 18px; }
.dia .num { width: 88px; flex: none; font-size: 34px; font-weight: 700; line-height: 1; }
.minis { display: flex; flex-wrap: wrap; gap: 6px; flex: 1; }
.mini { width: 124px; height: 124px; border-radius: 6px; overflow: hidden; cursor: pointer;
  position: relative; background-size: cover; background-position: center; }
.mini span { position: absolute; inset: 10px; font-size: 13px; overflow: hidden;
  display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; }
.mini:hover { filter: brightness(1.12); }
.fin { text-align: center; color: #48484a; margin: 20px 0; }
/* Estado */
.mestar { display: flex; gap: 16px; background: #262628; border-radius: 16px; padding: 16px 18px;
  margin-bottom: 12px; }
.mestar .izq { width: 100px; flex: none; }
.mestar .n { font-size: 44px; font-weight: 700; line-height: 1.05; }
.mestar .so { color: #98989d; font-size: 12.5px; margin-top: 6px; }
.mestar .so b { display: block; color: #d1d1d6; font-size: 17px; font-weight: 400; }
.dias { display: grid; grid-template-columns: repeat(auto-fill, minmax(58px, 1fr)); gap: 8px 4px; flex: 1; }
.cel { text-align: center; color: rgba(255,255,255,.35); font-size: 15px; }
.cel.algo { color: #f5f5f7; cursor: pointer; }
.circ { width: 52px; height: 52px; margin: 6px auto 0; border-radius: 50%; display: flex;
  align-items: center; justify-content: center; font-size: 25px; background-size: cover;
  background-position: center; position: relative; }
.circ.pila { box-shadow: 7px 0 0 -1px #3a3a3d; }
.circ .p { width: 10px; height: 10px; border-radius: 50%; background: #0a84ff; }
/* La historia */
.historia { position: fixed; inset: 0; z-index: 20; background: #000; color: #fff;
  display: flex; align-items: center; justify-content: center; overflow: hidden; }
.historia[hidden] { display: none; }
.h-fondo, .h-velo { position: absolute; inset: 0; }
.h-velo { background: linear-gradient(180deg, rgba(0,0,0,.55) 0%, rgba(0,0,0,.3) 30%,
  rgba(0,0,0,.4) 60%, rgba(0,0,0,.78) 100%); }
.h-velo.suave { background: linear-gradient(180deg, rgba(0,0,0,.25), rgba(0,0,0,0) 30%, rgba(0,0,0,.3)); }
.h-barras { position: absolute; top: 10px; left: 12px; right: 12px; display: flex; gap: 4px; z-index: 3; }
.h-barras i { flex: 1; height: 3px; border-radius: 2px; background: rgba(255,255,255,.35); }
.h-barras i.on { background: #fff; }
.h-arriba { position: absolute; top: 20px; left: 18px; right: 12px; display: flex; align-items: center;
  gap: 10px; z-index: 3; font-size: 14px; text-shadow: 0 1px 3px rgba(0,0,0,.6); }
.h-cuenta { color: rgba(255,255,255,.65); flex: 1; }
.h-x { font: inherit; font-size: 18px; color: #fff; background: rgba(0,0,0,.35); border: 0;
  width: 40px; height: 40px; border-radius: 50%; cursor: pointer; }
.h-izq, .h-der { position: absolute; top: 70px; bottom: 120px; width: 50%; z-index: 1;
  background: none; border: 0; cursor: pointer; }
.h-izq { left: 0; } .h-der { right: 0; }
.h-centro { position: relative; z-index: 2; max-width: 680px; padding: 0 28px; text-align: center;
  pointer-events: none; }
.h-titulo { margin: 0; font-size: clamp(24px, 4.2vw, 34px); line-height: 1.25; font-weight: 700;
  text-shadow: 0 2px 12px rgba(0,0,0,.55), 0 1px 2px rgba(0,0,0,.6); }
.h-texto { margin: 14px 0 0; font-size: clamp(15px, 2vw, 18px); line-height: 1.5; color: rgba(255,255,255,.92);
  text-shadow: 0 1px 8px rgba(0,0,0,.6); white-space: pre-wrap; max-height: 40vh; overflow: auto; }
.h-abajo { position: absolute; left: 0; right: 0; bottom: 26px; z-index: 3; display: flex;
  flex-direction: column; align-items: center; gap: 12px; }
.h-estado { font-size: 18px; background: rgba(0,0,0,.4); border-radius: 999px; padding: 4px 14px; }
.h-estado:empty { display: none; }
.h-voz { display: flex; align-items: center; gap: 12px; background: rgba(0,0,0,.45);
  backdrop-filter: blur(8px); border-radius: 999px; padding: 6px 16px 6px 6px; min-width: 260px; }
.h-voz[hidden] { display: none; }
.h-play { width: 44px; height: 44px; border-radius: 50%; border: 0; background: #fff; color: #111;
  font-size: 16px; cursor: pointer; }
.h-pista { flex: 1; height: 4px; border-radius: 2px; background: rgba(255,255,255,.3); cursor: pointer; }
.h-lleno { height: 100%; width: 0; border-radius: 2px; background: #fff; }
.h-dur { font-variant-numeric: tabular-nums; font-size: 13.5px; }
.h-flecha { position: absolute; top: 50%; transform: translateY(-50%); z-index: 3; width: 48px; height: 48px;
  border-radius: 50%; border: 0; background: rgba(0,0,0,.35); color: #fff; font-size: 30px;
  line-height: 1; cursor: pointer; }
.h-ant { left: 14px; } .h-sig { right: 14px; }
@media (max-width: 600px) {
  main { padding: 12px; }
  .momento { grid-template-columns: 48px 28px 1fr; }
  ol.linea::before { left: 61px; }
  .momento .punto { margin-left: 7px; }
  .fotos img { width: calc(50% - 4px); height: 110px; }
  .mini { width: calc(33.3% - 4px); height: auto; aspect-ratio: 1; }
  .dia .num { width: 52px; font-size: 26px; }
  .mestar { flex-direction: column; }
  .h-flecha { display: none; }
}
@media print {
  body { background: #fff; color: #1d1d1f; }
  .barra { position: static; background: #fff; border-color: #e1e4ea; }
  .barra .sub { color: #6e6e73; }
  .tabs, .historia, #v-momentos, #v-estado, #medios { display: none !important; }
  #v-linea { display: block !important; }
  section h2 { background: #eef4ff; color: #0060df; }
  ol.linea::before { background: #d2d6de; }
  .momento time { color: #3a3a3c; }
  .momento .punto { background: #fff; }
  .tarjeta { background: #fff; border: 1px solid #e1e4ea; }
  .tarjeta p { color: #3a3a3c; }
  .voz { background: #eef4ff; color: #0060df; }
}
"#;

/// El JavaScript de la pagina: las pestanas, Momentos, Estado y la
/// historia. Sin nada de fuera.
const JS: &str = r##"
(function () {
  "use strict";
  var D = JSON.parse(document.getElementById("datos").textContent);
  var M = D.momentos, porId = {};
  M.forEach(function (m) { porId[m.id] = m; });
  function el(t, c, txt) { var e = document.createElement(t); if (c) e.className = c; if (txt != null) e.textContent = txt; return e; }
  function src(f) { var i = document.getElementById(f); return i ? i.getAttribute("src") : ""; }
  function fondo(m) { return "linear-gradient(135deg," + m.g[0] + "," + m.g[1] + ")"; }

  // Las pestanas.
  var hechas = {};
  document.querySelectorAll(".tabs button").forEach(function (b) {
    b.addEventListener("click", function () {
      var v = b.getAttribute("data-v");
      document.querySelectorAll(".tabs button").forEach(function (x) { x.classList.toggle("on", x === b); });
      document.querySelectorAll(".vista").forEach(function (x) { x.classList.toggle("on", x.id === "v-" + v); });
      if (!hechas[v]) { hechas[v] = true; if (v === "momentos") momentos(); if (v === "estado") estado(); }
    });
  });

  // La linea: cada tarjeta abre su historia, en el orden de la pagina.
  var orden = M.map(function (m) { return m.id; });
  document.querySelectorAll(".momento").forEach(function (li) {
    var t = li.querySelector(".tarjeta");
    function abrirla() { abrir(orden, orden.indexOf(li.getAttribute("data-id"))); }
    t.addEventListener("click", abrirla);
    t.addEventListener("keydown", function (e) { if (e.key === "Enter") abrirla(); });
  });

  // Por dia; dentro del dia, en el orden de la pagina.
  function porDia() {
    var d = {};
    M.forEach(function (m) { (d[m.dia] = d[m.dia] || []).push(m.id); });
    return d;
  }

  function momentos() {
    var raiz = document.getElementById("v-momentos"), d = porDia();
    var dias = Object.keys(d).sort().reverse(), lista = [];
    dias.forEach(function (k) { lista = lista.concat(d[k]); });
    var anio = "", mes = "";
    dias.forEach(function (k) {
      var a = k.slice(0, 4), me = k.slice(0, 7);
      if (a !== anio) { anio = a; raiz.appendChild(el("div", "anio", a)); }
      if (me !== mes) { mes = me; raiz.appendChild(el("div", "mes", D.meses[+k.slice(5, 7) - 1])); }
      var fila = el("div", "dia"), minis = el("div", "minis");
      fila.appendChild(el("div", "num", String(+k.slice(8, 10))));
      d[k].forEach(function (id) {
        var m = porId[id], c = el("div", "mini");
        if (m.fotos.length) c.style.backgroundImage = "url(" + src(m.fotos[0]) + ")";
        else { c.style.background = fondo(m); c.appendChild(el("span", "", m.titulo)); }
        c.addEventListener("click", function () { abrir(lista, lista.indexOf(id)); });
        minis.appendChild(c);
      });
      fila.appendChild(minis);
      raiz.appendChild(fila);
    });
    raiz.appendChild(el("div", "fin", "— • —"));
  }

  function estado() {
    var raiz = document.getElementById("v-estado"), d = porDia();
    var meses = {};
    Object.keys(d).forEach(function (k) { meses[k.slice(0, 7)] = true; });
    Object.keys(meses).sort().reverse().forEach(function (me, i) {
      var y = +me.slice(0, 4), mm = +me.slice(5, 7);
      var desde = new Date(y, mm, 0).getDate();
      if (i === 0) {
        // El mes mas nuevo empieza en su ultimo dia con algo.
        desde = 1;
        Object.keys(d).forEach(function (k) { if (k.slice(0, 7) === me) desde = Math.max(desde, +k.slice(8, 10)); });
      }
      var t = el("div", "mestar"), izq = el("div", "izq");
      izq.appendChild(el("div", "n", String(mm)));
      var so = D.sobre[me];
      if (so) { var p = el("div", "so", D.rot.sobre); p.appendChild(el("b", "", "«" + so + "»")); izq.appendChild(p); }
      t.appendChild(izq);
      var rej = el("div", "dias");
      for (var n = desde; n >= 1; n--) {
        var k = me + "-" + (n < 10 ? "0" : "") + n;
        rej.appendChild(celda(n, d[k]));
      }
      t.appendChild(rej);
      raiz.appendChild(t);
    });
  }

  function celda(n, ids) {
    var c = el("div", "cel" + (ids ? " algo" : ""), String(n));
    if (!ids) return c;
    var m = porId[ids[0]], fot = null, emo = null;
    ids.forEach(function (x) {
      if (!emo && porId[x].emo) emo = porId[x].emo;
      if (!fot && porId[x].fotos.length) fot = porId[x].fotos[0];
    });
    var ci = el("div", "circ" + (ids.length > 1 ? " pila" : ""));
    if (emo) { ci.textContent = emo; ci.style.background = fondo(m); }
    else if (fot) ci.style.backgroundImage = "url(" + src(fot) + ")";
    else { ci.style.background = fondo(m); ci.appendChild(el("span", "p")); }
    c.appendChild(ci);
    c.addEventListener("click", function () { abrir(ids, 0); });
    return c;
  }

  // La historia.
  var H = document.getElementById("historia");
  function q(s) { return H.querySelector(s); }
  var lista = [], pos = 0, foto = 0, sonando = null;
  function parar() {
    if (sonando) { sonando.pause(); sonando = null; }
    q(".h-play").innerHTML = "&#9654;";
    q(".h-lleno").style.width = "0";
  }
  function pintar() {
    var m = porId[lista[pos]];
    if (!m) { cerrar(); return; }
    parar();
    var n = m.fotos.length;
    foto = Math.max(0, Math.min(foto, n - 1));
    var f = q(".h-fondo"), v = q(".h-velo");
    if (n) { f.style.background = "#000 url(" + src(m.fotos[foto]) + ") center / cover no-repeat"; v.className = "h-velo"; }
    else { f.style.background = fondo(m); v.className = "h-velo suave"; }
    var b = q(".h-barras"); b.innerHTML = "";
    if (n > 1) for (var i = 0; i < n; i++) b.appendChild(el("i", i === foto ? "on" : ""));
    q(".h-fecha").textContent = m.fecha;
    q(".h-cuenta").textContent = lista.length > 1 ? (pos + 1) + " / " + lista.length : "";
    q(".h-titulo").textContent = m.titulo;
    q(".h-texto").textContent = m.texto;
    q(".h-estado").textContent = (m.emo || "") + (m.leccion ? (m.emo ? " " : "") + "💡" : "");
    var voz = q(".h-voz");
    if (m.audio && document.getElementById(m.audio)) { voz.hidden = false; q(".h-dur").textContent = m.dur; }
    else voz.hidden = true;
    var varios = lista.length > 1 || n > 1;
    q(".h-ant").style.display = varios ? "" : "none";
    q(".h-sig").style.display = varios ? "" : "none";
    try { history.replaceState(null, "", "#m=" + m.id); } catch (e) {}
  }
  function abrir(l, p) {
    if (p < 0) return;
    lista = l; pos = p; foto = 0; H.hidden = false;
    document.body.style.overflow = "hidden";
    pintar();
  }
  function cerrar() {
    parar(); H.hidden = true; document.body.style.overflow = "";
    try { history.replaceState(null, "", location.pathname + location.search); } catch (e) {}
  }
  function siguiente() {
    var m = porId[lista[pos]];
    if (m && foto < m.fotos.length - 1) foto++;
    else if (pos < lista.length - 1) { pos++; foto = 0; }
    else return;
    pintar();
  }
  function anterior() {
    if (foto > 0) foto--;
    else if (pos > 0) { pos--; foto = Math.max(0, porId[lista[pos]].fotos.length - 1); }
    else return;
    pintar();
  }
  function momento(paso) {
    var p = pos + paso;
    if (p >= 0 && p < lista.length) { pos = p; foto = 0; pintar(); }
  }
  q(".h-x").addEventListener("click", cerrar);
  q(".h-izq").addEventListener("click", anterior);
  q(".h-der").addEventListener("click", siguiente);
  q(".h-ant").addEventListener("click", anterior);
  q(".h-sig").addEventListener("click", siguiente);
  q(".h-play").addEventListener("click", function () {
    var m = porId[lista[pos]], a = m && m.audio && document.getElementById(m.audio);
    if (!a) return;
    if (sonando) { parar(); return; }
    sonando = a; a.currentTime = 0; a.play();
    q(".h-play").innerHTML = "&#10074;&#10074;";
    a.ontimeupdate = function () { if (a.duration) q(".h-lleno").style.width = (100 * a.currentTime / a.duration) + "%"; };
    a.onended = parar;
  });
  q(".h-pista").addEventListener("click", function (e) {
    if (!sonando || !sonando.duration) return;
    var r = this.getBoundingClientRect();
    sonando.currentTime = sonando.duration * (e.clientX - r.left) / r.width;
  });
  document.addEventListener("keydown", function (e) {
    if (H.hidden) return;
    if (e.key === "Escape") cerrar();
    else if (e.key === "ArrowRight") siguiente();
    else if (e.key === "ArrowLeft") anterior();
    else if (e.key === "ArrowDown" || e.key === "PageDown") momento(1);
    else if (e.key === "ArrowUp" || e.key === "PageUp") momento(-1);
    else return;
    e.preventDefault();
  });
  H.addEventListener("wheel", function (e) { e.preventDefault(); momento(e.deltaY > 0 ? 1 : -1); }, { passive: false });
  var y0 = null;
  H.addEventListener("touchstart", function (e) { y0 = e.touches[0].clientY; }, { passive: true });
  H.addEventListener("touchend", function (e) {
    if (y0 == null) return;
    var dy = e.changedTouches[0].clientY - y0; y0 = null;
    if (Math.abs(dy) > 60) momento(dy < 0 ? 1 : -1);
  });

  // «#m=<id>» abre esa historia al cargar.
  var h = location.hash.match(/^#m=([A-Za-z0-9_-]+)/);
  if (h && porId[h[1]]) abrir(orden, orden.indexOf(h[1]));
})();
"##;

#[cfg(test)]
mod pruebas {
    use super::*;

    fn op() -> Opciones {
        Opciones {
            titulo: "Timeline".into(),
            subtitulo: "Últimas 24 horas".into(),
            pie: "PixPin".into(),
            nota_de_voz: "Nota de voz".into(),
            desfase: 0,
            ingles: false,
            pestanas: ["Línea".into(), "Momentos".into(), "Estado".into()],
            sobre_todo: "Sobre todo".into(),
        }
    }

    fn una(m: &Momento) -> Vec<Seccion<'_>> {
        vec![Seccion {
            rotulo: "Hoy".into(),
            momentos: vec![m],
        }]
    }

    /// Los datos que lee el JavaScript, como los leeria `JSON.parse`.
    fn datos(html: &str) -> serde_json::Value {
        let a = html.find("id=\"datos\">").expect("datos") + "id=\"datos\">".len();
        let b = a + html[a..].find("</script>").expect("fin de datos");
        serde_json::from_str(&html[a..b]).expect("JSON valido")
    }

    #[test]
    fn base64_como_el_estandar() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn la_pagina_lleva_hora_titulo_descripcion_fotos_y_el_audio_dentro() {
        let mut m = Momento::nuevo(
            "a".into(),
            (14 * 60 + 5) * 60_000,
            "Llegó <el> pedido".into(),
            "venía\nroto".into(),
        );
        m.fotos = vec!["a.png".into(), "falta.png".into()];
        m.audio = Some("a.m4a".into());
        m.duracion_ms = 42_000;
        let html = pagina(
            &op(),
            &una(&m),
            &|n| (n == "a.png").then(|| data_uri(n, b"x")),
            &|n| Some(data_uri(n, b"voz")),
        );
        assert!(html.contains("<time>14:05</time>"));
        assert!(html.contains("Llegó &lt;el&gt; pedido"), "lo escrito se escapa");
        assert!(html.contains("venía<br>roto"));
        assert_eq!(html.matches("<img ").count(), 1, "la foto que falta se omite");
        assert!(html.contains("data:image/png;base64,eA=="));
        assert!(html.contains("Nota de voz · 0:42"));
        // El audio va dentro, como MP4, una sola vez.
        assert_eq!(html.matches("data:audio/mp4;base64,dm96").count(), 1);
        let d = datos(&html);
        let m0 = &d["momentos"][0];
        assert_eq!(m0["audio"], "a0");
        assert_eq!(m0["fotos"][0], "f0");
        assert_eq!(m0["dur"], "0:42");
        assert!(html.contains("break-inside: avoid"), "un momento no se parte al imprimir");
    }

    #[test]
    fn caso_negativo_un_titulo_con_comillas_y_script_no_rompe_la_pagina() {
        let m = Momento::nuevo(
            "a".into(),
            0,
            "Dijo \"hola\" </script><script>alert(1)</script>".into(),
            "l\u{2028}nea 'x'".into(),
        );
        let html = pagina(&op(), &una(&m), &|_| None, &|_| None);
        // Solo los dos <script> de la pagina; el del titulo va escapado.
        assert_eq!(html.matches("<script").count(), 2);
        assert_eq!(html.matches("</script>").count(), 2);
        let d = datos(&html);
        assert_eq!(
            d["momentos"][0]["titulo"],
            "Dijo \"hola\" </script><script>alert(1)</script>"
        );
        // En los datos, U+2028 va escapado (en el HTML de la tarjeta es solo
        // texto y no molesta).
        let a = html.find("id=\"datos\">").expect("datos");
        assert!(!html[a..].contains('\u{2028}'));
        assert_eq!(d["momentos"][0]["texto"], "l\u{2028}nea 'x'");
    }

    #[test]
    fn sin_descripcion_ni_audio_no_salen_sus_trozos() {
        let m = Momento::nuevo("a".into(), 0, "Solo".into(), String::new());
        let html = pagina(&op(), &una(&m), &|_| None, &|_| None);
        assert!(!html.contains("<p>"));
        assert!(!html.contains("class=\"voz\""));
        assert!(!html.contains("class=\"fotos\""));
        assert!(!html.contains("<audio"));
        assert_eq!(datos(&html)["momentos"][0]["audio"], serde_json::Value::Null);
    }

    #[test]
    fn el_tipo_de_cada_fichero_por_su_extension() {
        assert!(data_uri("v.m4a", b"").starts_with("data:audio/mp4;"));
        assert!(data_uri("v.MP3", b"").starts_with("data:audio/mpeg;"));
        assert!(data_uri("f.jpg", b"").starts_with("data:image/jpeg;"));
        // Caso negativo: lo desconocido no se hace pasar por audio.
        assert!(data_uri("x.raro", b"").starts_with("data:image/png;"));
        assert_eq!(id_html("tl-1 2/3"), "tl-1_2_3");
    }
}
