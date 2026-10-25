// El visor de planos DWG y DXF del iPad (`ventana.rs` de Windows,
// `VisorPlano.swift` del Mac) con Metal y el tacto:
//
// - pellizcar acerca donde estan los dedos, uno o dos dedos mueven el plano
//   (midiendo o marcando, dos), doble toque lo ve entero;
// - la regla con enganche (`Plano.enganchar`): cada toque pone un punto (o
//   se arrastra un dedo y se suelta donde se quiere); doble toque o «✓» la
//   guarda como cota en la capa que viaja;
// - los marcos para imprimir (arrastrar), las capas (encender y apagar) y
//   el modo noche comun (`ModoNoche`);
// - anotar con todas las herramientas del lienzo (`AnotarPlanoIPad.swift`);
// - imprimir los marcos (con lamina y membrete o sin marco) y exportar en
//   HTML, PDF, PNG, JPG y DXF (`PapelPlanoIPad.swift`);
// - buscar en los textos del plano, las letras SHX del usuario («Aa») y el
//   aviso de los objetos de Civil 3D guardados sin su dibujo;
// - los marcadores con su vista (`MarcadoresPlanoIPad.swift`).
//
// **Solo se pinta cuando algo cambia** (§4bis): la `MTKView` esta parada
// (`isPaused`) y pinta al pedirselo (`enableSetNeedsDisplay`). Lo de encima
// (cotas, la mira, la tinta) va en una vista transparente con Core Graphics,
// que tambien se pinta solo al cambiar.
//
// Las coordenadas: la camara del plano (`CamaraPlano`) cuenta los pixeles
// desde abajo a la izquierda (la y del dibujo sube); UIKit los cuenta desde
// arriba, asi que aqui se da la vuelta a la y al pasar de uno a otro.

import Combine
import Metal
import MetalKit
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Que hacen el dedo y el lapiz en el visor.
public enum ModoVisorPlano: String, CaseIterable, Sendable {
    /// Arrastrar mueve el plano.
    case mover
    /// Cada toque pone un punto de la regla (con enganche); doble toque la
    /// termina y la guarda como cota.
    case medir
    /// Arrastrar dibuja un marco para imprimir.
    case marco
}

/// **El estado del visor de un plano**: lo leido, las capas, lo anotado y lo
/// que se esta midiendo. La vista de Metal lo pinta.
@MainActor
final class VisorPlano: ObservableObject {
    @Published private(set) var datos: PlanoEnTarjeta?
    @Published private(set) var error: String?
    @Published private(set) var capas: [CapaPlano] = []
    @Published private(set) var anotado = AnotadoPlano(cotas: [], marcos: [])
    @Published var modo: ModoVisorPlano = .mover {
        didSet {
            guard modo != oldValue else { return }
            // Medir y marcar no van a la vez que anotar.
            if modo != .mover, anotador != nil { terminarDeAnotar() }
            vista?.cambioDeModo()
        }
    }
    /// El panel de las capas.
    @Published var verCapas = false
    /// La medida de la regla en curso (o una ayuda), para la barra.
    @Published private(set) var medida = ""
    /// Cuantos puntos lleva la regla en curso.
    @Published private(set) var puntosEnCurso = 0
    /// Anotando con el lapiz encima.
    @Published private(set) var anotando = false
    /// El modelo del lienzo mientras se anota (sus barras).
    @Published private(set) var modeloAnotar: ModeloEditorLienzo?
    /// El buscador de textos a la vista, lo hallado y si se busca.
    @Published var buscando = false
    @Published private(set) var hallados: [TextoHallado] = []
    @Published private(set) var buscandoAhora = false
    @Published private(set) var sinResultado = false
    /// El texto hallado que se ensena (con una marca encima).
    @Published private(set) var hallado: TextoHallado?
    /// La hoja de las letras («Aa»).
    @Published var verLetras = false
    /// Las letras que el plano pide y no estan.
    @Published private(set) var faltan: [String] = []
    /// Cuantos objetos de Civil 3D se guardaron sin su dibujo
    /// (`PROXYGRAPHICS = 0`), y si el aviso se ve.
    @Published private(set) var sinDibujoCivil: UInt32 = 0
    @Published var avisoCivil = false
    /// Algo se esta preparando para exportar o imprimir.
    @Published private(set) var preparando = false
    /// Se ha visto un Apple Pencil: desde entonces solo el pinta (la palma no).
    @Published var vistoLapiz = false

    let titulo: String
    let ruta: String
    /// Los grupos que se ven.
    private(set) var visibles: Set<UInt32> = []
    /// Lo anotado con el lapiz (la capa del lienzo), para verlo.
    private(set) var tinta: LienzoLeido?
    private(set) var encaje: EncajePlano?
    /// Los textos de cada tramo de cada cota.
    private(set) var etiquetas: [[String]] = []
    weak var vista: VistaPlanoMetal?
    private(set) var anotador: AnotadorPlanoIPad?
    /// El nucleo con que se abrio (letras, el lienzo, los marcadores).
    private(set) var nucleo: Nucleo?
    /// Para no pisar una busqueda con la anterior.
    private var turnoBusqueda = 0
    /// Los marcadores con su vista (`.vistas.json` de Windows).
    private(set) lazy var marcadores = MarcadoresDeVista(documento: URL(fileURLWithPath: ruta), nucleo: nucleo)

    init(ruta: String, titulo: String) {
        self.ruta = ruta
        self.titulo = titulo
    }

    /// El fondo: el modo noche comun de los visores.
    var oscuro: Bool { ModoNoche.compartido.activo }

    /// Lee el plano fuera del hilo principal y lo sube a la tarjeta.
    func cargar(nucleo: Nucleo) {
        guard datos == nil, error == nil else { return }
        guard let motor = MotorMetal.compartido else {
            error = "Este iPad no tiene una tarjeta gráfica con Metal."
            return
        }
        self.nucleo = nucleo
        let ruta = self.ruta
        Task {
            let hecho = await Task.detached(priority: .userInitiated) { () -> Result<PlanoEnTarjeta, Error> in
                Result { try PlanoEnTarjeta.cargar(ruta: ruta, nucleo: nucleo, motor: motor) }
            }.value
            switch hecho {
            case let .success(d): self.poner(d)
            case let .failure(e): self.error = "No se pudo abrir el plano: \(e)"
            }
        }
    }

    private func poner(_ d: PlanoEnTarjeta) {
        datos = d
        visibles = Set(d.plano.gruposVisibles())
        capas = d.plano.capas()
        anotado = d.plano.anotado()
        faltan = d.plano.letrasQueFaltan()
        sinDibujoCivil = VisorPlano.cuantosSinDibujo(d.resumen.sinDibujar)
        avisoCivil = sinDibujoCivil > 0
        rehacerEtiquetas()
        leerTinta()
        vista?.alLlegarElPlano()
        aplicarZonaPendiente()
        if let q = buscarPendiente {
            buscarPendiente = nil
            buscando = true
            buscar(q)
        }
    }

    // MARK: Abrir mirando a una zona (el asistente: `abrir` con `zona`)

    /// La zona pedida que aun no se pudo mirar (el plano o la vista no
    /// estaban), y el texto pedido que buscar al llegar el plano.
    private var zonaPendiente: (x: Double, y: Double, ancho: Double)?
    private(set) var buscarPendiente: String?

    /// **Lo que pide un pedido** (`IrEnElLector`): mirar a una zona del
    /// dibujo (el centro y lo que cabe a lo ancho) y, si lo dice, buscar.
    func pedir(_ p: IrEnElLector.Peticion) {
        if let z = p.zona {
            zonaPendiente = z
            aplicarZonaPendiente()
        }
        if let q = p.buscar {
            if datos != nil {
                buscando = true
                buscar(q)
            } else {
                buscarPendiente = q
            }
        }
    }

    /// Mira a la zona pedida si ya se puede (el plano leido y la vista con
    /// su tamano); si no, queda para luego.
    func aplicarZonaPendiente() {
        guard let z = zonaPendiente, datos != nil, let v = vista, v.anchoPx > 1, v.altoPx > 1 else { return }
        zonaPendiente = nil
        v.mirar(centroX: z.x, centroY: z.y, ancho: z.ancho)
    }

    /// Los objetos de otras aplicaciones (Civil 3D...) guardados sin su
    /// dibujo de reserva: Rust los apunta con «sin dibujo: <clase>».
    nonisolated static func cuantosSinDibujo(_ l: [NoDibujado]) -> UInt32 {
        l.filter { $0.tipo.hasPrefix("sin dibujo: ") }.reduce(0) { $0 + $1.cuantos }
    }

    /// El aviso de Civil 3D, con los textos de Windows.
    var textoAvisoCivil: String {
        "\(sinDibujoCivil) objetos de Civil 3D se guardaron sin su dibujo: guárdalo con PROXYGRAPHICS = 1 o exporta a LandXML"
    }

    /// **Vuelve a leer el plano** (con letras nuevas): fuera lo de la
    /// tarjeta y otra vez desde el principio (la cache lo lee de nuevo).
    func releer() {
        guard let nucleo else { return }
        if anotador != nil { terminarDeAnotar() }
        datos = nil
        error = nil
        capas = []
        visibles = []
        tinta = nil
        hallados = []
        hallado = nil
        vista?.repintar()
        cargar(nucleo: nucleo)
    }

