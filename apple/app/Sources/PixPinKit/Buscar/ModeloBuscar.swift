// **Buscar en PixPin** (D8, `buscar_todo.rs` de Windows): busca en todo
// —chats, mensajes y notas, ficheros por su nombre y por lo que pone dentro
// (PDF, Word, PowerPoint...), tareas, lecciones, capturas por su nombre, su
// fecha y su texto, y los momentos del timeline—. El modelo es el del Mac
// tal cual (la logica de la lista esta en `EstadoBuscar.swift`); la vista
// del iPad, en `VistaBuscar.swift`.
//
// Rendimiento (§4bis del plan):
// - se espera a que se deje de escribir (200 ms) antes de buscar, y cada
//   letra nueva CANCELA la busqueda anterior: la `Task` de Swift y, en Rust,
//   el `Busqueda` que mira en cada linea si tiene que parar;
// - cada clase llega hasta un tope (`tamPagina`); al llegar al final de la
//   lista se pide con el doble (`cargarMas`);
// - el texto de los documentos y de las capturas se lee en segundo plano
//   (`LectorTextoDocumentos`, `LectorTextoGaleria`) y se vuelve a buscar
//   cuando llega algo nuevo.

import Foundation
import PixPinCore
import SwiftUI

@MainActor
public final class ModeloBuscar: ObservableObject {
    /// El tope de cada clase en la primera vuelta.
    public nonisolated static let tamPagina: UInt32 = 50
    /// Lo que se espera desde la ultima letra antes de buscar.
    public nonisolated static let espera: Duration = .milliseconds(200)

    @Published public var estado = EstadoBuscar()
    @Published public private(set) var busquedas: [String] = []
    @Published public private(set) var buscando = false
    @Published public private(set) var error: String?
    /// Las clases que se cortaron en el tope.
    @Published public private(set) var recortadas: Set<ClaseDeResultado> = []
    /// El aviso de abajo («Copiado», «Captura borrada»).
    @Published public var aviso: String?

    let almacen: Almacen
    /// La busqueda en curso (la espera y la llamada a Rust).
    private(set) var tarea: Task<Void, Never>?
    private var enRust: Busqueda?
    private var tope = ModeloBuscar.tamPagina
    /// Las capturas borradas, para ⌘Z.
    var borradas: [CapturaBorrada] = []
    private var finAviso: Task<Void, Never>?

    public init(almacen: Almacen) {
        self.almacen = almacen
    }

    // Lo de siempre, para quien ya lo usaba (y sus pruebas).
    public var consulta: String { estado.consulta }
    public var resultados: [ResultadoBusqueda] { estado.resultados }
    public var hayMas: Bool { !recortadas.isEmpty }

    /// Lo escrito cambio: cancela lo anterior y busca tras la espera.
    public func cambiar(_ texto: String) {
        estado.consulta = texto
        estado.menu = nil
        tope = ModeloBuscar.tamPagina
        cancelar()
        buscando = true
        tarea = Task { [weak self] in
            try? await Task.sleep(for: ModeloBuscar.espera)
            guard !Task.isCancelled else { return }
            await self?.buscarAhora(texto)
        }
    }

    /// Cambia de pestana (con la caja vacia, cada una pregunta lo suyo).
    public func ir(a p: PestanaBuscar) {
        if estado.ir(a: p) == .buscar {
            refrescar()
        }
    }

    /// Vuelve a buscar lo de ahora, ya.
    public func refrescar(mantener: Bool = false) {
        cancelar()
        let texto = estado.consulta
        tarea = Task { [weak self] in await self?.buscarAhora(texto, mantener: mantener) }
    }

    /// Para lo que este en marcha (tambien la busqueda en Rust).
    public func cancelar() {
        tarea?.cancel()
        tarea = nil
        enRust?.cancelar()
        enRust = nil
    }

    /// Busca ya, sin esperar.
    public func buscarAhora(_ texto: String, mantener: Bool = false) async {
        estado.consulta = texto
        let b = Busqueda.nueva()
        enRust = b
        buscando = true
        let pedido = pedidoDe(texto)
        // «carpeta …»: las carpetas de todos los chats (Windows 22c3ce2).
        if let resto = AccionBuscar.consultaDeCarpetas(texto) {
            let nucleo = almacen.nucleo
            estado.carpetas = await Task.detached(priority: .userInitiated) {
                try? nucleo.carpetasParaFlow(texto: resto)
            }.value
        } else {
            estado.carpetas = nil
        }
        let hecho = await pedir(pedido, busqueda: b)
        // Si mientras tanto se escribio otra cosa, esto ya no vale.
        guard enRust === b, !Task.isCancelled else { return }
        buscando = false
        enRust = nil
        guard let hecho, !hecho.cancelada else { return }
        recortadas = Set(hecho.recortadas)
        estado.poner(hecho.resultados, mantener: mantener)
    }

