// El menu del boton de PixPin en Safari: lo seleccionado (tarea, nota, al
// chat, leccion, leer, asistente) y la pagina (su foto, su enlace). Arma el
// mensaje (`nucleo.js`) y lo manda por el trabajador de fondo a la
// extension nativa, que lo deja en el buzon; PixPin lo hace al abrirse (o
// en el acto si esta delante, en Split View).

import { MENU, deDataUrl, explicarFallo, mensajeDe } from "./nucleo.js";

const api = globalThis.browser ?? globalThis.chrome;
const $ = (id) => document.getElementById(id);
const [pestana] = await api.tabs.query({ active: true, currentWindow: true });

let texto = "";
try {
  const [r] = await api.scripting.executeScript({
    target: { tabId: pestana.id },
    func: () => String(getSelection?.() ?? ""),
  });
  texto = (r?.result ?? "").trim();
} catch {
  // Una pagina donde no se puede leer la seleccion.
}
if (texto) $("vista").textContent = `«${texto.replace(/\s+/g, " ").slice(0, 120)}»`;

for (const f of MENU) {
  const b = document.createElement("button");
  b.textContent = f.titulo;
  b.disabled = f.contexto === "selection" && !texto;
  b.addEventListener("click", () => elegir(f.accion));
  $(f.contexto === "selection" ? "seleccion" : "pagina").append(b);
}

$("pregunta").addEventListener("keydown", (e) => {
  if (e.key === "Enter") mandar("asistente", $("pregunta").value);
});

function elegir(accion) {
  const fila = MENU.find((f) => f.accion === accion);
  if (fila.contexto === "selection" && !texto) return;
  if (accion === "asistente") {
    $("pregunta").style.display = "block";
    $("pregunta").focus();
    return;
  }
  mandar(accion);
}

async function mandar(accion, pregunta) {
  $("estado").className = "";
  $("estado").textContent = accion === "captura_pagina" ? "Fotografiando la página…" : "Enviando…";
  for (const b of document.querySelectorAll("button")) b.disabled = true;
  try {
    const datos = { url: pestana.url, titulo: pestana.title, texto, pregunta };
    if (accion === "captura_pagina") {
      datos.imagen = deDataUrl(await api.tabs.captureVisibleTab(pestana.windowId, { format: "png" }));
    }
    const r = await api.runtime.sendMessage({ tipo: "pixpin-mensaje", mensaje: mensajeDe(accion, datos) });
    if (!r?.ok) throw new Error(r?.error || "PixPin no contestó");
    $("estado").textContent = r.aviso ?? "Hecho";
    setTimeout(() => window.close(), 900);
  } catch (e) {
    $("estado").className = "mal";
    $("estado").textContent = explicarFallo(e);
    for (const b of document.querySelectorAll("button")) b.disabled = false;
  }
}
