// Lo que la zona de organizar pone en marcha al arrancar la app: el centro
// de avisos (su delegado unico de `UNUserNotificationCenter`, las
// categorias con sus botones y el permiso), el vigia de los recordatorios
// y el barrendero de las capturas que caducan; y los ganchos de Ajustes
// (Google Calendar) y del chat («Hacer lección»). Se apunta con
// `Arranque.apuntar` (`Raiz/EntornoApp.swift`), sin tocar `Raiz/`.
//
// Swift no tiene inicializadores de modulo: alguien tiene que llamar a
// `ArranqueOrganizar.apuntar()` antes de crear el `EntornoApp` (o en el
// `@main`). Por si nadie lo hace, las secciones de esta zona tambien lo
// llaman al aparecer (repetirlo no hace nada; si la app ya arranco,
// `Arranque` lo hace en el momento).

import Foundation
import PixPinCore

@MainActor
public enum ArranqueOrganizar {
    private static var apuntado = false
    /// El barrendero, vivo mientras viva la app.
    private(set) static var barrendero: BarrenderoCapturas?

    public static func apuntar() {
        guard !apuntado else { return }
        apuntado = true
        // Los ganchos de otras zonas (§0 del inventario): conectar Google
        // Calendar lo lee ya (sus eventos llaman como las clases) y «Hacer
        // lección» sale en el menu de los mensajes.
        GanchosAjustes.alConectar = { id in
            if id.lowercased().contains("calendar") { Recordatorios.compartido.leerCalendario() }
        }
        GanchosChat.hacerLeccion = { chat, mensaje, almacen in
            HacerLeccionIPad.abrir(chat: chat, mensaje: mensaje, almacen: almacen)
        }
        Arranque.apuntar { entorno in
            CentroAvisos.compartido.preparar()
            guard let almacen = entorno.almacen else { return }
            let nucleo = almacen.nucleo
            // Los dias de caducidad, del nucleo (los mismos de Windows).
            Task.detached(priority: .utility) { _ = AjusteCaducidad.leer(nucleo) }
            Recordatorios.compartido.vigilar(nucleo: nucleo)
            let b = BarrenderoCapturas(nucleo: nucleo)
            b.arrancar()
            barrendero = b
            // Lo que trae otro aparato puede traer recordatorios nuevos.
            NotificationCenter.default.addObserver(forName: .pixpinTrajoCambios, object: nil, queue: .main) { _ in
                MainActor.assumeIsolated { Recordatorios.compartido.releerConCalma() }
            }
        }
    }
}