    /// Lo anotado con el lapiz (aqui, en Windows o en Android) se ve encima.
    private func leerTinta() {
        guard let d = datos else { return }
        let ruta = d.plano.rutaDeLaCapa()
        encaje = d.plano.encaje()
        guard FileManager.default.fileExists(atPath: ruta) else {
            tinta = nil
            vista?.repintar()
            return
        }
        Task {
            let leida = await Task.detached(priority: .utility) { () -> LienzoLeido? in
                guard let l = try? Lienzo.deFichero(ruta: ruta) else { return nil }
                return try? leerLienzo(l.plano())
            }.value
            self.tinta = leida
            self.vista?.repintar()
        }
    }

    private func rehacerEtiquetas() {
        guard let p = datos?.plano else { etiquetas = []; return }
        etiquetas = anotado.cotas.map { c in
            zip(c.puntos, c.puntos.dropFirst()).map { a, b in p.textoDeMedida(distancia: hypot(b.x - a.x, b.y - a.y)) }
        }
    }

    // MARK: Capas

    func ponerCapa(_ nombre: String, encendida: Bool) {
        guard let p = datos?.plano, p.ponerCapa(nombre: nombre, encendida: encendida) else { return }
        visibles = Set(p.gruposVisibles())
        capas = p.capas()
        vista?.repintar()
    }

    func todasLasCapas(_ encendidas: Bool) {
        guard let p = datos?.plano else { return }
        for c in capas { _ = p.ponerCapa(nombre: c.nombre, encendida: encendidas) }
        visibles = Set(p.gruposVisibles())
        capas = p.capas()
        vista?.repintar()
    }

    // MARK: Regla, cotas y marcos

    /// El texto de una medida (con la unidad del plano).
    func texto(_ d: Double) -> String { datos?.plano.textoDeMedida(distancia: d) ?? String(format: "%.3f", d) }

    func ponerMedida(_ t: String, puntos: Int) {
        if medida != t { medida = t }
        if puntosEnCurso != puntos { puntosEnCurso = puntos }
    }

    /// Guarda en la capa (la que viaja) una cota nueva.
    func anadirCota(_ puntos: [PuntoPlano]) {
        guard puntos.count >= 2 else { return }
        var a = anotado
        a.cotas.append(CotaPlano(puntos: puntos))
        guardar(a)
    }

    func anadirMarco(_ c: CajaPlano) {
        var a = anotado
        a.marcos.append(c)
        guardar(a)
    }

    func borrarCotas() {
        var a = anotado
        a.cotas = []
        guardar(a)
    }

    func borrarMarcos() {
        var a = anotado
        a.marcos = []
        guardar(a)
    }

    func quitarUltimoMarco() {
        guard !anotado.marcos.isEmpty else { return }
        var a = anotado
        a.marcos.removeLast()
        guardar(a)
    }

    /// Una ficha de abajo: mira el marco `i` entero, con un margen.
    func irAMarco(_ i: Int) {
        guard i < anotado.marcos.count, let v = vista else { return }
        let m = anotado.marcos[i]
        let ancho = max(m.x1 - m.x0, (m.y1 - m.y0) * v.anchoPx / max(v.altoPx, 1)) * 1.1
        v.mirar(centroX: (m.x0 + m.x1) / 2, centroY: (m.y0 + m.y1) / 2, ancho: ancho)
    }