    /// Al llegar al final de la lista: lo mismo con el doble de tope.
    public func cargarMas() async {
        guard hayMas, enRust == nil, tope < 1000 else { return }
        tope = min(tope * 2, 1000)
        await buscarAhora(estado.consulta, mantener: true)
    }

    /// Que se le pide al nucleo para `texto` en la pestana de ahora.
    enum Pedido {
        /// La caja vacia en «Todo»: lo abierto hace poco.
        case inicio
        /// Nada (una letra que no busca: «t pan» solo apunta).
        case nada
        case buscar(OpcionesBusqueda)
    }

    func pedidoDe(_ texto: String) -> Pedido {
        let limpio = texto.trimmingCharacters(in: .whitespaces)
        let dias = Int64(AjusteCaducidad.dias)
        func opciones(_ q: String, _ clases: [ClaseDeResultado]) -> Pedido {
            .buscar(OpcionesBusqueda(consulta: q, clases: clases, tope: tope,
                                     diasCaducidad: dias, desfaseMs: desfaseLocalMs()))
        }
        // «carpeta diario»: solo las carpetas, como `p carpeta` de Flow.
        if AccionBuscar.soloCarpetas(texto) { return .nada }
        if let (letra, resto) = LetrasRapidas.analizar(texto) {
            // «g muro» busca en las capturas; las demas letras no buscan.
            return letra == "g" ? opciones(resto, [.captura]) : .nada
        }
        if limpio.isEmpty {
            switch estado.pestana {
            case .todo: return .inicio
            case .acciones: return .nada
            case let p: return opciones("", p.clases)
            }
        }
        // Con algo escrito se busca en todo: las pestanas solo filtran (y
        // asi ensenan su cuenta).
        return opciones(limpio, [])
    }

    private func pedir(_ p: Pedido, busqueda: Busqueda) async -> ResultadosEnTodo? {
        let nucleo = almacen.nucleo
        switch p {
        case .nada:
            return ResultadosEnTodo(resultados: [], recortadas: [], cancelada: false)
        case .inicio:
            let dias = Int64(AjusteCaducidad.dias)
            let desfase = desfaseLocalMs()
            let (inicio, recientes) = await Task.detached(priority: .userInitiated) {
                (nucleo.buscarInicio(diasCaducidad: dias, desfaseMs: desfase), nucleo.buscarRecientes())
            }.value
            busquedas = recientes.busquedas
            return ResultadosEnTodo(resultados: inicio, recortadas: [], cancelada: false)
        case let .buscar(o):
            let hecho = await withTaskCancellationHandler {
                await Task.detached(priority: .userInitiated) { () -> Result<ResultadosEnTodo, Error> in
                    Result(catching: { try nucleo.buscarEnTodo(opciones: o, busqueda: busqueda) })
                }.value
            } onCancel: {
                busqueda.cancelar()
            }
            switch hecho {
            case let .success(r):
                error = nil
                return r
            case let .failure(e):
                error = "\(e)"
                return nil
            }
        }
    }

    // MARK: Recientes

    /// Apunta lo elegido: la busqueda en las recientes y el resultado en
    /// «Abierto hace poco».
    func recordar(_ f: FilaBuscar) {
        let nucleo = almacen.nucleo
        let consulta = estado.consulta
        let abierto = f.resultado
        Task.detached(priority: .utility) {
            try? nucleo.buscarApuntar(consulta: consulta, abierto: abierto)
        }
    }

    func quitarBusqueda(_ i: Int) {
        try? almacen.nucleo.buscarQuitarBusqueda(indice: UInt32(i))
        busquedas = almacen.nucleo.buscarRecientes().busquedas
    }

    func borrarBusquedas() {
        try? almacen.nucleo.buscarBorrarBusquedas()
        busquedas = []
    }

    // MARK: Avisos

    /// Un aviso abajo; con algo que deshacer dura mas (como Windows: 6 s).
    func avisar(_ texto: String) {
        aviso = texto
        finAviso?.cancel()
        let largo: Duration = estado.seDeshace ? .seconds(6) : .seconds(2)
        finAviso = Task { [weak self] in
            try? await Task.sleep(for: largo)
            guard !Task.isCancelled, let self else { return }
            self.aviso = nil
            self.estado.seDeshace = false
            self.borradas = []
        }
    }
}
