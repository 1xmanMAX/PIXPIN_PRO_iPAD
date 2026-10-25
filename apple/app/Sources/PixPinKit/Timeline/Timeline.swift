// El timeline, como `timeline/ventana.rs` de Windows (B5): apuntar lo que
// pasa, minuto a minuto, escribiendo, con la voz o con una foto, y verlo
// como una linea de tiempo vertical.
//
// Arriba, la barra comun (`Cabecera`): el buscador y las pestanas «Hoy»,
// «Momentos», «Calendario» y «Lecciones», y «Exportar». Dentro:
//
// - **Hoy**: las ultimas 24 horas y abajo, como en un chat, la caja para
//   apuntar (↩ apunta, ⇧↩ parte el renglon, ⌘V pega texto o una imagen,
//   ↑ con la caja vacia corrige el ultimo, el microfono dicta).
// - **Momentos**: el archivo de lo mandado a «Momentos», dia a dia.
// - **Calendario** («Estado» en el codigo de Windows): una tarjeta por mes
//   con el circulo de cada dia; clic abre el dia y ⌘clic lo elige para
//   exportar varios juntos.
// - **Lecciones**: las lecciones aprendidas (la misma vista que su ventana).
// - **Un dia**, abierto encima de la pestana, y **la busqueda**, encima de
//   todo: los resultados de todo el timeline.
// - **El detalle** de un momento: la foto entera sobre negro; ←/→ pasan de
//   foto y ↑/↓ de momento.
//
// Lo que se lee y se escribe es `timeline/momentos.jsonl`, el mismo fichero
// de Windows, por el nucleo de Rust (`timeline.rs`): aqui no se decide nada
// del formato.
//
// Rendimiento (§4bis del plan): los momentos llegan por paginas, fuera del
// hilo principal; las fotos son miniaturas de ImageIO de la cache comun.
//
// En el iPad, el modelo es el del Mac tal cual (sin AppKit); la vista es
// `VistaTimeline.swift`.

import Foundation
import PixPinCore
import SwiftUI

@MainActor
public final class ModeloTimeline: ObservableObject {
    /// Las pestanas de la barra (⌘1…4).
    public enum Pestana: Int, CaseIterable, Identifiable, Sendable {
        case hoy = 1, momentos, estado, lecciones
        public var id: Int { rawValue }

        public var rotulo: String {
            switch self {
            case .hoy: "Hoy"
            case .momentos: "Momentos"
            case .estado: "Calendario"
            case .lecciones: "Lecciones"
            }
        }
    }

    /// Que se ve: todos los momentos o solo los mandados a «Momentos» (la
    /// lista de dias, `dias`).
    public enum Filtro: String, CaseIterable, Identifiable {
        case todo = "Todo"
        case momentos = "Momentos"
        public var id: String { rawValue }
    }

    /// Los momentos de la linea: «Hoy» o un dia.
    public enum Eleccion: Hashable, Sendable {
        case hoy
        case dia(String)
    }

    /// Lo que ocupa el contenido, por prioridad: lo buscado tapa el dia
    /// abierto, y el dia abierto tapa la pestana (`Modo` de Windows).
    public enum Modo: Equatable {
        case pestana(Pestana)
        case dia(String)
        case buscar
    }

    public nonisolated static let tamPaginaDias: UInt32 = 60
    public nonisolated static let tamPaginaMomentos: UInt32 = 80

    @Published public var pestana: Pestana = .hoy
    @Published public var filtro: Filtro = .todo
    @Published public private(set) var dias: [DiaTimeline] = []
    @Published public private(set) var totalDias: UInt64 = 0
    @Published public private(set) var momentos: [MomentoTimeline] = []
    @Published public private(set) var totalMomentos: UInt64 = 0
    @Published public private(set) var error: String?
    @Published public private(set) var eleccion: Eleccion = .hoy

    /// Lo escrito en el buscador de arriba y lo encontrado.
    @Published public var consulta = ""
    @Published public private(set) var encontrados: [MomentoTimeline] = []
    /// «Momentos» y «Calendario».
    @Published public private(set) var archivo: [DiaArchivo] = []
    @Published public private(set) var totalArchivo: UInt64 = 0
    @Published public private(set) var meses: [MesEstado] = []
    /// Los dias elegidos con ⌘clic en «Calendario» para exportar.
    @Published public var elegidos: Set<String> = []

