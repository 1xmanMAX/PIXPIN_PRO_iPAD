// El trabajador de fondo de la extension de Safari: lo que pide el menu del
// boton (`menu.js`) va a la extension nativa con
// `browser.runtime.sendNativeMessage`, que lo deja en el buzon de PixPin.
// Sin modulos (Safari arranca el trabajador sin `type: module`): las
// cuentas estan en `nucleo.js`, que el menu ya aplica antes de mandar.

const api = globalThis.browser ?? globalThis.chrome;

api.runtime.onMessage.addListener((m, _de, responder) => {
  if (m?.tipo !== "pixpin-mensaje") return false;
  api.runtime
    .sendNativeMessage("io.github.1xmanmax.pixpin", m.mensaje)
    .then(
      (r) => responder(r ?? { ok: false, error: "PixPin no contestó" }),
      (e) => responder({ ok: false, error: String(e?.message ?? e) }),
    );
  return true;
});
