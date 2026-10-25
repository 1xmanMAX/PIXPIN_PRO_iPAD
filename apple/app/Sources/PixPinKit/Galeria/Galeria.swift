// La galeria de capturas, como `galeria_capturas.rs` de Windows (la v2,
// B6): las de `capturas/`, la mas nueva primero y agrupadas por dia, con
// los dias que les quedan; un buscador que tambien encuentra el texto de
// dentro (leido con Vision en segundo plano, `LectorTextoGaleria`); elegir
// varias con su barra; y en el clic derecho pinear, copiar, compartir,
// «Guardar como…», mandar a un proyecto, anotar en un lienzo, mostrar en el
// Finder, conservar, «7 dias mas» y borrar (con deshacer). Lo que decide
// (caducidad, papelera, conservar, buscar) es del nucleo de Rust, con los
// mismos ficheros que Windows (`galeria.rs`).
//
// Rendimiento (§4bis del plan):
// - la lista llega por paginas (`tamPagina`) y la rejilla es perezosa: solo
//   existen las celdas que se ven, y la pagina siguiente se pide al llegar
//   al final;
// - las miniaturas son de ImageIO, reducidas al leer y en la cache con tope
//   de toda la app (`Miniaturas.compartidas`), fuera del hilo principal;
// - la carpeta la vigila el sistema (`VigiaCarpeta`): sin sondear.
//
// En el iPad (del Mac casi tal cual): los dias del ajuste salen del nucleo
// (`[capturas] dias_caducidad`, el mismo de Windows), leidos al arrancar.

import Foundation
import PixPinCore
import SwiftUI

/// Los dias que aguanta una captura (el ajuste «dias hasta borrar» de
/// Windows, `[capturas] dias_caducidad` de `pixpinmax.toml`). 0 = nunca.
/// Se lee del nucleo al arrancar y en cada barrido (`BarrenderoCapturas`);
/// mientras tanto vale lo ultimo leido (guardado en `UserDefaults`).
public enum AjusteCaducidad {
    public static let clave = "capturas.diasCaducidad"
    public static let porOmision = 7

    public static var dias: Int64 {
        let d = UserDefaults.standard.object(forKey: clave) as? Int ?? porOmision
        return Int64(max(0, d))
    }

    /// Lo que dice el nucleo (bloqueante: fuera del hilo principal).
    public static func leer(_ nucleo: Nucleo) -> Int64 {
        guard let a = try? nucleo.ajustes() else { return dias }
        let d = Int(a.diasCaducidad)
        UserDefaults.standard.set(d, forKey: clave)
        return Int64(d)
    }
}

@MainActor
public final class ModeloGaleria: ObservableObject {
    /// Cuantas capturas trae cada pagina: unas cuantas pantallas de rejilla.
    public nonisolated static let tamPagina: UInt32 = 120

    @Published public private(set) var capturas: [CapturaGaleria] = []
    /// Cuantas hay (con algo buscado, cuantas encajan).
    @Published public private(set) var total: UInt64 = 0
    /// Cuantas hay en la carpeta, se busque o no (el subtitulo).
    @Published public private(set) var enLaCarpeta: UInt64 = 0
    @Published public private(set) var error: String?
    /// Lo ultimo que se borro, para «Deshacer».
    @Published public private(set) var ultimoBorrado: [CapturaBorrada] = []
    @Published public private(set) var aviso: String?
    /// Lo escrito en el buscador.
    @Published public var consulta = ""

    public let almacen: Almacen
    private var cargando = false
    private var vigia: VigiaCarpeta?
    private var tareaBuscar: Task<Void, Never>?
    private var tareaAviso: Task<Void, Never>?
    private var lector: LectorTextoGaleria?
    /// Los dias del ajuste; las pruebas lo fijan.
    var dias: () -> Int64 = { AjusteCaducidad.dias }

    public init(almacen: Almacen) {
        self.almacen = almacen
    }

    private var nucleo: Nucleo { almacen.nucleo }

    public var quedanMas: Bool { UInt64(capturas.count) < total }