    /// La caja de apuntar.
    @Published public var texto = ""
    @Published public private(set) var pegadas: [FotoNueva] = []
    /// El momento que se esta corrigiendo en la caja.
    @Published public private(set) var corrigiendo: MomentoTimeline?
    /// El aviso de abajo («Apuntado», «Momento borrado»…) y si se deshace.
    @Published public private(set) var aviso: String?
    @Published public private(set) var deshacer: String?

    let nucleo: Nucleo
    private var generacion = 0
    private var cargandoDias = false
    private var cargandoMomentos = false
    private var tareaBuscar: Task<Void, Never>?
    private var tareaAviso: Task<Void, Never>?

    public init(nucleo: Nucleo) {
        self.nucleo = nucleo
    }

    public var quedanDias: Bool { UInt64(dias.count) < totalDias }
    public var quedanMomentos: Bool { UInt64(momentos.count) < totalMomentos }
    public var quedaArchivo: Bool { UInt64(archivo.count) < totalArchivo }

    public var buscando: Bool { !consulta.trimmingCharacters(in: .whitespaces).isEmpty }

    public var modo: Modo {
        // En «Lecciones» el buscador busca en ellas.
        if buscando, pestana != .lecciones { return .buscar }
        if case let .dia(iso) = eleccion { return .dia(iso) }
        return .pestana(pestana)
    }

    // MARK: Leer

    /// Relee lo que se ve: los dias, la linea, la pestana y lo buscado.
    public func recargar() async {
        await cargarDias(desde: 0, sustituir: true)
        await recargarLinea()
        switch pestana {
        case .momentos: await cargarArchivo(desde: 0, sustituir: true)
        case .estado: await cargarEstado()
        case .hoy, .lecciones: break
        }
        if buscando { await buscar() }
    }

    public func cargarMasDias() async {
        guard quedanDias else { return }
        await cargarDias(desde: UInt64(dias.count), sustituir: false)
    }

    private func cargarDias(desde: UInt64, sustituir: Bool) async {
        guard !cargandoDias else { return }
        cargandoDias = true
        defer { cargandoDias = false }
        let nucleo = self.nucleo
        let solo = filtro == .momentos
        let desfase = desfaseLocalMs()
        do {
            let p = try await Task.detached(priority: .userInitiated) {
                try nucleo.timelineDias(desfaseMs: desfase, soloEnMomentos: solo, desde: desde,
                                        cuantos: ModeloTimeline.tamPaginaDias)
            }.value
            dias = sustituir ? p.dias : dias + p.dias
            totalDias = p.total
            error = nil
        } catch {
            self.error = "\(error)"
        }
    }

    /// Ensena «Hoy» o un dia (abierto encima de la pestana), desde su
    /// primer momento.
    public func elegir(_ e: Eleccion) async {
        eleccion = e
        await recargarLinea()
    }

    /// Cierra el dia abierto y vuelve a la pestana.
    public func volverDelDia() async {
        await elegir(.hoy)
    }

    private func recargarLinea() async {
        generacion += 1
        cargandoMomentos = false
        let mia = generacion
        let p = await pagina(desde: 0)
        guard mia == generacion else { return }
        aplicar(p, sustituir: true)
    }

    public func cargarMasMomentos() async {
        guard quedanMomentos, !cargandoMomentos else { return }
        cargandoMomentos = true
        let mia = generacion
        let p = await pagina(desde: UInt64(momentos.count))
        guard mia == generacion else { return }
        cargandoMomentos = false
        aplicar(p, sustituir: false)
    }

    private func aplicar(_ r: Result<PaginaMomentos, Error>, sustituir: Bool) {
        switch r {
        case let .success(p):
            momentos = sustituir ? p.momentos : momentos + p.momentos
            totalMomentos = p.total
            error = nil
        case let .failure(e):
            error = "\(e)"
        }
    }