    /// Lo anotado se ve al momento y se escribe fuera del hilo principal.
    private func guardar(_ a: AnotadoPlano) {
        guard let p = datos?.plano else { return }
        anotado = a
        rehacerEtiquetas()
        vista?.repintar()
        Task.detached(priority: .utility) {
            _ = try? p.guardarAnotado(anotado: a)
            await MainActor.run { NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil) }
        }
    }

    func encuadrar() { vista?.encuadrar() }

    // MARK: Imprimir y exportar

    /// Las cotas y lo anotado, para el papel (imprimir y exportar).
    func encimaParaPapel() -> EncimaPapel {
        let p = datos?.plano
        return EncimaPapel(cotas: anotado.cotas,
                           medida: { d in p?.textoDeMedida(distancia: d) ?? String(format: "%.3f", d) },
                           tinta: tinta, encaje: encaje)
    }

    /// Primero lo anotado, guardado y leido (va en el papel); luego `hacer`.
    private func conLoAnotado(_ hacer: @escaping () -> Void) {
        if anotador != nil { terminarDeAnotar(hacer) } else { hacer() }
    }

    /// **Imprimir los marcos** (`Ctrl P` de Windows): cada marco en su hoja
    /// (sin marcos, lo que se ve), con la lamina y el membrete o sin marco.
    /// Va en vectores por la hoja de imprimir del sistema.
    func imprimir(conMembrete: Bool) {
        conLoAnotado { [weak self] in
            guard let self, let d = self.datos, let v = self.vista else { return }
            let marcos = self.anotado.marcos.isEmpty ? [v.cajaVisible()] : self.anotado.marcos
            let lamina = LaminaPlano(conMembrete: conMembrete, proyecto: CuentasLamina.proyecto(de: self.ruta),
                                     titulo: (self.titulo as NSString).deletingPathExtension, autor: "",
                                     fecha: CuentasLamina.hoy())
            let impresora = UIPrintInteractionController.shared
            let info = UIPrintInfo.printInfo()
            info.jobName = self.titulo
            info.outputType = .general
            if let m = marcos.first { info.orientation = (m.x1 - m.x0) >= (m.y1 - m.y0) ? .landscape : .portrait }
            impresora.printInfo = info
            impresora.printingItem = nil
            impresora.printingItems = nil
            impresora.printPageRenderer = ImpresoraPlano(datos: d, visibles: self.visibles, marcos: marcos,
                                                         encima: self.encimaParaPapel(), lamina: lamina)
            // Al cerrar, fuera el pintor: la hoja de imprimir es compartida.
            _ = impresora.present(from: CGRect(x: v.bounds.maxX - 60, y: 60, width: 1, height: 1), in: v,
                                  animated: true) { c, _, _ in c.printPageRenderer = nil }
        }
    }

    /// Los formatos que este plano puede dar (el DXF, solo de un DWG).
    var formatosDeExportar: [ExportarPlanoIPad.Formato] {
        ExportarPlanoIPad.Formato.allCases.filter { $0 != .dxf || (datos?.plano.esDwg() ?? false) }
    }

    /// **Exportar el plano** (`Ctrl E` de Windows) en `formato`, con las
    /// capas como se ven, las cotas, los marcos y lo anotado; sale la hoja
    /// de compartir del sistema (Guardar en Archivos, AirDrop, Mail...).
    func exportar(_ formato: ExportarPlanoIPad.Formato) {
        conLoAnotado { [weak self] in self?.exportarYa(formato) }
    }

    private func exportarYa(_ formato: ExportarPlanoIPad.Formato) {
        guard let d = datos, !preparando else { return }
        let base = ExportarPlanoIPad.base(titulo)
        let carpeta = ExportarPlanoIPad.carpetaTemporal()
        let vista = self.vista
        func listo(_ urls: [URL]) {
            preparando = false
            if urls.isEmpty {
                try? FileManager.default.removeItem(at: carpeta)
                CajetinesLienzo.avisar("Exportar", "No se pudo preparar el \(formato.rawValue.uppercased()).",
                                       desde: (vista as UIView?) ?? UIView())
                return
            }
            ExportarPlanoIPad.compartir(urls, carpeta: carpeta, desde: vista)
        }
        preparando = true
        let plano = d.plano
        switch formato {
        case .html:
            let anotado = self.anotado, claro = !oscuro
            let salida = carpeta.appendingPathComponent(base + ".html")
            Task {
                let ok = await Task.detached(priority: .userInitiated) { () -> Bool in
                    guard let html = try? plano.paginaWeb(anotado: anotado, claro: claro) else { return false }
                    return (try? Data(html.utf8).write(to: salida)) != nil
                }.value
                listo(ok ? [salida] : [])
            }
        case .pdf:
            let salida = carpeta.appendingPathComponent(base + ".pdf")
            let datos = ExportarPlanoIPad.pdf(d, visibles: visibles, marcos: anotado.marcos, util: d.resumen.cajaUtil,
                                              encima: encimaParaPapel())
            listo(datos.flatMap { (try? $0.write(to: salida)) != nil ? [salida] : nil } ?? [])
        case .png, .jpg:
            let jpg = formato == .jpg
            let salida = carpeta.appendingPathComponent(base + (jpg ? ".jpg" : ".png"))
            let img = ExportarPlanoIPad.imagen(d, visibles: visibles, region: d.resumen.cajaUtil, encima: encimaParaPapel())
            let datos = img.flatMap { ExportarPlanoIPad.codificar($0, jpg: jpg) }
            listo(datos.flatMap { (try? $0.write(to: salida)) != nil ? [salida] : nil } ?? [])
        case .dxf:
            let salida = carpeta.appendingPathComponent(base + ".dxf")
            Task {
                let ok = await Task.detached(priority: .userInitiated) { () -> Bool in
                    (try? plano.exportarDxf(salida: salida.path)) != nil
                }.value
                listo(ok ? [salida] : [])
            }
        case .original:
            let origen = URL(fileURLWithPath: ruta)
            let salida = carpeta.appendingPathComponent(origen.lastPathComponent)
            listo((try? FileManager.default.copyItem(at: origen, to: salida)) != nil ? [salida] : [])
        }
    }

    // MARK: Buscar en los textos

    /// Busca `consulta` en los textos del plano (la primera vez los lee:
    /// fuera del hilo principal) y lleva al primero.
    func buscar(_ consulta: String) {
        guard let p = datos?.plano else { return }
        let q = consulta.trimmingCharacters(in: .whitespacesAndNewlines)
        turnoBusqueda += 1
        let turno = turnoBusqueda
        guard !q.isEmpty else {
            hallados = []
            hallado = nil
            sinResultado = false
            buscandoAhora = false
            vista?.repintar()
            return
        }
        buscandoAhora = true
        Task {
            let r = await Task.detached(priority: .userInitiated) { try? p.buscarTextos(consulta: q, max: 50) }.value
            guard turno == self.turnoBusqueda else { return }
            self.buscandoAhora = false
            self.hallados = r ?? []
            self.sinResultado = (r ?? []).isEmpty
            if let h = self.hallados.first { self.mirar(h) } else { self.hallado = nil; self.vista?.repintar() }
        }
    }

    /// Lleva la vista a un texto hallado y lo marca.
    func mirar(_ h: TextoHallado) {
        hallado = h
        vista?.mirar(centroX: h.centroX, centroY: h.centroY, ancho: h.ancho)
    }

    /// El siguiente (o el anterior) de lo hallado.
    func otroHallado(_ paso: Int) {
        guard !hallados.isEmpty else { return }
        let i = hallado.flatMap { h in hallados.firstIndex(of: h) } ?? -paso
        mirar(hallados[((i + paso) % hallados.count + hallados.count) % hallados.count])
    }

    /// Que numero (desde 1) es lo hallado que se ensena; 0 si ninguno.
    var posicionHallado: Int {
        guard let h = hallado, let i = hallados.firstIndex(of: h) else { return 0 }
        return i + 1
    }

    func cerrarBusqueda() {
        buscando = false
        turnoBusqueda += 1
        hallados = []
        hallado = nil
        sinResultado = false
        buscandoAhora = false
        vista?.repintar()
    }

    // MARK: Letras («Aa»)

    /// Añade letras (SHX, TTF, OTF, TTC) y vuelve a leer el plano con ellas.
    func anadirLetras(_ urls: [URL]) {
        guard let nucleo, !urls.isEmpty else { return }
        // Lo elegido en Archivos puede estar fuera de la caja de la app: se
        // copia mientras se tiene permiso para leerlo.
        let carpeta = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-letras-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        var rutas: [String] = []
        for u in urls {
            let permiso = u.startAccessingSecurityScopedResource()
            defer { if permiso { u.stopAccessingSecurityScopedResource() } }
            let destino = carpeta.appendingPathComponent(u.lastPathComponent)
            if (try? FileManager.default.copyItem(at: u, to: destino)) != nil { rutas.append(destino.path) }
        }
        guard !rutas.isEmpty else { return }
        Task {
            let n = await Task.detached(priority: .userInitiated) { nucleo.anadirLetrasDePlanos(rutas: rutas) }.value
            try? FileManager.default.removeItem(at: carpeta)
            if n > 0 { self.releer() } else { UINotificationFeedbackGenerator().notificationOccurred(.error) }
        }
    }

    /// Quita las letras del usuario y vuelve a leer el plano.
    func quitarLetras() {
        guard let nucleo else { return }
        Task {
            let n = await Task.detached(priority: .userInitiated) { nucleo.quitarLetrasDePlanos() }.value
            if n > 0 { self.releer() }
        }
    }

    /// Cuantas letras puso el usuario.
    var letrasMias: UInt32 { nucleo?.letrasDePlanos() ?? 0 }

    // MARK: Marcadores con su vista

    /// **Marcador aqui** (`Ctrl M` de Windows): el centro y lo que se ve.
    func ponerMarcador() {
        guard let v = vista, datos != nil else { return }
        let c = v.camara
        marcadores.poner(x: c.centroX, y: c.centroY, ancho: c.px * v.anchoPx, alto: c.px * v.altoPx)
    }

    /// Volver a un marcador: cabe todo lo que se veia.
    func ir(a m: MarcadorDeVista) {
        guard let v = vista, v.anchoPx > 0, v.altoPx > 0 else { return }
        v.mirar(centroX: m.x, centroY: m.y, ancho: max(m.ancho, m.alto * v.anchoPx / v.altoPx))
    }

    // MARK: Anotar (las herramientas del lienzo)

    func alternarAnotar() {
        if anotador != nil { terminarDeAnotar() } else { empezarAnotar() }
    }

    private func empezarAnotar() {
        guard let d = datos else { return }
        if modo != .mover { modo = .mover }
        do {
            let a = try AnotadorPlanoIPad(plano: d.plano, nucleo: nucleo, titulo: titulo)
            let m = a.modelo
            m.alCerrar = { [weak self] in self?.terminarDeAnotar() }
            // Deshacer y rehacer de la barra: el plano se pinta otra vez.
            m.alDeshacer = { [weak self, weak a] in
                a?.deshacer()
                self?.vista?.repintar()
            }
            m.alRehacer = { [weak self, weak a] in
                a?.rehacer()
                self?.vista?.repintar()
            }
            anotador = a
            modeloAnotar = m
            anotando = true
            vista?.alCambiarAnotar()
        } catch {
            NSLog("PixPin planos: no se puede anotar: \(error)")
        }
    }

    /// El doble toque del Pencil: la goma, y otra vez lo de antes.
    func alternarGoma() {
        anotador?.modelo.alternarGoma()
        vista?.repintar()
    }

    /// Termina de anotar: guarda y vuelve a leer lo anotado (las cotas y los
    /// marcos pudieron cambiar con el lienzo).
    func terminarDeAnotar(_ despues: (() -> Void)? = nil) {
        guard let a = anotador else { despues?(); return }
        // Mientras se guarda, lo anotado sigue a la vista.
        _ = a.editor.terminarTexto()
        a.releer()
        tinta = a.leido
        anotador = nil
        modeloAnotar = nil
        anotando = false
        vista?.alCambiarAnotar()
        Task {
            await a.terminar()
            if let p = self.datos?.plano {
                self.anotado = p.anotado()
                self.rehacerEtiquetas()
            }
            self.leerTinta()
            self.vista?.repintar()
            despues?()
        }
    }
}

// MARK: - La vista de Metal

/// **El plano en Metal.** Una `MTKView` parada que pinta solo al cambiar
/// algo; los gestos mueven la camara (`CamaraPlano`).
final class VistaPlanoMetal: MTKView, UIGestureRecognizerDelegate, UIPencilInteractionDelegate, UIKeyInput {
    let visor: VisorPlano
    private let motor: MotorMetal
    private let pintor: PintorPlano
    private(set) var camara = CamaraPlano(centroX: 0, centroY: 0, px: 1) {
        didSet { seguirAnotador() }
    }
    private var encuadrado = false
    let encima = CapaEncimaPlano()

    // La regla y el marco.
    private(set) var cadena: [PuntoPlano] = []
    private(set) var mira: Enganche?
    private(set) var marcoEnCurso: (PuntoPlano, PuntoPlano)?

    // Los gestos.
    private let mover = UIPanGestureRecognizer()
    private let herramienta = UIPanGestureRecognizer()
    private let pellizco = UIPinchGestureRecognizer()
    private let toque = UITapGestureRecognizer()
    private let dobleToque = UITapGestureRecognizer()
    private var ultimoMover: CGPoint?
    private var dedosMover = 0
    /// El trazo del lapiz en curso.
    private weak var trazando: UITouch?

