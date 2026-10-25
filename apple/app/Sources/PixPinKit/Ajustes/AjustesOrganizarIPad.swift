// **Tareas, Lienzo y Pinear** tal como valen ahora en el iPad: lo ultimo
// leido o guardado de `pixpinmax.toml` (`Nucleo.ajustesOrganizar`), para que
// lo usen el lienzo (el iman, el suavizado y que herramientas salen) y los
// pines (las palabras magicas) sin depender de la pantalla de Ajustes.
//
// Lo pone `ModeloAjustes` al cargar y al guardar, y quien no haya pasado por
// Ajustes lo lee con `cargar(nucleo:)`. Al cambiar avisa con
// `Notification.Name.pixpinCambiaronAjustesOrganizar`.

import Foundation
import PixPinCore

public extension Notification.Name {
    /// Cambiaron Tareas, Lienzo o Pinear en Ajustes (ya guardados).
    static let pixpinCambiaronAjustesOrganizar = Notification.Name("pixpinCambiaronAjustesOrganizar")
}

public enum AjustesOrganizarIPad {
    private static let cerrojo = NSLock()
    nonisolated(unsafe) private static var leidos: AjustesOrganizar?

    /// Lo de ahora; `nil` si nadie lo ha leido aun.
    public static var vigentes: AjustesOrganizar? {
        cerrojo.lock()
        defer { cerrojo.unlock() }
        return leidos
    }

    /// Lo pone quien lo leyo o lo guardo, y avisa.
    public static func poner(_ a: AjustesOrganizar) {
        cerrojo.lock()
        let cambia = leidos != a
        leidos = a
        cerrojo.unlock()
        if cambia {
            NotificationCenter.default.post(name: .pixpinCambiaronAjustesOrganizar, object: nil)
        }
    }

    /// **Lee el TOML** (bloqueante: fuera del hilo principal) y lo deja
    /// vigente. Si el fichero no se entiende, los de fabrica.
    @discardableResult
    public static func cargar(nucleo: Nucleo) -> AjustesOrganizar {
        let a = (try? nucleo.ajustesOrganizar()) ?? nucleo.ajustesOrganizarDeFabrica()
        poner(a)
        return a
    }

    /// Lo de ahora o, si nadie lo ha leido, lo lee.
    public static func actuales(nucleo: Nucleo) -> AjustesOrganizar {
        vigentes ?? cargar(nucleo: nucleo)
    }

    // MARK: Pinear

    /// **Las palabras magicas guardadas** como las leen `palabraMagica` y
    /// las demas funciones del nucleo: `nil` es «las de fabrica».
    public static func palabrasGuardadas(_ a: AjustesOrganizar) -> String? {
        a.palabrasPropias ? a.palabrasMagicas : nil
    }

    /// **La herramienta que abre `texto` al pinearlo** (`TIMER`, `STOPWATCH`,
    /// `CHECKLIST`, `COUNTER`, `LEDGER`, `BOARD`, `RULETA`, `DRAW`, `SHEET`),
    /// con las palabras de Ajustes → Pinear, o `nil` si es texto corriente.
    /// La palabra tiene que ir sola: «timer» si, «el timer» no.
    public static func palabraMagica(_ texto: String, nucleo: Nucleo) -> String? {
        let a = actuales(nucleo: nucleo)
        return PixPinCore.palabraMagica(texto: texto, guardadas: palabrasGuardadas(a))
    }

    // MARK: Lienzo

    /// El nombre de `[herramientas]` de una herramienta de la barra del
    /// lienzo del iPad (`flecha_libre` → `flecha-libre`). La flecha no se
    /// apaga (sin ella no se elige nada); la zona al chat va con la zona.
    public static func nombreEnToml(_ id: String) -> String? {
        switch id {
        case "seleccion": return nil
        case "zona_chat": return "zona"
        default: return id.replacingOccurrences(of: "_", with: "-")
        }
    }

    /// Si la herramienta `id` de la barra sale en `sitio` (`lienzo`, `pin`,
    /// `lector`, `pantalla`; vacio: en todos) con estos ajustes.
    public static func sale(_ id: String, en sitio: String, con a: AjustesOrganizar) -> Bool {
        guard let n = nombreEnToml(id) else { return true }
        let esta: ([String]) -> Bool = { l in l.contains { $0.trimmingCharacters(in: .whitespaces).caseInsensitiveCompare(n) == .orderedSame } }
        if esta(a.herramientasApagadas) { return false }
        if !sitio.isEmpty, let l = a.herramientasPorSitio[sitio], esta(l) { return false }
        return true
    }
}