    private func pagina(desde: UInt64) async -> Result<PaginaMomentos, Error> {
        let nucleo = self.nucleo
        let solo = filtro == .momentos
        let desfase = desfaseLocalMs()
        let eleccion = self.eleccion
        return await Task.detached(priority: .userInitiated) { () -> Result<PaginaMomentos, Error> in
            Result {
                switch eleccion {
                case .hoy:
                    return try nucleo.timelineUltimas24h(desfaseMs: desfase, desde: desde,
                                                         cuantos: ModeloTimeline.tamPaginaMomentos)
                case let .dia(iso):
                    guard let f = ModeloTimeline.partes(iso) else {
                        return PaginaMomentos(momentos: [], desde: 0, total: 0)
                    }
                    return try nucleo.timelineMomentos(anio: f.0, mes: f.1, dia: f.2, desfaseMs: desfase,
                                                       soloEnMomentos: solo, desde: desde,
                                                       cuantos: ModeloTimeline.tamPaginaMomentos)
                }
            }
        }.value
    }

    /// «Momentos»: los dias del archivo, por paginas.
    public func cargarArchivo(desde: UInt64 = 0, sustituir: Bool = true) async {
        let nucleo = self.nucleo
        let desfase = desfaseLocalMs()
        do {
            let p = try await Task.detached(priority: .userInitiated) {
                try nucleo.timelineArchivo(desfaseMs: desfase, desde: desde,
                                           cuantos: ModeloTimeline.tamPaginaDias)
            }.value
            archivo = sustituir ? p.dias : archivo + p.dias
            totalArchivo = p.total
        } catch {
            self.error = "\(error)"
        }
    }

    public func cargarMasArchivo() async {
        guard quedaArchivo else { return }
        await cargarArchivo(desde: UInt64(archivo.count), sustituir: false)
    }

    /// «Calendario»: las tarjetas de los meses.
    public func cargarEstado() async {
        let nucleo = self.nucleo
        let desfase = desfaseLocalMs()
        do {
            meses = try await Task.detached(priority: .userInitiated) {
                try nucleo.timelineEstado(desfaseMs: desfase)
            }.value
        } catch {
            self.error = "\(error)"
        }
    }

    /// Cambia de pestana tal cual: sin la busqueda ni el dia que tapaban lo
    /// de debajo. Los dias elegidos solo valen en «Calendario».
    public func cambiar(a p: Pestana) async {
        if p != pestana { consulta = "" }
        pestana = p
        if p != .estado { elegidos = [] }
        if case .dia = eleccion { await elegir(.hoy) }
        await recargar()
    }

