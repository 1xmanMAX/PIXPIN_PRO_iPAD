// **Lo que cambia en el disco, a la vista** (`lector_pdf/al_dia.rs` de
// Windows). El lector guarda en memoria la tinta de cada hoja que ya leyo;
// lo que otro escribia en ella con el lector abierto (la sincronizacion con
// el movil, el asistente) no se veia hasta reabrir, y al guardar lo de aqui
// lo pisaba. Ahora:
//
// - **Mirar barato**: como mucho cada 0,7 s, solo la firma (fecha y tamano)
//   de las tintas ya abiertas. Si cambio y aqui no hay nada a medias, se
//   relee.
// - **Sin pisar**: con algo a medias no se relee; al guardar, si el disco
//   cambio desde que se leyo, lo de aqui se junta con lo del disco con la
//   regla de la sincronizacion (`juntarTintaConElDisco`), siendo la base lo
//   que se leyo, y se sigue anotando sobre lo junto.
//
// Todo esto va en la cola de guardar del lector (en serie): nada del disco
// en el hilo principal.

import Foundation
import PixPinCore

/// Lo que se sabe del disco de cada tinta abierta. Solo se toca desde la
/// cola de guardar del lector.
final class AlDiaLector: @unchecked Sendable {
    /// Cada cuanto, como mucho, se mira el disco.
    static let mirarCada: TimeInterval = 0.7

    /// La firma de un fichero: su fecha y su tamano.
    struct Sello: Equatable {
        let fecha: Date
        let tam: Int
    }

    /// Como estaba en el disco la tinta de una hoja la ultima vez que se
    /// leyo o se escribio aqui.
    struct Visto {
        let ruta: String
        let sello: Sello?
        /// El texto de entonces: la base para juntar.
        let base: String?
    }

    private var vistos: [Int: Visto] = [:]

    static func sello(_ ruta: String) -> Sello? {
        guard let a = try? FileManager.default.attributesOfItem(atPath: ruta),
              let f = a[.modificationDate] as? Date, let t = a[.size] as? NSNumber else { return nil }
        return Sello(fecha: f, tam: t.intValue)
    }

    /// La firma antes que el texto: si cambia entre medias, la proxima vez
    /// se vuelve a leer (de mas, nunca de menos).
    static func ver(_ ruta: String) -> Visto {
        let s = sello(ruta)
        return Visto(ruta: ruta, sello: s, base: try? String(contentsOfFile: ruta, encoding: .utf8))
    }

    func apuntar(_ i: Int, _ v: Visto) { vistos[i] = v }

    func olvidar() { vistos = [:] }

    /// **Apunta** las tintas abiertas que aun no se miraban, olvida las que
    /// ya no estan y dice cuales cambiaron en el disco desde la ultima vez.
    func cambiadas(_ abiertas: [Int: String]) -> [Int] {
        for (i, ruta) in abiertas where vistos[i]?.ruta != ruta {
            vistos[i] = AlDiaLector.ver(ruta)
        }
        for i in vistos.keys where abiertas[i] == nil { vistos[i] = nil }
        return vistos.compactMap { i, v in AlDiaLector.sello(v.ruta) != v.sello ? i : nil }.sorted()
    }

    /// **Guarda una tinta**. Si el disco cambio desde que se leyo, se junta
    /// con el (y devuelve `true`: hay que releerla); si no, se guarda como
    /// siempre y se apunta lo escrito, que no cuenta como cambio de fuera.
    /// `escrito`: si escribio algo.
    func guardar(_ i: Int, _ ed: EditorLienzo) throws -> (escrito: Bool, junto: Bool) {
        let ruta = ed.ruta()
        if let v = vistos[i], v.ruta == ruta, FileManager.default.fileExists(atPath: ruta),
           AlDiaLector.sello(ruta) != v.sello,
           let suyo = try? String(contentsOfFile: ruta, encoding: .utf8) {
            guard try ed.guardar() else { return (false, false) }
            let mio = try String(contentsOfFile: ruta, encoding: .utf8)
            if let junto = juntarTintaConElDisco(base: v.base, mio: mio, suyo: suyo) {
                try Data(junto.utf8).write(to: URL(fileURLWithPath: ruta), options: .atomic)
            }
            // La base no se mueve hasta releer: si lo junto no llega a
            // abrirse (se siguio anotando), el proximo guardado junta otra vez.
            return (true, true)
        }
        let escrito = try ed.guardar()
        if escrito { vistos[i] = AlDiaLector.ver(ruta) }
        return (escrito, false)
    }
}