    init(visor: VisorPlano, motor: MotorMetal) {
        self.visor = visor
        self.motor = motor
        pintor = PintorPlano(motor: motor, color: .bgra8Unorm, muestras: motor.muestras)
        super.init(frame: CGRect(x: 0, y: 0, width: 800, height: 600), device: motor.dispositivo)
        colorPixelFormat = .bgra8Unorm
        depthStencilPixelFormat = .invalid
        sampleCount = motor.muestras
        isPaused = true
        enableSetNeedsDisplay = true
        autoResizeDrawable = true
        isMultipleTouchEnabled = true
        encima.vista = self
        encima.frame = bounds
        encima.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        addSubview(encima)

        mover.addTarget(self, action: #selector(alMover(_:)))
        herramienta.addTarget(self, action: #selector(alArrastrarHerramienta(_:)))
        herramienta.maximumNumberOfTouches = 1
        pellizco.addTarget(self, action: #selector(alPellizcar(_:)))
        toque.addTarget(self, action: #selector(alTocar(_:)))
        dobleToque.addTarget(self, action: #selector(alDobleToque(_:)))
        dobleToque.numberOfTapsRequired = 2
        for g in [mover, herramienta, pellizco, toque, dobleToque] as [UIGestureRecognizer] {
            g.delegate = self
            g.delaysTouchesEnded = false
            addGestureRecognizer(g)
        }
        // Con raton o trackpad, la rueda mueve el plano.
        mover.allowedScrollTypesMask = .continuous
        // El doble toque del Pencil (o apretarlo): goma y vuelta.
        let lapiz = UIPencilInteraction()
        lapiz.delegate = self
        addInteraction(lapiz)
        visor.vista = self
        try? pintor.preparar()
        actualizarGestos()
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    /// Pinta otra vez (y lo de encima).
    func repintar() {
        setNeedsDisplay()
        encima.setNeedsDisplay()
    }

    /// Al mover la camara con los dedos: el plano y lo de encima, ya.
    private func repintarYa() {
        draw()
        encima.setNeedsDisplay()
    }

    /// Pixeles por punto de la pantalla.
    var escala: Double {
        bounds.width > 0 && drawableSize.width > 0 ? Double(drawableSize.width) / Double(bounds.width) : Double(contentScaleFactor)
    }

    var anchoPx: Double { drawableSize.width > 0 ? Double(drawableSize.width) : Double(bounds.width * contentScaleFactor) }
    var altoPx: Double { drawableSize.height > 0 ? Double(drawableSize.height) : Double(bounds.height * contentScaleFactor) }

    /// El punto del dibujo bajo un punto de la vista (y hacia abajo).
    func aDibujo(_ p: CGPoint) -> PuntoPlano {
        camara.aDibujo(x: Double(p.x) * escala, y: Double(bounds.height - p.y) * escala, ancho: anchoPx, alto: altoPx)
    }

    /// El punto de la vista (y hacia abajo) de un punto del dibujo.
    func aVista(_ p: PuntoPlano) -> CGPoint {
        let q = camara.aPixel(p, ancho: anchoPx, alto: altoPx)
        let e = escala
        return CGPoint(x: q.x / e, y: bounds.height - q.y / e)
    }

    func alLlegarElPlano() {
        encuadrado = false
        encuadrar()
    }

    func encuadrar() {
        guard let d = visor.datos, bounds.width > 0, bounds.height > 0 else { return }
        camara = CamaraPlano.encuadrar(d.resumen.cajaUtil, ancho: anchoPx, alto: altoPx)
        encuadrado = true
        repintar()
    }

    /// Mira a `(centroX, centroY)` con `ancho` del dibujo a lo ancho.
    func mirar(centroX: Double, centroY: Double, ancho: Double) {
        guard anchoPx > 0, ancho > 0 else { return }
        camara = CamaraPlano(centroX: centroX, centroY: centroY, px: ancho / anchoPx)
        encuadrado = true
        repintar()
    }

    func cambioDeModo() {
        cadena = []
        mira = nil
        marcoEnCurso = nil
        visor.ponerMedida(ayuda(), puntos: 0)
        actualizarGestos()
        repintar()
    }

    func alCambiarAnotar() {
        cancelarTrazo()
        ajustarTeclado()
        seguirAnotador()
        actualizarGestos()
        repintar()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        if !encuadrado { encuadrar() }
        // Una zona pedida antes de que la vista tuviera tamano.
        visor.aplicarZonaPendiente()
        seguirAnotador()
        repintar()
    }

    // MARK: Pintar

    override func draw(_ rect: CGRect) {
        let oscuro = visor.oscuro
        clearColor = oscuro ? MTLClearColor(red: 0.13, green: 0.15, blue: 0.19, alpha: 1)
            : MTLClearColor(red: 0.99, green: 0.988, blue: 0.976, alpha: 1)
        guard let pasada = currentRenderPassDescriptor, let dibujable = currentDrawable,
              let cb = motor.cola.makeCommandBuffer(),
              let enc = cb.makeRenderCommandEncoder(descriptor: pasada) else { return }
        if let d = visor.datos {
            do {
                try pintor.pintar(d, visibles: visor.visibles, camara: camara, ancho: anchoPx, alto: altoPx,
                                  oscuro: oscuro, en: enc)
            } catch {
                NSLog("PixPin planos: \(error)")
            }
        }
        enc.endEncoding()
        cb.present(dibujable)
        cb.commit()
    }

    // MARK: Gestos

    /// Que gesto atiende que segun el modo: moviendo, uno o dos dedos mueven;
    /// midiendo o marcando, un dedo es la herramienta y dos mueven;
    /// anotando, el lapiz pinta (fuera de los gestos) y dos dedos mueven.
    private func actualizarGestos() {
        let dedos: [NSNumber] = [NSNumber(value: UITouch.TouchType.direct.rawValue),
                                 NSNumber(value: UITouch.TouchType.indirectPointer.rawValue),
                                 NSNumber(value: UITouch.TouchType.indirect.rawValue)]
        let todos = dedos + [NSNumber(value: UITouch.TouchType.pencil.rawValue)]
        let anotando = visor.anotando
        for g in [mover, herramienta, pellizco, toque, dobleToque] as [UIGestureRecognizer] {
            g.allowedTouchTypes = anotando ? dedos : todos
        }
        // Anotando sin Pencil, un dedo pinta: mover pide dos.
        let unDedoMueve = visor.modo == .mover && !(anotando && !visor.vistoLapiz)
        mover.minimumNumberOfTouches = unDedoMueve ? 1 : 2
        mover.maximumNumberOfTouches = 2
        herramienta.isEnabled = visor.modo != .mover && !anotando
        toque.isEnabled = visor.modo == .medir && !anotando
        dobleToque.isEnabled = !(anotando && !visor.vistoLapiz)
    }

    func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otro: UIGestureRecognizer) -> Bool {
        // Pellizcar y mover a la vez (el plano sigue a los dos dedos).
        (g === pellizco && otro === mover) || (g === mover && otro === pellizco)
    }

    /// Acercar `factor` dejando quieto el punto `p` de la vista.
    private func zoom(_ factor: Double, en p: CGPoint) {
        camara = camara.zoom(factor, x: Double(p.x) * escala, y: Double(bounds.height - p.y) * escala,
                             ancho: anchoPx, alto: altoPx)
        encuadrado = true
        repintarYa()
    }

    @objc private func alMover(_ g: UIPanGestureRecognizer) {
        let p = g.translation(in: self)
        switch g.state {
        case .began:
            ultimoMover = p
            dedosMover = g.numberOfTouches
        case .changed:
            // Al poner o quitar un dedo el centro salta: no se mueve.
            if g.numberOfTouches != dedosMover {
                dedosMover = g.numberOfTouches
                ultimoMover = p
                return
            }
            let a = ultimoMover ?? p
            // La y de la pantalla baja; la de la camara sube.
            camara = camara.mover(dx: Double(p.x - a.x) * escala, dy: -Double(p.y - a.y) * escala)
            ultimoMover = p
            encuadrado = true
            repintarYa()
        default:
            ultimoMover = nil
        }
    }

    @objc private func alPellizcar(_ g: UIPinchGestureRecognizer) {
        guard g.state == .began || g.state == .changed else { return }
        zoom(Double(g.scale), en: g.location(in: self))
        g.scale = 1
    }

    @objc private func alDobleToque(_ g: UITapGestureRecognizer) {
        if visor.modo == .medir {
            // El primer toque del doble ya puso su punto: se guarda la cota.
            terminarCadena()
        } else {
            encuadrar()
        }
    }

    @objc private func alTocar(_ g: UITapGestureRecognizer) {
        guard visor.modo == .medir, visor.datos != nil else { return }
        let m = enganche(g.location(in: self))
        let q = PuntoPlano(x: m.x, y: m.y)
        if cadena.last != q { cadena.append(q) }
        mira = m
        actualizarMedida()
        encima.setNeedsDisplay()
    }

    /// Un dedo midiendo (la mira sigue al dedo y el punto va donde se
    /// suelta) o marcando (el marco de la esquina de inicio al dedo).
    @objc private func alArrastrarHerramienta(_ g: UIPanGestureRecognizer) {
        guard visor.datos != nil else { return }
        let p = g.location(in: self)
        switch visor.modo {
        case .mover:
            return
        case .medir:
            switch g.state {
            case .began, .changed:
                mira = enganche(p)
                actualizarMedida()
            case .ended:
                let m = enganche(p)
                mira = m
                let q = PuntoPlano(x: m.x, y: m.y)
                if cadena.last != q { cadena.append(q) }
                actualizarMedida()
            default:
                break
            }
        case .marco:
            switch g.state {
            case .began:
                // Desde donde empezo el dedo, no desde donde se reconocio.
                let t = g.translation(in: self)
                let m = enganche(CGPoint(x: p.x - t.x, y: p.y - t.y))
                let q = PuntoPlano(x: m.x, y: m.y)
                marcoEnCurso = (q, q)
            case .changed:
                guard let a = marcoEnCurso?.0 else { return }
                let m = enganche(p)
                mira = m
                marcoEnCurso = (a, PuntoPlano(x: m.x, y: m.y))
            case .ended:
                if case let (a, b)? = marcoEnCurso {
                    let c = CajaPlano(x0: min(a.x, b.x), y0: min(a.y, b.y), x1: max(a.x, b.x), y1: max(a.y, b.y))
                    // Un toque sin arrastrar no es un marco.
                    if (c.x1 - c.x0) / camara.px > 8 && (c.y1 - c.y0) / camara.px > 8 { visor.anadirMarco(c) }
                }
                marcoEnCurso = nil
                mira = nil
            default:
                marcoEnCurso = nil
                mira = nil
            }
        }
        encima.setNeedsDisplay()
    }

    /// El enganche de la regla bajo el dedo (a 14 puntos de la pantalla: el
    /// dedo es menos preciso que el raton).
    private func enganche(_ p: CGPoint) -> Enganche {
        let d = aDibujo(p)
        guard let plano = visor.datos?.plano else { return Enganche(x: d.x, y: d.y, tipo: .libre) }
        let radio = 14 * escala * camara.px
        return plano.enganchar(x: d.x, y: d.y, radio: radio, desde: cadena.last)
    }

    private func ayuda() -> String {
        switch visor.modo {
        case .medir: "Toca los puntos · doble toque: guardar la cota"
        case .marco: "Arrastra para poner un marco"
        case .mover: ""
        }
    }

    private func actualizarMedida() {
        var puntos = cadena
        if let m = mira {
            let q = PuntoPlano(x: m.x, y: m.y)
            if puntos.last != q { puntos.append(q) }
        }
        guard puntos.count >= 2 else {
            visor.ponerMedida(ayuda(), puntos: cadena.count)
            return
        }
        var total = 0.0
        for (a, b) in zip(puntos, puntos.dropFirst()) { total += hypot(b.x - a.x, b.y - a.y) }
        let ultimo = hypot(puntos[puntos.count - 1].x - puntos[puntos.count - 2].x,
                           puntos[puntos.count - 1].y - puntos[puntos.count - 2].y)
        visor.ponerMedida(puntos.count > 2 ? "\(visor.texto(ultimo)) · total \(visor.texto(total))" : visor.texto(ultimo),
                          puntos: cadena.count)
    }

    /// Guarda la regla en curso como cota.
    func terminarCadena() {
        var puntos = cadena
        // El segundo toque de un doble toque cae casi en el mismo sitio.
        while puntos.count >= 2 {
            let a = aVista(puntos[puntos.count - 1]), b = aVista(puntos[puntos.count - 2])
            if hypot(a.x - b.x, a.y - b.y) < 12 { puntos.removeLast() } else { break }
        }
        cadena = []
        mira = nil
        if puntos.count >= 2 { visor.anadirCota(puntos) }
        actualizarMedida()
        encima.setNeedsDisplay()
    }

    func quitarUltimoPunto() {
        guard !cadena.isEmpty else { return }
        cadena.removeLast()
        mira = nil
        actualizarMedida()
        encima.setNeedsDisplay()
    }

    func soltarLoDeAMedias() {
        cadena = []
        mira = nil
        marcoEnCurso = nil
        actualizarMedida()
        encima.setNeedsDisplay()
    }

    // MARK: El lapiz (anotando)

    /// Unidades del dibujo por punto de la pantalla.
    var unidadesPorPunto: Double { camara.px * escala }

    private func seguirAnotador() {
        guard let a = visor.anotador, a.encaje.u > 0, unidadesPorPunto > 0 else { return }
        a.escalaDeLoNuevo(zoom: a.encaje.u / unidadesPorPunto)
    }

    /// Un punto de la vista en la capa del plano.
    private func aCapa(_ p: CGPoint, encaje e: EncajePlano) -> (x: Double, y: Double) {
        let d = aDibujo(p)
        return ((d.x - e.ceroX) / e.u, (e.ceroY - d.y) / e.u)
    }

    private func puedePintar(_ t: UITouch, _ event: UIEvent?) -> Bool {
        guard visor.anotando else { return false }
        if t.type == .pencil { return true }
        return !visor.vistoLapiz && t.type == .direct && (event?.allTouches?.count ?? 1) == 1
    }

    private func modificadores(_ evento: UIEvent?) -> UInt32 {
        guard let f = evento?.modifierFlags else { return 0 }
        var m: UInt32 = 0
        if f.contains(.shift) { m |= 1 }
        if f.contains(.alternate) { m |= 2 }
        return m
    }

    private func mandar(_ fase: FasePuntero, _ t: UITouch, _ event: UIEvent?) {
        guard let a = visor.anotador, a.encaje.u > 0, unidadesPorPunto > 0 else { return }
        let zoom = a.encaje.u / unidadesPorPunto
        let toques = (fase == .mover ? event?.coalescedTouches(for: t) : nil) ?? [t]
        let mods = modificadores(event)
        for c in toques {
            let punto = c.type == .pencil ? c.preciseLocation(in: self) : c.location(in: self)
            let q = aCapa(punto, encaje: a.encaje)
            let presion: Float? = c.type == .pencil && c.maximumPossibleForce > 0
                ? Float(c.force / c.maximumPossibleForce) : nil
            a.puntero(fase, x: q.x, y: q.y, presion: presion, modificadores: mods, zoom: zoom)
        }
        encima.setNeedsDisplay()
        if fase == .pulsar || fase == .soltar {
            a.trasCambio()
            ajustarTeclado()
        }
        if fase == .soltar { atenderPeticion(a) }
    }

    /// Tras soltar con una herramienta de medir del lienzo: el cajetin que
    /// pida (calibrar, dictar la cota). La foto de una zona no se hace aqui
    /// (sobre un plano no tiene sentido): se suelta la peticion.
    private func atenderPeticion(_ a: AnotadorPlanoIPad) {
        guard let peticion = a.editor.tomarPeticion() else { return }
        let editor = a.editor
        switch peticion {
        case .zona:
            break
        case let .calibrar(largoPx):
            CajetinesLienzo.calibrar(unidades: editor.unidadesDeMedida(), desde: self) { [weak self, weak a] valor, unidad in
                if editor.calibrar(largoPx: largoPx, valor: valor, unidad: unidad) {
                    a?.trasCambio()
                    self?.encima.setNeedsDisplay()
                }
            }
        case let .dictarCota(id, largo, grados, unidad):
            CajetinesLienzo.dictarCota(largo: largo, grados: grados, unidad: unidad, desde: self) { [weak self, weak a] l, g in
                if editor.dictarCota(id: id, largo: l, grados: g) {
                    a?.trasCambio()
                    self?.encima.setNeedsDisplay()
                }
            }
        }
    }

    func pencilInteraction(_ interaction: UIPencilInteraction, didReceiveTap tap: UIPencilInteraction.Tap) {
        guard visor.anotando else { return }
        visor.alternarGoma()
    }

    private func cancelarTrazo() {
        guard trazando != nil else { return }
        trazando = nil
        visor.anotador?.puntero(.cancelar, x: 0, y: 0, presion: nil, zoom: 1)
        encima.setNeedsDisplay()
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesBegan(touches, with: event)
        guard visor.anotando else { return }
        for t in touches {
            if t.type == .pencil, !visor.vistoLapiz {
                visor.vistoLapiz = true
                actualizarGestos()
            }
            if trazando == nil, puedePintar(t, event) {
                trazando = t
                if visor.modeloAnotar?.desplegado != nil { visor.modeloAnotar?.desplegado = nil }
                mandar(.pulsar, t, event)
            } else if trazando != nil, t.type != .pencil, trazando?.type != .pencil {
                // Un segundo dedo: era para mover, no para pintar.
                cancelarTrazo()
            }
        }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesMoved(touches, with: event)
        guard let t = trazando, touches.contains(t) else { return }
        mandar(.mover, t, event)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesEnded(touches, with: event)
        guard let t = trazando, touches.contains(t) else { return }
        mandar(.soltar, t, event)
        trazando = nil
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesCancelled(touches, with: event)
        guard let t = trazando, touches.contains(t) else { return }
        cancelarTrazo()
    }

    // MARK: El teclado (el texto del lienzo)

    private var escribiendo: Bool { visor.anotador?.editor.escribiendo() ?? false }

    override var canBecomeFirstResponder: Bool { escribiendo }

    /// El teclado sale mientras se escribe un texto y se va al terminar.
    func ajustarTeclado() {
        if escribiendo, !isFirstResponder {
            becomeFirstResponder()
        } else if !escribiendo, isFirstResponder {
            resignFirstResponder()
        }
    }

    var hasText: Bool { true }

    func insertText(_ text: String) {
        guard let a = visor.anotador, a.editor.escribiendo() else { return }
        let hecho = text == "\n" ? a.editor.tecla(nombre: "entrar") : a.editor.teclear(texto: text)
        if hecho {
            a.trasCambio()
            encima.setNeedsDisplay()
        }
        ajustarTeclado()
    }

    func deleteBackward() {
        guard let a = visor.anotador, a.editor.tecla(nombre: "retroceso") else { return }
        a.trasCambio()
        encima.setNeedsDisplay()
    }

    var autocorrectionType: UITextAutocorrectionType = .no
    var spellCheckingType: UITextSpellCheckingType = .no
    var smartQuotesType: UITextSmartQuotesType = .no
    var smartDashesType: UITextSmartDashesType = .no

    /// Las flechas, Inicio, Fin, Suprimir y Esc del teclado fisico mientras
    /// se escribe.
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        var usadas = false
        if let a = visor.anotador, a.editor.escribiendo() {
            for p in presses {
                guard let k = p.key else { continue }
                let nombre: String? = switch k.keyCode {
                case .keyboardLeftArrow: "izquierda"
                case .keyboardRightArrow: "derecha"
                case .keyboardHome: "inicio"
                case .keyboardEnd: "fin"
                case .keyboardDeleteForward: "suprimir"
                case .keyboardEscape: "escape"
                default: nil
                }
                if let nombre {
                    usadas = true
                    if a.editor.tecla(nombre: nombre) { a.trasCambio() }
                }
            }
            encima.setNeedsDisplay()
            ajustarTeclado()
        }
        if !usadas { super.pressesBegan(presses, with: event) }
    }

    /// Lo que se ve ahora, en el dibujo (para imprimir sin marcos).
    func cajaVisible() -> CajaPlano {
        let mx = anchoPx / 2 * camara.px, my = altoPx / 2 * camara.px
        return CajaPlano(x0: camara.centroX - mx, y0: camara.centroY - my, x1: camara.centroX + mx, y1: camara.centroY + my)
    }
}

// MARK: - Lo de encima

/// **Lo que va encima del plano**, con Core Graphics: lo anotado con el
/// lapiz, las cotas (naranja, como Windows y Android), los marcos para
/// imprimir (azul), la regla en curso y la mira del enganche. No recibe los
/// toques: los atiende la vista de Metal.
final class CapaEncimaPlano: UIView {
    weak var vista: VistaPlanoMetal?

    static let naranja = UIColor(red: 1, green: 0x8A / 255.0, blue: 0, alpha: 1)
    static let azul = UIColor(red: 0x3D / 255.0, green: 0x8B / 255.0, blue: 1, alpha: 1)

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = false
        backgroundColor = .clear
        isUserInteractionEnabled = false
        contentMode = .redraw
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) no se usa") }

    override func draw(_ rect: CGRect) {
        guard let v = vista, let ctx = UIGraphicsGetCurrentContext() else { return }
        let visor = v.visor
        if let a = visor.anotador {
            // Anotando, la capa entera (tambien las cotas y los marcos) la
            // pinta el lienzo.
            let s = v.unidadesPorPunto
            guard a.encaje.u > 0, s > 0 else { return }
            let t = PintorTintaPlano.transformacion(camara: v.camara, s: s, encaje: a.encaje, tamano: bounds.size)
            let origen = CGPoint.zero.applying(t.inverted())
            a.releer(origen: origen, zoom: a.encaje.u / s, tamano: bounds.size)
            if let l = a.leido {
                pintarCapa(l, a.encaje, v, ctx, imagen: a.modelo.imagenes(escala: CGFloat(a.encaje.u / s) * contentScaleFactor))
            }
            return
        }
        if let t = visor.tinta, let e = visor.encaje { pintarCapa(t, e, v, ctx) }
        // El texto hallado: un recuadro amarillo alrededor de su sitio.
        if let h = visor.hallado {
            let p = v.aVista(PuntoPlano(x: h.x, y: h.y))
            let alto = max(CGFloat(h.alto / v.unidadesPorPunto), 6)
            let ancho = max(alto * CGFloat(max(h.texto.count, 1)) * 0.6, 12)
            let r = CGRect(x: p.x - 4, y: p.y - alto - 4, width: ancho + 8, height: alto + 8)
            ctx.setFillColor(UIColor.systemYellow.withAlphaComponent(0.25).cgColor)
            ctx.fill(r)
            ctx.setStrokeColor(UIColor.systemOrange.cgColor)
            ctx.setLineWidth(2)
            ctx.stroke(r)
        }
        // Los marcos para imprimir, con su numero abajo a la izquierda.
        ctx.setLineWidth(1.2)
        ctx.setStrokeColor(CapaEncimaPlano.azul.cgColor)
        ctx.setLineDash(phase: 0, lengths: [6, 4])
        for m in visor.anotado.marcos { ctx.stroke(self.rect(v, m)) }
        if case let (a, b)? = v.marcoEnCurso {
            ctx.stroke(self.rect(v, CajaPlano(x0: min(a.x, b.x), y0: min(a.y, b.y), x1: max(a.x, b.x), y1: max(a.y, b.y))))
        }
        ctx.setLineDash(phase: 0, lengths: [])
        for (i, m) in visor.anotado.marcos.enumerated() {
            let r = self.rect(v, m)
            let t = "\(i + 1)" as NSString
            let atr: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: 10, weight: .semibold),
                                                      .foregroundColor: UIColor.white]
            let tam = t.size(withAttributes: atr)
            let caja = CGRect(x: r.minX, y: r.maxY - 16, width: max(16, tam.width + 6), height: 16)
            ctx.setFillColor(CapaEncimaPlano.azul.cgColor)
            ctx.fill(caja)
            t.draw(at: CGPoint(x: caja.midX - tam.width / 2, y: caja.midY - tam.height / 2), withAttributes: atr)
        }
        // Las cotas guardadas y la regla en curso.
        for (k, c) in visor.anotado.cotas.enumerated() {
            cadena(v, c.puntos, textos: k < visor.etiquetas.count ? visor.etiquetas[k] : [], ctx: ctx)
        }
        if !v.cadena.isEmpty {
            var puntos = v.cadena
            if let m = v.mira, visor.modo == .medir {
                let q = PuntoPlano(x: m.x, y: m.y)
                if puntos.last != q { puntos.append(q) }
            }
            let textos = zip(puntos, puntos.dropFirst()).map { a, b in visor.texto(hypot(b.x - a.x, b.y - a.y)) }
            cadena(v, puntos, textos: textos, ctx: ctx)
        }
        if let m = v.mira, visor.modo != .mover { pintarMira(v.aVista(PuntoPlano(x: m.x, y: m.y)), m.tipo, ctx) }
    }

    /// La capa del lienzo en su sitio, con la camara del plano y el pintor
    /// del lienzo (formas, flechas, textos, imagenes...).
    private func pintarCapa(_ l: LienzoLeido, _ e: EncajePlano, _ v: VistaPlanoMetal, _ ctx: CGContext,
                            imagen: @escaping (UInt64) -> CGImage? = { _ in nil }) {
        let s = v.unidadesPorPunto
        guard e.u > 0, s > 0 else { return }
        let t = PintorTintaPlano.transformacion(camara: v.camara, s: s, encaje: e, tamano: bounds.size)
        ctx.saveGState()
        ctx.concatenate(t)
        PintorLienzo(lienzo: l, imagen: imagen).pintar(en: ctx, limites: bounds.applying(t.inverted()))
        ctx.restoreGState()
    }

    private func rect(_ v: VistaPlanoMetal, _ c: CajaPlano) -> CGRect {
        let a = v.aVista(PuntoPlano(x: c.x0, y: c.y0)), b = v.aVista(PuntoPlano(x: c.x1, y: c.y1))
        return CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(b.x - a.x), height: abs(b.y - a.y))
    }

