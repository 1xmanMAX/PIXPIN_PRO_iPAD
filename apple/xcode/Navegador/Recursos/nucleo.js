// Lo que la extension de Safari del iPad decide sin tocar el navegador: el
// menu y el mensaje para la app (el de `extensiones/navegador/nucleo.js`
// de Windows, sin el puente: en el iPad el mensaje va a la propia
// extension nativa, `SafariWebExtensionHandler.swift`, que lo deja en el
// buzon del App Group y PixPin lo hace al abrirse).
//
// Safari del iPad no tiene menu contextual para las extensiones: todo va
// por el menu del boton (las imagenes sueltas, no; la foto de lo que se
// ve, si).

/** Version del mensaje que entiende la extension nativa. */
export const VERSION = 1;

/** El menu: una fila por accion. `contexto`: `selection` o `page`. */
export const MENU = [
  { accion: "tarea", contexto: "selection", titulo: "Nueva tarea" },
  { accion: "nota", contexto: "selection", titulo: "Nueva nota" },
  { accion: "chat", contexto: "selection", titulo: "Al chat" },
  { accion: "leccion", contexto: "selection", titulo: "Lección aprendida" },
  { accion: "leer", contexto: "selection", titulo: "Leer en voz alta" },
  { accion: "asistente", contexto: "selection", titulo: "Preguntar al asistente…" },
  { accion: "captura_pagina", contexto: "page", titulo: "Foto de lo que se ve" },
  { accion: "enlace_al_chat", contexto: "page", titulo: "Enlace al chat" },
];

/** Lo mas que se manda de imagen (base64), para no ahogar el mensaje. */
export const TOPE_IMAGEN_BASE64 = 24 * 1024 * 1024;

/** La fila del menu de una accion, o `undefined`. */
export function filaDe(accion) {
  return MENU.find((f) => f.accion === accion);
}

/** Si la accion necesita texto seleccionado. */
export function pideTexto(accion) {
  return filaDe(accion)?.contexto === "selection";
}

/**
 * El mensaje para la extension nativa. `datos`: `{texto, url, titulo,
 * imagen, pregunta}` (`imagen` = `{tipo, base64}`). Lo que no hace falta
 * no se manda.
 */
export function mensajeDe(accion, datos = {}) {
  const fila = filaDe(accion);
  if (!fila) throw new Error(`Acción desconocida: ${accion}`);
  const m = { v: VERSION, accion };
  if (fila.contexto === "selection") {
    const texto = (datos.texto ?? "").trim();
    if (!texto) throw new Error("No hay texto seleccionado");
    m.texto = texto;
    if (accion === "asistente" && datos.pregunta?.trim()) m.pregunta = datos.pregunta.trim();
  }
  if (accion === "captura_pagina") {
    if (!datos.imagen?.base64) throw new Error("No llegó la imagen");
    if (datos.imagen.base64.length > TOPE_IMAGEN_BASE64) throw new Error("La imagen es demasiado grande");
    m.imagen = { tipo: datos.imagen.tipo, base64: datos.imagen.base64 };
  }
  if (datos.url) m.url = String(datos.url);
  if (datos.titulo) m.titulo = String(datos.titulo);
  return m;
}

/** `data:image/png;base64,AAAA` a `{tipo, base64}`. */
export function deDataUrl(url) {
  const m = /^data:([^;,]+);base64,(.*)$/s.exec(String(url ?? ""));
  if (!m) throw new Error("No llegó la imagen");
  return { tipo: m[1], base64: m[2] };
}

/** El aviso de un fallo que entiende una persona. */
export function explicarFallo(error) {
  const t = String(error?.message ?? error ?? "");
  if (/grupo|App Group/i.test(t)) return "Esta copia de PixPin no está firmada con su App Group: no hay buzón.";
  return t || "Algo falló";
}