    /// Empieza a vigilar `capturas/` y a leer el texto de las capturas (al
    /// abrir la ventana).
    public func vigilar() {
        guard vigia == nil else { return }
        let carpeta = URL(fileURLWithPath: nucleo.carpetaCapturas(), isDirectory: true)
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        vigia = VigiaCarpeta(carpeta) { [weak self] in
            Task {
                await self?.recargar()
                self?.lector?.despertar()
            }
        }
        let lector = LectorTextoGaleria(nucleo: nucleo) { [weak self] in
            // Lo recien leido puede encajar con lo buscado.
            guard let self, !self.consulta.trimmingCharacters(in: .whitespaces).isEmpty else { return }
            Task { await self.recargar() }
        }
        self.lector = lector
        lector.despertar()
    }

    public func dejarDeVigilar() {
        vigia = nil
        lector?.parar()
        lector = nil
    }

    /// Relee desde el principio lo que ya estaba cargado (al menos una
    /// pagina): tras borrar o al cambiar la carpeta.
    public func recargar() async {
        let cuantos = UInt32(min(max(capturas.count, Int(ModeloGaleria.tamPagina)), 2000))
        await cargar(desde: 0, cuantos: cuantos, sustituir: true)
    }

    /// La pagina siguiente a lo cargado (al llegar al final de la rejilla).
    public func cargarMas() async {
        guard quedanMas else { return }
        await cargar(desde: UInt64(capturas.count), cuantos: ModeloGaleria.tamPagina, sustituir: false)
    }