    /// Una cadena de cotas: la raya, sus puntos y cada medida en su tramo.
    private func cadena(_ v: VistaPlanoMetal, _ puntos: [PuntoPlano], textos: [String], ctx: CGContext) {
        let q = puntos.map { v.aVista($0) }
        guard q.count >= 2 else {
            if let p = q.first { marca(p, ctx) }
            return
        }
        ctx.setStrokeColor(CapaEncimaPlano.naranja.cgColor)
        ctx.setLineWidth(1.5)
        ctx.addLines(between: q)
        ctx.strokePath()
        for p in q { marca(p, ctx) }
        let atributos: [NSAttributedString.Key: Any] = [
            .font: UIFont.monospacedDigitSystemFont(ofSize: 12, weight: .semibold),
            .foregroundColor: UIColor.white,
            .backgroundColor: CapaEncimaPlano.naranja.withAlphaComponent(0.9),
        ]
        for (i, t) in textos.enumerated() where i + 1 < q.count {
            let m = CGPoint(x: (q[i].x + q[i + 1].x) / 2, y: (q[i].y + q[i + 1].y) / 2)
            let s = NSAttributedString(string: " \(t) ", attributes: atributos)
            let tam = s.size()
            s.draw(at: CGPoint(x: m.x - tam.width / 2, y: m.y - tam.height - 3))
        }
    }

