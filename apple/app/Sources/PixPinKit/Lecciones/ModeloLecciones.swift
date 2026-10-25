// Las lecciones aprendidas, por paginas (§4bis.2): la lista se pide al
// nucleo de Rust (`Nucleo.lecciones`) en trozos de `tamPagina`, con el
// filtro (palabras, etiqueta, «para repasar»), y la siguiente pagina se pide
// al llegar al final. Todo lo que toca el disco va fuera del hilo principal.
//
// Se guardan EXACTAMENTE como en Windows y Android (un `.leccion` en
// `android/guardados/lecciones/` del chat y su mensaje `ARCHIVO`), asi que
// viajan al sincronizar sin hacer nada mas.

import Foundation
import PixPinCore

/// Las causas de siempre (las de `pixpin_lecciones::leccion::CAUSAS`).
public let causasDeLeccion = ["Prisa", "No revisé", "Comunicación", "No sabía", "Supuse algo",
                              "Herramienta", "Planificación", "Cansancio", "Distracción"]
/// Las areas con las que se empieza (`AREAS`).
public let areasDeLeccion = ["Trabajo", "Construcción", "Estudio", "Vida diaria"]
/// Un tipo de leccion (`TIPO_LECCION`, `TIPO_ERROR`, `TIPO_ACIERTO`).
public struct TipoLeccion: Identifiable, Hashable, Sendable {
    public let id: String
    public let nombre: String
}

public let tiposDeLeccion = [
    TipoLeccion(id: "leccion", nombre: "💡 Lección"),
    TipoLeccion(id: "error", nombre: "⚠️ Error que no repetir"),
    TipoLeccion(id: "acierto", nombre: "✅ Lo que funcionó"),
]

@MainActor
public final class ModeloLecciones: ObservableObject {
    public nonisolated static let tamPagina: UInt32 = 50

    @Published public private(set) var lecciones: [FichaLeccion] = []
    @Published public private(set) var total: UInt64 = 0
    @Published public private(set) var paraRepasar: UInt64 = 0
    @Published public private(set) var etiquetas: [EtiquetaContada] = []
    @Published public private(set) var error: String?
    @Published public private(set) var cargando = false

    /// Lo que se busca. Cambiarlo relee tras una pausa corta (se escribe).
    @Published public var texto = "" {
        didSet { if texto != oldValue { programarBusqueda() } }
    }
    @Published public var etiqueta: String? {
        didSet { if etiqueta != oldValue { Task { await recargar() } } }
    }
    @Published public var soloParaRepasar = false {
        didSet { if soloParaRepasar != oldValue { Task { await recargar() } } }
    }

    public let nucleo: Nucleo
    /// Cada lectura lleva su numero: una respuesta vieja que llega tarde
    /// (otra busqueda ya salio) se tira.
    private var generacion = 0
    private var espera: Task<Void, Never>?

    public init(nucleo: Nucleo) {
        self.nucleo = nucleo
    }

    public var filtro: FiltroLecciones {
        FiltroLecciones(texto: texto, etiqueta: etiqueta, soloParaRepasar: soloParaRepasar)
    }

    public var quedanMas: Bool { UInt64(lecciones.count) < total }