    /// Lo escrito cambio: se busca tras una espera corta, desde arriba.
    public func consultaCambiada() {
        tareaBuscar?.cancel()
        tareaBuscar = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(120))
            guard !Task.isCancelled, let self else { return }
            await self.cargar(desde: 0, cuantos: ModeloGaleria.tamPagina, sustituir: true)
        }
    }

    private func cargar(desde: UInt64, cuantos: UInt32, sustituir: Bool) async {
        guard !cargando else { return }
        cargando = true
        defer { cargando = false }
        let nucleo = self.nucleo
        let dias = self.dias()
        let desfase = desfaseLocalMs()
        let q = consulta.trimmingCharacters(in: .whitespaces)
        do {
            let (pagina, todas) = try await Task.detached(priority: .userInitiated) { () -> (PaginaGaleria, UInt64) in
                if q.isEmpty {
                    let p = try nucleo.galeria(desde: desde, cuantos: cuantos, dias: dias, desfaseMs: desfase)
                    return (p, p.total)
                }
                let p = try nucleo.galeriaBuscar(consulta: q, desde: desde, cuantos: cuantos, dias: dias,
                                                 desfaseMs: desfase)
                return (p, try nucleo.galeria(desde: 0, cuantos: 0, dias: dias, desfaseMs: desfase).total)
            }.value
            // Si lo buscado cambio mientras tanto, esto ya no vale.
            guard q == consulta.trimmingCharacters(in: .whitespaces) else { return }
            if sustituir {
                capturas = pagina.capturas
            } else {
                capturas.append(contentsOf: pagina.capturas)
            }
            total = pagina.total
            enLaCarpeta = todas
            error = nil
        } catch {
            self.error = "\(error)"
        }
    }

    /// Conserva las capturas: van a «Mensajes guardados» y dejan de caducar.
    public func conservar(_ nombres: [String]) async {
        let nucleo = self.nucleo
        let pendientes = nombres.filter { n in capturas.first { $0.nombre == n }?.conservada != true }
        guard !pendientes.isEmpty else {
            if !nombres.isEmpty { avisar("Ya estaban conservadas") }
            return
        }
        let fallo = await Task.detached(priority: .userInitiated) { () -> String? in
            var fallo: String?
            for n in pendientes {
                do { _ = try nucleo.conservarCaptura(nombre: n) } catch { fallo = "\(n): \(error)" }
            }
            return fallo
        }.value
        error = fallo
        if fallo == nil {
            avisar(pendientes.count == 1 ? "Conservada: está en Mensajes guardados y ya no se borra"
                : "\(pendientes.count) conservadas: están en Mensajes guardados")
        }
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        await recargar()
    }

    /// «Dar 7 dias mas» a las que aun se van.
    public func prorrogar(_ nombres: [String]) async {
        let nucleo = self.nucleo
        let dias = self.dias()
        let fallo = await Task.detached(priority: .userInitiated) { () -> String? in
            var fallo: String?
            for n in nombres {
                do { _ = try nucleo.prorrogarCaptura(nombre: n, dias: dias) } catch { fallo = "\(n): \(error)" }
            }
            return fallo
        }.value
        error = fallo
        if fallo == nil { avisar("Siete días más: se borrará más tarde") }
        await recargar()
    }

    /// Borra sin preguntar (a la papelera de PixPin): se deshace con
    /// `deshacer()`.
    public func borrar(_ nombres: [String]) async {
        guard !nombres.isEmpty else { return }
        let nucleo = self.nucleo
        let resultado = await Task.detached(priority: .userInitiated) { () -> Result<[CapturaBorrada], Error> in
            Result(catching: { try nucleo.borrarCapturas(nombres: nombres) })
        }.value
        switch resultado {
        case let .success(borradas):
            ultimoBorrado = borradas
            error = nil
            avisar(borradas.count == 1 ? "Llevada a la papelera de PixPin"
                : "\(borradas.count) llevadas a la papelera de PixPin", largo: true)
        case let .failure(e):
            error = "\(e)"
        }
        await recargar()
    }

    /// Devuelve lo ultimo borrado a su sitio, con lo que tenia apuntado.
    public func deshacer() async {
        let borradas = ultimoBorrado
        guard !borradas.isEmpty else { return }
        ultimoBorrado = []
        let nucleo = self.nucleo
        let vueltas = await Task.detached(priority: .userInitiated) {
            (try? nucleo.devolverCapturas(borradas: borradas)) ?? 0
        }.value
        avisar(vueltas == 0 ? "No se pudo deshacer" : vueltas == 1 ? "Recuperada" : "\(vueltas) recuperadas")
        await recargar()
    }

    public func url(de c: CapturaGaleria) -> URL {
        URL(fileURLWithPath: c.ruta)
    }

    /// El aviso de abajo, unos segundos.
    public func avisar(_ t: String?, largo: Bool = false) {
        aviso = t
        tareaAviso?.cancel()
        guard t != nil else { return }
        tareaAviso = Task { [weak self] in
            try? await Task.sleep(for: .seconds(largo ? 6 : 3))
            guard !Task.isCancelled else { return }
            self?.aviso = nil
        }
    }

    // MARK: Los grupos por dia

    /// Un dia de la rejilla: sus capturas, la mas nueva primero.
    public struct Grupo: Identifiable, Equatable {
        /// `AAAA-MM-DD` en el huso de aqui.
        public let id: String
        public let fecha: Date
        public let capturas: [CapturaGaleria]
    }

    /// Lo cargado, cortado por dias (`logica::disponer` de Windows).
    public var grupos: [Grupo] {
        ModeloGaleria.agrupar(capturas)
    }

    nonisolated static func agrupar(_ v: [CapturaGaleria], calendario: Calendar = .current) -> [Grupo] {
        var out: [Grupo] = []
        var actual: (String, Date, [CapturaGaleria])?
        for c in v {
            let f = fechaDe(ms: c.cuando)
            let k = calendario.dateComponents([.year, .month, .day], from: f)
            let id = String(format: "%04d-%02d-%02d", k.year ?? 0, k.month ?? 0, k.day ?? 0)
            if actual?.0 == id {
                actual?.2.append(c)
            } else {
                if let a = actual { out.append(Grupo(id: a.0, fecha: a.1, capturas: a.2)) }
                actual = (id, f, [c])
            }
        }
        if let a = actual { out.append(Grupo(id: a.0, fecha: a.1, capturas: a.2)) }
        return out
    }

    /// «Hoy · jueves 15 oct», «Ayer · …» o «lunes 12 oct».
    nonisolated static func rotulo(_ fecha: Date, ahora: Date = Date(), calendario: Calendar = .current) -> String {
        let dia = fecha.formatted(.dateTime.weekday(.wide).day().month(.abbreviated))
        if calendario.isDate(fecha, inSameDayAs: ahora) { return "Hoy · \(dia)" }
        if let ayer = calendario.date(byAdding: .day, value: -1, to: ahora),
           calendario.isDate(fecha, inSameDayAs: ayer) { return "Ayer · \(dia)" }
        if !calendario.isDate(fecha, equalTo: ahora, toGranularity: .year) {
            return fecha.formatted(.dateTime.weekday(.wide).day().month(.abbreviated).year())
        }
        return dia
    }
}