    private func marca(_ p: CGPoint, _ ctx: CGContext) {
        ctx.setFillColor(CapaEncimaPlano.naranja.cgColor)
        ctx.fillEllipse(in: CGRect(x: p.x - 3.5, y: p.y - 3.5, width: 7, height: 7))
    }

    /// La mira: un cuadrado en un punto, una escuadra en la perpendicular,
    /// una cruz en la raya y un circulo libre (las marcas de AutoCAD).
    private func pintarMira(_ p: CGPoint, _ tipo: TipoEnganche, _ ctx: CGContext) {
        let r: CGFloat = 8
        ctx.setLineWidth(1.5)
        ctx.setStrokeColor(UIColor.systemGreen.cgColor)
        switch tipo {
        case .punto:
            ctx.stroke(CGRect(x: p.x - r, y: p.y - r, width: 2 * r, height: 2 * r))
        case .perpendicular:
            ctx.addLines(between: [CGPoint(x: p.x - r, y: p.y - r), CGPoint(x: p.x - r, y: p.y + r), CGPoint(x: p.x + r, y: p.y + r)])
            ctx.addLines(between: [CGPoint(x: p.x - r, y: p.y), CGPoint(x: p.x, y: p.y), CGPoint(x: p.x, y: p.y + r)])
            ctx.strokePath()
        case .enLaRaya:
            ctx.addLines(between: [CGPoint(x: p.x - r, y: p.y - r), CGPoint(x: p.x + r, y: p.y + r)])
            ctx.addLines(between: [CGPoint(x: p.x - r, y: p.y + r), CGPoint(x: p.x + r, y: p.y - r)])
            ctx.strokePath()
        case .libre:
            ctx.setStrokeColor(UIColor.secondaryLabel.cgColor)
            ctx.strokeEllipse(in: CGRect(x: p.x - r / 2, y: p.y - r / 2, width: r, height: r))
        }
    }
}