    private func programarBusqueda() {
        espera?.cancel()
        espera = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 250_000_000)
            guard !Task.isCancelled else { return }
            await self?.recargar()
        }
    }

    /// La primera pagina con el filtro de ahora.
    public func recargar() async {
        generacion += 1
        let mia = generacion
        let nucleo = self.nucleo
        let filtro = self.filtro
        cargando = true
        let r = await Task.detached(priority: .userInitiated) { () -> Result<PaginaLecciones, Error> in
            do { return .success(try nucleo.lecciones(filtro: filtro, desde: 0, cuantas: ModeloLecciones.tamPagina)) } catch { return .failure(error) }
        }.value
        guard mia == generacion else { return }
        cargando = false
        aplicar(r, anadir: false)
    }

    /// La pagina siguiente (al llegar al final de la lista).
    public func cargarMas() async {
        guard quedanMas, !cargando else { return }
        let mia = generacion
        let nucleo = self.nucleo
        let filtro = self.filtro
        let desde = UInt64(lecciones.count)
        cargando = true
        let r = await Task.detached(priority: .userInitiated) { () -> Result<PaginaLecciones, Error> in
            do { return .success(try nucleo.lecciones(filtro: filtro, desde: desde, cuantas: ModeloLecciones.tamPagina)) } catch { return .failure(error) }
        }.value
        guard mia == generacion else { return }
        cargando = false
        aplicar(r, anadir: true)
    }

    private func aplicar(_ r: Result<PaginaLecciones, Error>, anadir: Bool) {
        switch r {
        case let .success(p):
            lecciones = anadir ? lecciones + p.lecciones : p.lecciones
            total = p.total
            paraRepasar = p.paraRepasar
            etiquetas = p.etiquetas
            error = nil
        case let .failure(e):
            error = ModeloAjustes.texto(de: e)
        }
    }

    /// Guarda una leccion (nueva o editada) y relee la lista.
    @discardableResult
    public func guardar(_ editada: LeccionEditada) async -> FichaLeccion? {
        let nucleo = self.nucleo
        let r = await Task.detached(priority: .userInitiated) { () -> Result<FichaLeccion, Error> in
            do { return .success(try nucleo.guardarLeccion(editada: editada)) } catch { return .failure(error) }
        }.value
        return await terminar(r)
    }

    /// Apunta el repaso (o que volvio a pasar).
    @discardableResult
    public func repasar(_ id: String, nota: NotaRepaso) async -> FichaLeccion? {
        let nucleo = self.nucleo
        let r = await Task.detached(priority: .userInitiated) { () -> Result<FichaLeccion, Error> in
            do { return .success(try nucleo.repasarLeccion(id: id, nota: nota)) } catch { return .failure(error) }
        }.value
        return await terminar(r)
    }

    public func borrar(_ id: String) async {
        let nucleo = self.nucleo
        let fallo = await Task.detached(priority: .userInitiated) { () -> String? in
            do {
                try nucleo.borrarLeccion(id: id)
                return nil
            } catch {
                return ModeloAjustes.texto(de: error)
            }
        }.value
        error = fallo
        avisarCambio()
        await recargar()
    }

    /// Lo que propone el etiquetador para lo escrito.
    public func proponer(texto: String, quitadas: [String]) async -> PropuestaLeccion {
        let nucleo = self.nucleo
        return await Task.detached(priority: .utility) {
            nucleo.proponerLeccion(texto: texto, quitadas: quitadas)
        }.value
    }

    private func terminar(_ r: Result<FichaLeccion, Error>) async -> FichaLeccion? {
        switch r {
        case let .success(f):
            error = nil
            avisarCambio()
            await recargar()
            return f
        case let .failure(e):
            error = ModeloAjustes.texto(de: e)
            return nil
        }
    }

    /// La sincronizacion sola cuenta sus 30 s de calma desde aqui.
    private func avisarCambio() {
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }
}

/// Lo que se edita en la ficha, campo a campo, antes de guardar.
public struct BorradorLeccion: Equatable {
    public var id: String?
    public var titulo = ""
    public var quePaso = ""
    public var porQue = ""
    public var proxima = ""
    public var tipo = "leccion"
    public var area = ""
    public var gravedad: Int64 = 1
    public var etiquetas: [String] = []
    public var etiquetasAuto: [String] = []
    public var quitadas: [String] = []
    public var causas: [String] = []
    public var chat: String?
    /// El mensaje del que sale una nueva («Hacer lección» de su menu).
    public var deMensaje: String?

    public init() {}

    public init(_ f: FichaLeccion) {
        id = f.id
        titulo = f.titulo
        quePaso = f.quePaso
        porQue = f.porQue
        proxima = f.proxima
        tipo = f.tipo
        area = f.area
        gravedad = f.gravedad
        etiquetas = f.etiquetas
        etiquetasAuto = f.etiquetasAuto
        quitadas = f.quitadas
        causas = f.causas
        chat = f.chat
    }

    /// Todo lo escrito junto, para el etiquetador.
    public var textoEntero: String {
        [titulo, quePaso, porQue, proxima].filter { !$0.isEmpty }.joined(separator: "\n")
    }

    /// Las etiquetas como se escriben: «obra, encofrado #prisa».
    public var etiquetasEscritas: String {
        get { etiquetas.joined(separator: ", ") }
        set {
            etiquetas = newValue
                .split(whereSeparator: { $0 == "," || $0 == "#" || $0 == "\n" })
                .map { $0.trimmingCharacters(in: .whitespaces) }
                .filter { !$0.isEmpty }
        }
    }

    /// Acepta una automatica: pasa a ser de las puestas.
    public mutating func aceptar(_ e: String) {
        etiquetasAuto.removeAll { $0 == e }
        if !etiquetas.contains(e) { etiquetas.append(e) }
    }

    /// Quita una automatica: no vuelve a proponerse.
    public mutating func quitar(_ e: String) {
        etiquetasAuto.removeAll { $0 == e }
        if !quitadas.contains(e) { quitadas.append(e) }
    }

    public var editada: LeccionEditada {
        LeccionEditada(id: id, titulo: titulo, quePaso: quePaso, porQue: porQue, proxima: proxima,
                       tipo: tipo, area: area, gravedad: gravedad, etiquetas: etiquetas,
                       etiquetasAuto: etiquetasAuto, quitadas: quitadas, causas: causas, chat: chat,
                       deMensaje: deMensaje)
    }
}
