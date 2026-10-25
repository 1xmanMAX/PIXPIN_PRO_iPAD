// **Lo que el asistente pone en marcha al arrancar** (sin tocar `Raiz/`):
//
// - el Llavero y el HTTPS en el nucleo (las claves de la nube cifradas);
// - `GanchosAlmacen.alEnviar`: lo que llega al chat «Asistente» o con
//   `@asistente` lo contesta `AsistenteEnElChat`;
// - `AbrirDesdeFuera.alEnlace`: los enlaces `pixpinmax://` van a
//   `HacerPedidos` (que tambien atiende Atajos y Siri, `AccionesApp`);
// - el motor de las automatizaciones (la hora, las capturas nuevas, lo que
//   trae la sincronizacion);
// - el chat «Asistente», que existe siempre (como «Mensajes guardados»).
//
// Lo llama la app al arrancar (`AsistenteDelIPad.activar()` en `PixPinApp`)
// y, por si acaso, la seccion del asistente al abrirse. Una vez.

import Foundation
import PixPinCore

@MainActor
public enum AsistenteDelIPad {
    private static var activado = false

    public static func activar() {
        guard !activado else { return }
        activado = true
        ModeloAsistente.instalarLaNube()
        Arranque.apuntar { entorno in
            HacerPedidos.ponerEnMarcha(entorno)
            AccionesApp.compartidas.registrar(almacen: entorno.almacen, sincro: entorno.sincro) {
                // En el iPad no se captura otra app: la galeria (camara,
                // escaner, fotos) es lo mas cercano.
                Navegacion.compartida.cerrarVisor()
                Navegacion.compartida.seccion = .galeria
            }
            GanchosAlmacen.alEnviar = { mensajes, chat, almacen in
                _ = AsistenteEnElChat.compartido.alEnviar(mensajes, chat: chat, almacen: almacen)
            }
            guard let almacen = entorno.almacen else { return }
            MotorAutomatizaciones.compartido.arrancar(nucleo: almacen.nucleo, almacen: almacen)
            Task { await almacen.asegurarAsistente() }
        }
    }
}