// MARK: - SwiftUI

/// La vista de Metal dentro de SwiftUI.
struct LienzoMetalPlano: UIViewRepresentable {
    let visor: VisorPlano
    let motor: MotorMetal
    /// Solo para que SwiftUI repinte al cambiar el modo noche.
    let oscuro: Bool

    func makeUIView(context _: Context) -> VistaPlanoMetal { VistaPlanoMetal(visor: visor, motor: motor) }
    func updateUIView(_ v: VistaPlanoMetal, context _: Context) { v.repintar() }
}

/// **El visor de un plano** (DWG, DXF...) a pantalla completa: la barra
/// comun arriba (mover, medir, marcos, anotar, ver entero, buscar, capas y
/// el menu: imprimir, exportar, marcador, letras, 3D), el plano en Metal y
/// las capas en un panel flotante.
public struct VisorPlanoIPad: View {
    let url: URL
    let titulo: String
    let nucleo: Nucleo
    @StateObject private var visor: VisorPlano
    @ObservedObject private var noche = ModoNoche.compartido
    @State private var consulta = ""
    @FocusState private var enBuscador: Bool

    public init(url: URL, titulo: String, nucleo: Nucleo) {
        self.url = url
        self.titulo = titulo
        self.nucleo = nucleo
        _visor = StateObject(wrappedValue: VisorPlano(ruta: url.path, titulo: titulo))
    }

    public var body: some View {
        ZStack(alignment: .top) {
            Color(white: noche.activo ? 0.15 : 0.98).ignoresSafeArea()
            contenido.ignoresSafeArea()
            if let m = visor.modeloAnotar {
                BarrasLienzoEncima(modelo: m, arriba: 58) { visor.vista?.repintar() }
            }
            VStack(spacing: 8) {
                BarraVisorIPad(tipo: "Plano", icono: "square.grid.3x3", titulo: titulo,
                               alFijar: { GestorPinesIPad.compartido.pinearLoQueSeVe() }) { botones }
                if visor.anotando {
                    // Las barras del lienzo van debajo (`BarrasLienzoEncima`).
                    Color.clear.frame(height: 56)
                }
                if visor.buscando { buscador }
                if visor.avisoCivil { avisoCivil }
                if !visor.faltan.isEmpty && !visor.anotando { avisoLetras }
                if !visor.medida.isEmpty { etiquetaDeMedida }
                Spacer(minLength: 0)
                if visor.datos != nil { abajo.frame(maxWidth: .infinity, alignment: .leading).padding(14) }
            }
            if visor.verCapas && visor.datos != nil {
                PanelFlotantePlano(alto: min(420, 60 + CGFloat(visor.capas.count) * 44)) { panelCapas }
            }
            if visor.preparando {
                ProgressView("Preparando…")
                    .padding(16)
                    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onAppear {
            if let p = IrEnElLector.tomar(url) { visor.pedir(p) }
            visor.cargar(nucleo: nucleo)
        }
        .onReceive(NotificationCenter.default.publisher(for: IrEnElLector.cambio)) { n in
            // Un pedido nuevo para este plano, ya abierto.
            guard IrEnElLector.esPara(n, url), let p = IrEnElLector.tomar(url) else { return }
            visor.pedir(p)
        }
        .onDisappear { if visor.anotando { visor.terminarDeAnotar() } }
        .onChange(of: noche.activo) { _, _ in visor.vista?.repintar() }
        .sheet(isPresented: $visor.verLetras) { LetrasDelPlanoIPad(visor: visor) }
    }

    @ViewBuilder private var botones: some View {
        Group {
            botonesDeModo
            botonesDeVer
            menuMas
        }
        .disabled(visor.datos == nil)
    }

    /// Mover, medir, marcos y anotar.
    @ViewBuilder private var botonesDeModo: some View {
            Button { visor.modo = .mover } label: { Image(systemName: "hand.raised") }
                .foregroundStyle(visor.modo == .mover && !visor.anotando ? Paleta.v2.acento : Color.primary)
                .help("Mover")
            Button { visor.modo = visor.modo == .medir ? .mover : .medir } label: { Image(systemName: "ruler") }
                .foregroundStyle(visor.modo == .medir ? Paleta.v2.acento : Color.primary)
                .keyboardShortcut(tecla("m"))
                .help("Medir (M)")
            Button { visor.modo = visor.modo == .marco ? .mover : .marco } label: { Image(systemName: "rectangle.dashed") }
                .foregroundStyle(visor.modo == .marco ? Paleta.v2.acento : Color.primary)
                .keyboardShortcut(tecla("i"))
                .help("Marcos para imprimir (I)")
            Button { visor.alternarAnotar() } label: { Image(systemName: "pencil.tip.crop.circle") }
                .foregroundStyle(visor.anotando ? Paleta.v2.acento : Color.primary)
                .keyboardShortcut(tecla("a"))
                .help(visor.anotando ? "Terminar de anotar" : "Anotar (A)")
    }

    /// Ver entero, buscar, letras y capas.
    @ViewBuilder private var botonesDeVer: some View {
            Button { visor.encuadrar() } label: { Image(systemName: "viewfinder") }
                .keyboardShortcut(tecla("f"))
                .help("Ver el plano entero (F)")
            Button {
                if visor.buscando { visor.cerrarBusqueda() } else { visor.buscando = true; enBuscador = true }
            } label: { Image(systemName: "magnifyingglass") }
                .foregroundStyle(visor.buscando ? Paleta.v2.acento : Color.primary)
                .keyboardShortcut("f", modifiers: .command)
                .help("Buscar en los textos (⌘F)")
            Button { visor.verLetras = true } label: {
                Text("Aa").font(.system(size: 15, weight: .semibold))
                    .overlay(alignment: .topTrailing) {
                        if !visor.faltan.isEmpty { Circle().fill(Color.orange).frame(width: 7, height: 7).offset(x: 4, y: -2) }
                    }
            }
            .keyboardShortcut(tecla("l"))
            .help(visor.faltan.isEmpty ? "Letras del plano (L)" : "Faltan \(visor.faltan.count) letras (L)")
            Button { visor.verCapas.toggle() } label: { Image(systemName: "square.3.layers.3d") }
                .foregroundStyle(visor.verCapas ? Paleta.v2.acento : Color.primary)
                .help("Capas")
    }

    /// El menu: imprimir, exportar, marcador, 3D y borrar.
    private var menuMas: some View {
            Menu {
                Section {
                    Button { visor.imprimir(conMembrete: true) } label: {
                        Label("Imprimir con marco y membrete…", systemImage: "printer")
                    }
                    .keyboardShortcut("p", modifiers: .command)
                    Button { visor.imprimir(conMembrete: false) } label: {
                        Label("Imprimir sin marco…", systemImage: "printer.dotmatrix")
                    }
                }
                Menu {
                    ForEach(visor.formatosDeExportar) { f in
                        Button { visor.exportar(f) } label: { Label(f.nombre, systemImage: f.simbolo) }
                    }
                } label: { Label("Exportar (HTML, PDF, PNG, DXF…)", systemImage: "square.and.arrow.up") }
                Button { visor.exportar(.pdf) } label: { Label("Exportar en PDF", systemImage: "doc.richtext") }
                    .keyboardShortcut("e", modifiers: .command)
                Divider()
                Button { visor.ponerMarcador() } label: { Label("Marcador aquí", systemImage: "bookmark") }
                    .keyboardShortcut("m", modifiers: .command)
                Button { Navegacion.compartida.abrir(.modelo3d(url: url, titulo: titulo)) } label: {
                    Label("Ver en 3D", systemImage: "cube")
                }
                .keyboardShortcut(tecla("3"))
                Divider()
                Button(role: .destructive) { visor.borrarCotas() } label: { Label("Borrar las cotas", systemImage: "ruler") }
                    .disabled(visor.anotado.cotas.isEmpty)
                Button(role: .destructive) { visor.borrarMarcos() } label: {
                    Label("Borrar los marcos", systemImage: "rectangle.dashed")
                }
                .disabled(visor.anotado.marcos.isEmpty)
            } label: { Image(systemName: "ellipsis.circle") }
    }

    /// Una tecla sola (sin ⌘) como atajo, salvo anotando: entonces las
    /// letras son del texto que se escribe.
    private func tecla(_ k: KeyEquivalent) -> KeyboardShortcut? {
        visor.anotando || visor.buscando ? nil : KeyboardShortcut(k, modifiers: [])
    }

    /// El buscador de textos: el campo, cuantos, anterior y siguiente.
    private var buscador: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Buscar en el plano", text: $consulta)
                .textFieldStyle(.plain)
                .focused($enBuscador)
                .submitLabel(.search)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .onSubmit { visor.buscar(consulta) }
                .frame(minWidth: 180, maxWidth: 280)
            resultadoDeBusqueda
            Button { visor.cerrarBusqueda() } label: { Image(systemName: "xmark.circle.fill") }
                .foregroundStyle(.secondary)
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(.regularMaterial, in: Capsule())
        .overlay(Capsule().strokeBorder(.separator.opacity(0.5)))
    }

    /// Lo hallado: cuantos, anterior, siguiente y el texto.
    @ViewBuilder private var resultadoDeBusqueda: some View {
        if visor.buscandoAhora {
            ProgressView().controlSize(.small)
        } else if visor.sinResultado {
            Text("No está").font(.callout).foregroundStyle(.secondary)
        } else if !visor.hallados.isEmpty {
            navegarHallados
        }
    }

    private var navegarHallados: some View {
        let texto: String = "\(visor.posicionHallado) de \(visor.hallados.count)"
        return HStack(spacing: 10) {
            Text(texto).font(.callout.monospacedDigit()).foregroundStyle(.secondary)
            Button { visor.otroHallado(-1) } label: { Image(systemName: "chevron.up") }
                .keyboardShortcut("g", modifiers: [.command, .shift])
            Button { visor.otroHallado(1) } label: { Image(systemName: "chevron.down") }
                .keyboardShortcut("g", modifiers: .command)
            Text(visor.hallado?.texto ?? "").font(.callout).lineLimit(1).frame(maxWidth: 220, alignment: .leading)
        }
    }

    /// Los objetos de Civil 3D sin su dibujo (`PROXYGRAPHICS = 0`).
    private var avisoCivil: some View {
        HStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
            Text(visor.textoAvisoCivil).font(.callout).lineLimit(3)
            Button { visor.avisoCivil = false } label: { Image(systemName: "xmark") }
                .buttonStyle(.borderless)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .frame(maxWidth: 640)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
    }

    /// Faltan letras: «Faltan n letras del plano: pulsa Aa arriba para añadirlas».
    private var avisoLetras: some View {
        Button { visor.verLetras = true } label: {
            HStack(spacing: 8) {
                Image(systemName: "textformat").foregroundStyle(.orange)
                Text("Faltan \(visor.faltan.count) letras del plano: pulsa Aa arriba para añadirlas")
                    .font(.callout).foregroundStyle(.primary).lineLimit(1)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 6)
            .background(.regularMaterial, in: Capsule())
        }
        .buttonStyle(.plain)
    }

    /// La medida de la regla (o la ayuda del modo), con deshacer y guardar.
    private var etiquetaDeMedida: some View {
        HStack(spacing: 12) {
            Text(visor.medida).font(.callout.monospacedDigit()).lineLimit(1)
            if visor.modo == .medir && visor.puntosEnCurso > 0 {
                Button { visor.vista?.quitarUltimoPunto() } label: { Image(systemName: "arrow.uturn.backward") }
                    .help("Quitar el último punto")
                Button { visor.vista?.soltarLoDeAMedias() } label: { Image(systemName: "xmark.circle") }
                    .help("Soltar la regla")
                if visor.puntosEnCurso >= 2 {
                    Button { visor.vista?.terminarCadena() } label: { Image(systemName: "checkmark.circle.fill") }
                        .help("Guardar la cota")
                }
            }
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 14)
        .padding(.vertical, 6)
        .background(.regularMaterial, in: Capsule())
    }

    /// Abajo a la izquierda: los marcos para imprimir (azules, con imprimir
    /// al lado) y los marcadores con su vista (redondos).
    private var abajo: some View {
        HStack(spacing: 6) {
            if !visor.anotado.marcos.isEmpty || visor.modo == .marco {
                fichasDeMarcos
                if !visor.anotado.marcos.isEmpty {
                    Button { visor.imprimir(conMembrete: true) } label: {
                        Image(systemName: "printer").frame(width: 34, height: 34)
                            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 7))
                    }
                    .buttonStyle(.plain)
                    .help("Imprimir los marcos (⌘P)")
                }
            }
            MarcadoresDelPlano(visor: visor, marcadores: visor.marcadores)
        }
    }

    private var fichasDeMarcos: some View {
        HStack(spacing: 6) {
            ForEach(Array(visor.anotado.marcos.indices), id: \.self) { i in
                Button { visor.irAMarco(i) } label: {
                    Text("\(i + 1)").font(.system(size: 13, weight: .semibold)).foregroundStyle(.white)
                        .frame(width: 34, height: 34)
                        .background(RoundedRectangle(cornerRadius: 7).fill(Color(uiColor: CapaEncimaPlano.azul)))
                }
                .buttonStyle(.plain)
                .contextMenu {
                    Button("Quitar el último marco", role: .destructive) { visor.quitarUltimoMarco() }
                }
            }
        }
    }

    @ViewBuilder private var contenido: some View {
        if let error = visor.error {
            ContentUnavailableView("No se puede ver el plano", systemImage: "exclamationmark.triangle",
                                   description: Text(error))
        } else if let motor = MotorMetal.compartido {
            ZStack {
                LienzoMetalPlano(visor: visor, motor: motor, oscuro: noche.activo)
                if visor.datos == nil { ProgressView("Leyendo el plano…") }
            }
        } else {
            ContentUnavailableView("Sin Metal", systemImage: "cpu",
                                   description: Text("Este iPad no tiene una tarjeta gráfica con Metal."))
        }
    }

    private var panelCapas: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("Capas").font(.headline)
                Spacer()
                Button("Todas") { visor.todasLasCapas(true) }
                Button("Ninguna") { visor.todasLasCapas(false) }
            }
            .buttonStyle(.borderless)
            .padding(12)
            List(visor.capas, id: \.nombre) { c in
                Toggle(isOn: Binding(get: { c.encendida }, set: { visor.ponerCapa(c.nombre, encendida: $0) })) {
                    Text(c.enElPlano ? c.nombre : "\(c.nombre) (apagada en el plano)")
                        .foregroundStyle(c.enElPlano ? .primary : .secondary)
                        .lineLimit(1)
                }
                .listRowBackground(Color.clear)
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
        }
    }
}