    /// Lo escrito en el buscador, con una espera corta (cada letra cancela
    /// la busqueda anterior).
    public func consultaCambiada() {
        tareaBuscar?.cancel()
        tareaBuscar = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(150))
            guard !Task.isCancelled else { return }
            await self?.buscar()
        }
    }

    public func buscar() async {
        let q = consulta.trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else {
            encontrados = []
            return
        }
        let nucleo = self.nucleo
        let desfase = desfaseLocalMs()
        let hallados = (try? await Task.detached(priority: .userInitiated) {
            try nucleo.timelineBuscar(consulta: q, desfaseMs: desfase, cuantos: 500)
        }.value) ?? []
        guard q == consulta.trimmingCharacters(in: .whitespaces) else { return }
        encontrados = hallados
    }

    /// Lo que ensena la lista ahora (el orden que siguen las flechas del
    /// detalle).
    public var visibles: [MomentoTimeline] {
        switch modo {
        case .buscar: return encontrados
        case .dia: return momentos
        case .pestana(.hoy): return momentos
        case .pestana(.momentos): return archivo.flatMap(\.momentos)
        case .pestana: return []
        }
    }

    // MARK: Apuntar

    /// Intro: apunta lo de la caja (con la voz dictada que esperaba, si la
    /// hay), o guarda la correccion. Devuelve si se hizo algo.
    @discardableResult
    public func enviar(audio: NotaGrabada? = nil) async -> Bool {
        let escrito = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        if let c = corrigiendo {
            let nucleo = self.nucleo
            let desfase = desfaseLocalMs()
            do {
                _ = try await Task.detached(priority: .userInitiated) {
                    try nucleo.timelineCorregir(id: c.id, texto: escrito, desfaseMs: desfase)
                }.value
                texto = ""
                corrigiendo = nil
                avisar("Corregido")
                await recargar()
                return true
            } catch {
                avisar(ModeloTimeline.esArgumento(error) ? "Escribe algo para el título" : "No se pudo guardar el timeline")
                return false
            }
        }
        guard !escrito.isEmpty || !pegadas.isEmpty || audio != nil else { return false }
        let fotos = pegadas
        if await apuntar(texto: escrito, fotos: fotos, audio: audio) {
            texto = ""
            pegadas = []
            return true
        }
        return false
    }

    /// Apunta un momento nuevo y vuelve a «Hoy» (para verlo).
    @discardableResult
    public func apuntar(texto: String, fotos: [FotoNueva], audio: NotaGrabada?, dicho: String? = nil,
                        cuando: Int64? = nil) async -> Bool {
        let nucleo = self.nucleo
        let desfase = desfaseLocalMs()
        let ruta = audio?.url.path
        let ms = audio?.duracionMs ?? 0
        do {
            _ = try await Task.detached(priority: .userInitiated) {
                try nucleo.timelineApuntar(texto: texto, fotos: fotos, audio: ruta, duracionMs: ms,
                                           dicho: dicho, cuando: cuando, desfaseMs: desfase)
            }.value
            consulta = ""
            if pestana != .hoy || eleccion != .hoy {
                pestana = .hoy
                eleccion = .hoy
            }
            await recargar()
            return true
        } catch {
            avisar("No se pudo guardar el timeline")
            self.error = "\(error)"
            return false
        }
    }

    /// Una foto a la caja: elegida (se copia) o pegada (un temporal, se
    /// mueve al apuntar).
    public func anadirFoto(_ url: URL, temporal: Bool) {
        pegadas.append(FotoNueva(ruta: url.path, temporal: temporal))
    }

    public func quitarPegada(_ i: Int) {
        guard pegadas.indices.contains(i) else { return }
        let p = pegadas.remove(at: i)
        if p.temporal { try? FileManager.default.removeItem(atPath: p.ruta) }
    }

    /// Vacia la caja (y deja de corregir).
    public func vaciarCaja() {
        texto = ""
        for p in pegadas where p.temporal { try? FileManager.default.removeItem(atPath: p.ruta) }
        pegadas = []
        corrigiendo = nil
    }

    /// Pone un momento en la caja para corregirlo (se corrige en la caja de
    /// «Hoy», que es la unica).
    public func corregir(_ m: MomentoTimeline) async {
        vaciarCaja()
        texto = m.descripcion.isEmpty ? m.titulo : "\(m.titulo)\n\(m.descripcion)"
        corrigiendo = m
        if modo != .pestana(.hoy) {
            consulta = ""
            pestana = .hoy
            await elegir(.hoy)
        }
    }

    /// ↑ con la caja vacia: corrige el ultimo, como en Telegram.
    public func corregirElUltimo() async {
        guard texto.isEmpty, corrigiendo == nil, let ultimo = momentos.last else { return }
        await corregir(ultimo)
    }

    // MARK: Cambiar uno

    public func borrar(_ m: MomentoTimeline) async {
        let nucleo = self.nucleo
        do {
            let linea = try await Task.detached(priority: .userInitiated) {
                try nucleo.timelineBorrar(id: m.id)
            }.value
            if corrigiendo?.id == m.id { vaciarCaja() }
            avisar("Momento borrado", deshacer: linea)
            await recargar()
        } catch {
            self.error = "\(error)"
        }
    }

    /// ⌘Z mientras dura el aviso del borrado.
    public func deshacerBorrado() async {
        guard let linea = deshacer else { return }
        let nucleo = self.nucleo
        deshacer = nil
        aviso = nil
        try? await Task.detached(priority: .userInitiated) { try nucleo.timelineDevolver(linea: linea) }.value
        await recargar()
    }

    /// Lo manda a «Momentos», o lo saca.
    public func alternarMomentos(_ m: MomentoTimeline) async {
        let nucleo = self.nucleo
        do {
            let dentro = try await Task.detached(priority: .userInitiated) {
                try nucleo.timelineAMomentos(id: m.id)
            }.value
            avisar(dentro ? "Mandado a Momentos" : "Quitado de Momentos")
            await recargar()
        } catch {
            self.error = "\(error)"
        }
    }

    /// La bombilla: lo hace leccion, o lo desmarca (la leccion se queda).
    public func alternarLeccion(_ m: MomentoTimeline) async {
        let nucleo = self.nucleo
        do {
            let l = try await Task.detached(priority: .userInitiated) {
                try nucleo.timelineLeccion(id: m.id)
            }.value
            avisar(l != nil ? "💡 Hecha lección: está en «Lecciones»"
                : "Ya no está marcado. La lección sigue en «Lecciones».")
            await recargar()
        } catch {
            avisar("No se pudo guardar la lección")
        }
    }

    // MARK: Exportar

    /// Lo que se exporta segun lo que se ve: lo buscado, el dia abierto, en
    /// «Momentos» y «Calendario» los dias elegidos (o sin eleccion, los del
    /// mes de hoy que tienen algo) y, si no, las ultimas 24 horas.
    public func queExportar() -> ExportarTimeline {
        switch modo {
        case .buscar:
            return .buscar(consulta: consulta)
        case let .dia(iso):
            return .dias(isos: [iso])
        case .pestana(.momentos), .pestana(.estado):
            if pestana == .estado, !elegidos.isEmpty { return .dias(isos: elegidos.sorted()) }
            let mes = meses.first
            let isos = mes?.dias.filter { $0.cuantos > 0 }.map(\.iso) ?? []
            return isos.isEmpty ? .ultimas24h : .dias(isos: isos)
        case .pestana:
            return .ultimas24h
        }
    }

    /// La pagina de lo que se ve (HTML con todo dentro).
    public func paginaParaExportar() async throws -> PaginaTimeline {
        if pestana == .estado, meses.isEmpty { await cargarEstado() }
        let que = queExportar()
        let nucleo = self.nucleo
        let desfase = desfaseLocalMs()
        return try await Task.detached(priority: .userInitiated) {
            try nucleo.timelineExportar(que: que, desfaseMs: desfase)
        }.value
    }

    // MARK: Avisos

    /// El aviso de abajo, unos segundos (con «Deshacer», un poco mas).
    public func avisar(_ t: String, deshacer linea: String? = nil) {
        aviso = t
        deshacer = linea
        tareaAviso?.cancel()
        tareaAviso = Task { [weak self] in
            try? await Task.sleep(for: .seconds(linea == nil ? 2.5 : 6))
            guard !Task.isCancelled else { return }
            self?.aviso = nil
            self?.deshacer = nil
        }
    }

    nonisolated static func esArgumento(_ e: Error) -> Bool {
        if case .Argumento = e as? ErrorNucleo { return true }
        return false
    }

    /// `AAAA-MM-DD` en sus tres numeros.
    nonisolated static func partes(_ iso: String) -> (Int32, UInt8, UInt8)? {
        let p = iso.split(separator: "-")
        guard p.count == 3, let a = Int32(p[0]), let m = UInt8(p[1]), let d = UInt8(p[2]) else { return nil }
        return (a, m, d)
    }

    /// «jueves, 15 de octubre de 2026» de un `AAAA-MM-DD`.
    nonisolated static func rotulo(iso: String) -> String {
        guard let (a, m, d) = partes(iso) else { return iso }
        return rotulo(DiaTimeline(anio: a, mes: m, dia: d, iso: iso, cuantos: 0))
    }

    /// «jueves, 15 de octubre de 2026» en el idioma del sistema.
    nonisolated static func rotulo(_ d: DiaTimeline) -> String {
        var c = DateComponents()
        c.year = Int(d.anio)
        c.month = Int(d.mes)
        c.day = Int(d.dia)
        c.hour = 12
        guard let f = Calendar.current.date(from: c) else { return d.iso }
        return f.formatted(.dateTime.weekday(.wide).day().month(.wide).year())
    }

    /// El dia (`AAAA-MM-DD`, en el huso de aqui) de un momento.
    nonisolated static func iso(de ms: Int64) -> String {
        let c = Calendar.current.dateComponents([.year, .month, .day], from: fechaDe(ms: ms))
        return String(format: "%04d-%02d-%02d", c.year ?? 0, c.month ?? 0, c.day ?? 0)
    }
}