/// Los marcadores del plano: un redondel numerado por marcador, que lleva a
/// su vista (mantener pulsado: quitarlo).
struct MarcadoresDelPlano: View {
    @ObservedObject var visor: VisorPlano
    @ObservedObject var marcadores: MarcadoresDeVista

    var body: some View {
        HStack(spacing: 6) {
            ForEach(Array(marcadores.lista.enumerated()), id: \.element.id) { i, m in
                Button { visor.ir(a: m) } label: {
                    Text("\(i + 1)").font(.system(size: 13, weight: .semibold)).foregroundStyle(.white)
                        .frame(width: 32, height: 32)
                        .background(Circle().fill(Color(white: 0.12, opacity: 0.7)))
                }
                .buttonStyle(.plain)
                .help("Marcador \(i + 1)")
                .contextMenu { Button("Quitar", role: .destructive) { marcadores.quitar(m) } }
            }
        }
    }
}

/// **Las letras del plano** («Aa», `L` de Windows y Android): las que el
/// plano pide y no estan, y añadir o quitar las del usuario (SHX, TTF, OTF,
/// TTC). Las que se añaden mandan sobre las del iPad y las de PixPin.
struct LetrasDelPlanoIPad: View {
    @ObservedObject var visor: VisorPlano
    @Environment(\.dismiss) private var cerrar
    @State private var eligiendo = false

    /// Lo que se deja elegir en Archivos.
    static var tipos: [UTType] {
        ["shx", "ttf", "otf", "ttc"].compactMap { UTType(filenameExtension: $0) } + [.font]
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    if visor.faltan.isEmpty {
                        Text("No falta ninguna letra que el plano diga por su nombre.")
                    } else {
                        Text("Este plano pide letras que no están; sus textos salen con otras parecidas y pueden no caber en sus recuadros:")
                        ForEach(visor.faltan, id: \.self) { Text($0).font(.body.monospaced()) }
                    }
                }
                Section {
                    Text("Las SHX suelen estar en la carpeta Fonts de AutoCAD o junto al plano que te pasaron; las TrueType, en Archivos. Las que añadas aquí mandan sobre las del iPad y las de PixPin.")
                        .font(.callout).foregroundStyle(.secondary)
                    Button("Añadir letras (SHX, TTF, OTF, TTC)…") { eligiendo = true }
                    Button("Quitar las mías (\(visor.letrasMias))", role: .destructive) {
                        visor.quitarLetras()
                        cerrar()
                    }
                    .disabled(visor.letrasMias == 0)
                }
            }
            .navigationTitle("Letras del plano")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Listo") { cerrar() } } }
            .fileImporter(isPresented: $eligiendo, allowedContentTypes: LetrasDelPlanoIPad.tipos,
                          allowsMultipleSelection: true) { r in
                if case let .success(urls) = r, !urls.isEmpty {
                    visor.anadirLetras(urls)
                    cerrar()
                }
            }
        }
    }
}

/// Un panel que flota arriba a la derecha, bajo la barra del visor: las
/// capas del plano o los niveles del modelo, solo al pedirlos.
struct PanelFlotantePlano<Contenido: View>: View {
    var alto: CGFloat = 420
    @ViewBuilder let contenido: () -> Contenido

    var body: some View {
        contenido()
            .frame(width: 300)
            .frame(height: alto)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(.separator))
            .shadow(color: .black.opacity(0.18), radius: 10, y: 4)
            .padding(.top, 64)
            .padding(.trailing, 14)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
    }
}
