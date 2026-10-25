// «Enviar y recibir por Wi-Fi» con un codigo de seis cifras, sin grupo:
// `sincronizar/enviar_wifi.rs` y `recibir_wifi.rs` de Windows (y
// `EnviarActivity`/`RecibirActivity` de Android). Los dos sentidos:
//
// - **Enviar y esperar**: se eligen archivos, se abre una puerta con un
//   codigo (y su QR `pixpin-envio:1:…`), se anuncia `_pixpinenvio._tcp` y
//   cada aparato que llega con el codigo espera a que se le apruebe.
// - **Esperar para recibir**: un codigo (y su QR `pixpin-recibe:1:…`) y se
//   anuncia `_pixpinrecibe._tcp`; quien envia lo teclea y manda.
// - **Tengo un codigo**: el del otro (o el texto de su QR): si es de quien
//   envia, se recibe; si es de quien espera, se le mandan archivos.
//
// Se mandan **archivos, un proyecto entero o lienzos** (un `.excalidraw`
// sale como lienzo, que el movil abre en su editor). Al recibir, **antes de
// aceptar** se ve lo que llega y, de lo que ya se tiene (los tres codigos),
// se elige «Actualizar el que tengo» o «Crear como nuevo»
// (`RecibirActivity.Oferta` del movil, `recibir_wifi.rs` de Windows).
//
// La red y el protocolo son de Rust (`PuertaEnvio`, `PuertaRecibir`,
// `recibirEnvio`, `mandarAQuienEspera`); aqui solo la pantalla, los hilos
// que esperan en las puertas y el anuncio. Al cerrar la ventana el codigo
// deja de existir.
//
// iPad: copia del Mac con `UIImage`, el selector de Archivos en vez de
// `NSOpenPanel` y la camara para leer el QR del otro.

import PixPinCore
import SwiftUI
import UIKit

@MainActor
public final class ModeloEnvio: ObservableObject {
    /// Un aparato que llamo a la puerta de envio.
    public struct Llamada: Identifiable, Equatable {
        public var id: UInt64
        public var nombre: String
        public var esperaAprobacion: Bool
        public var hechos: UInt64 = 0
        public var total: UInt64 = 0
        public var fin: String?
    }

    /// Lo abierto ahora.
    public enum Abierta: Equatable {
        case nada
        case enviando(codigo: String, archivos: [String])
        case esperando(codigo: String)
    }

    /// Lo que se manda: archivos (y lienzos) o un proyecto de la lista.
    public enum QueMandar: Equatable, Sendable {
        case archivos([URL])
        case proyecto(ficha: String, nombre: String)
    }

    @Published public private(set) var abierta: Abierta = .nada
    @Published public private(set) var qr: UIImage?
    @Published public private(set) var textoDelQr: String?
    @Published public private(set) var llamadas: [Llamada] = []
    /// Lo que se recibio (en esta ventana).
    @Published public private(set) var recibido: [RecibidoEnvio] = []
    @Published public private(set) var fase: String?
    @Published public private(set) var aviso: String?
    @Published public private(set) var error: String?
    /// **Lo que ofrece quien envia, esperando** «Aceptar» o «No, gracias».
    @Published public var oferta: OfertaPendiente?

    public let nucleo: Nucleo
    private var puertaEnvio: PuertaEnvio?
    private var puertaRecibir: PuertaRecibir?
    private let anuncio = AnuncioDePuerta()
    private let nombre: String

    public init(nucleo: Nucleo, nombre: String) {
        self.nucleo = nucleo
        self.nombre = nombre
    }

    // MARK: - Enviar y esperar

    /// Abre la puerta para enviar `urls`.
    public func enviar(_ urls: [URL]) async {
        await enviar(.archivos(urls))
    }

    /// Abre la puerta para enviar archivos o un proyecto entero.
    public func enviar(_ que: QueMandar) async {
        cerrar()
        error = nil
        let nucleo = self.nucleo
        fase = "Preparando…"
        do {
            let p = try await Task.detached(priority: .utility) { () throws -> PuertaEnvio in
                switch que {
                case let .archivos(urls): try nucleo.abrirEnvio(rutas: urls.map(\.path))
                case let .proyecto(ficha, _): try nucleo.abrirEnvioDeProyecto(ficha: ficha)
                }
            }.value
            fase = nil
            puertaEnvio = p
            abierta = .enviando(codigo: p.codigo(), archivos: nombres(de: que))
            ponerQR(p.textoDelQr(host: ipv4Local()))
            let nombre = self.nombre
            anuncio.anunciar({ p.anuncio(nombre: nombre) }, puerto: p.puerto())
            atenderEnHilo(nombre: "pixpin-envio-puerta", { try p.atender() },
                          esFin: { $0 == .cerrada || $0 == .quemado }) { _ in }
            escucharAvisos(de: p)
        } catch {
            fase = nil
            self.error = textoDeError(error)
        }
    }

    private func nombres(de que: QueMandar) -> [String] {
        switch que {
        case let .archivos(urls): urls.map(\.lastPathComponent)
        case let .proyecto(_, nombre): ["el proyecto «\(nombre)»"]
        }
    }

    /// El `DecidirOferta` del nucleo: el hilo que recibe se queda esperando
    /// hasta que la ventana contesta.
    private func puenteOferta() -> PuenteOferta {
        PuenteOferta { [weak self] oferta, responder in
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let self else { return responder(nil) }
                    self.oferta?.contestar(nil)
                    self.oferta = OfertaPendiente(oferta, responder: responder)
                }
            }
        }
    }

    /// Los avisos de la puerta (quien llega, cuanto va), en un hilo que
    /// duerme en `siguienteAviso()` hasta que hay uno.
    private func escucharAvisos(de p: PuertaEnvio) {
        let hilo = Thread { [weak self] in
            while true {
                let a = p.siguienteAviso()
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { self?.recibirAviso(a) }
                }
                if a == .cerrada { return }
            }
        }
        hilo.name = "pixpin-envio-avisos"
        hilo.qualityOfService = .utility
        hilo.start()
    }

    private func recibirAviso(_ a: AvisoEnvio) {
        switch a {
        case let .llega(llamada, nombre):
            llamadas.append(Llamada(id: llamada, nombre: nombre, esperaAprobacion: true))
        case let .avance(llamada, hechos, total):
            if let i = llamadas.firstIndex(where: { $0.id == llamada }) {
                llamadas[i].hechos = hechos
                llamadas[i].total = total
                llamadas[i].esperaAprobacion = false
            }
        case let .fin(llamada, fin):
            let texto = textoDeFin(fin)
            if let i = llamadas.firstIndex(where: { $0.id == llamada }) {
                llamadas[i].fin = texto
                llamadas[i].esperaAprobacion = false
            } else if case .codigoMalo = fin {
                aviso = texto
            } else if case .quemado = fin {
                aviso = texto
            }
        case .cerrada:
            if case .enviando = abierta { abierta = .nada }
        }
    }

    public func aprobar(_ llamada: UInt64, _ si: Bool) {
        puertaEnvio?.aprobar(llamada: llamada, si: si)
        if let i = llamadas.firstIndex(where: { $0.id == llamada }) {
            llamadas[i].esperaAprobacion = false
            if !si { llamadas[i].fin = "No aprobado" }
        }
    }

    public func aprobarTodos() {
        puertaEnvio?.aprobarTodos()
        for i in llamadas.indices { llamadas[i].esperaAprobacion = false }
    }

    // MARK: - Esperar para recibir

    public func esperarParaRecibir() async {
        cerrar()
        error = nil
        let nucleo = self.nucleo
        do {
            let p = try await Task.detached(priority: .utility) { try nucleo.esperarEnvios() }.value
            p.preguntarCon(decidir: puenteOferta())
            puertaRecibir = p
            abierta = .esperando(codigo: p.codigo())
            ponerQR(p.textoDelQr(host: ipv4Local()))
            let nombre = self.nombre
            anuncio.anunciar({ p.anuncio(nombre: nombre) }, puerto: p.puerto())
            atenderEnHilo(nombre: "pixpin-recibir-puerta", { try p.atender() },
                          esFin: { $0 == .cerrada || $0 == .quemado }) { [weak self] r in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { self?.recibio(r) }
                }
            }
        } catch {
            self.error = textoDeError(error)
        }
    }

    private func recibio(_ r: Result<FinRecibir, Error>) {
        switch r {
        case let .success(.recibido(de, cosas)):
            oferta = nil
            recibido.append(contentsOf: cosas)
            aviso = textoDeRecibido(cosas, de: de)
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        case .success(.rechazado):
            oferta = nil
            aviso = "No se aceptó."
        case let .success(.codigoMalo(intentos)):
            aviso = "Alguien probó otro código (\(intentos) de 5)."
        case .success(.quemado):
            aviso = "Demasiados códigos equivocados: se cerró."
            abierta = .nada
        case let .success(.fallo(detalle)):
            aviso = detalle
        case .success(.cerrada):
            break
        case let .failure(e):
            aviso = textoDeError(e)
        }
    }

    // MARK: - Tengo un codigo

    /// Con el codigo (o el texto del QR) de otro: recibe de quien envia o,
    /// si es de quien espera para recibir, le manda `archivos` (que hay que
    /// elegir antes).
    public func usarCodigo(_ texto: String, archivos: [URL] = []) async {
        await usarCodigo(texto, mandar: archivos.isEmpty ? nil : .archivos(archivos))
    }

    /// Lo mismo, mandando archivos o un proyecto entero a quien espera.
    public func usarCodigo(_ texto: String, mandar: QueMandar?) async {
        error = nil
        guard let c = leerCodigoDeEnvio(texto: texto) else {
            error = "Eso no es un código de envío: son seis cifras (o el texto de su QR)."
            return
        }
        if c.esperaRecibir && mandar == nil {
            error = "Ese código es de quien espera para recibir: elige qué mandarle."
            return
        }
        var destino: Direccion?
        if let host = c.host, c.puerto > 0 {
            destino = Direccion(host: host, puerto: c.puerto)
        } else {
            fase = "Buscando a quien tiene ese código en esta Wi-Fi…"
            let codigo = c.codigo
            let etiqueta = await Task.detached(priority: .utility) { etiquetaDeEnvio(codigo: codigo) }.value
            let tipo = c.esperaRecibir ? tipoDeServicioRecibir() : tipoDeServicioEnvio()
            destino = await buscarPuerta(tipo: tipo, etiqueta: etiqueta)
        }
        guard let destino else {
            fase = nil
            error = "No apareció nadie con ese código. ¿Tiene PixPin abierto en la misma Wi-Fi?"
            return
        }
        let nucleo = self.nucleo
        do {
            if c.esperaRecibir, let mandar {
                fase = "Enviando…"
                let fin = try await Task.detached(priority: .utility) { () throws -> FinEnvio in
                    switch mandar {
                    case let .archivos(urls):
                        try nucleo.mandarAQuienEspera(host: destino.host, puerto: destino.puerto,
                                                      codigo: c.codigo, rutas: urls.map(\.path))
                    case let .proyecto(ficha, _):
                        try nucleo.mandarProyectoAQuienEspera(host: destino.host, puerto: destino.puerto,
                                                              codigo: c.codigo, ficha: ficha)
                    }
                }.value
                aviso = textoDeFin(fin)
            } else {
                fase = "Esperando a que te aprueben y recibiendo…"
                let puente = puenteOferta()
                let cosas = try await Task.detached(priority: .utility) {
                    try nucleo.recibirEnvioPreguntando(host: destino.host, puerto: destino.puerto,
                                                       codigo: c.codigo, decidir: puente)
                }.value
                oferta = nil
                if let cosas {
                    recibido.append(contentsOf: cosas)
                    aviso = textoDeRecibido(cosas)
                    NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                } else {
                    aviso = "No se aceptó."
                }
            }
        } catch {
            oferta = nil
            self.error = textoDeError(error)
        }
        fase = nil
    }

    // MARK: - Cerrar

    /// Cierra lo abierto: el codigo deja de valer y se deja de anunciar.
    public func cerrar() {
        oferta?.contestar(nil)
        oferta = nil
        puertaEnvio?.cerrar()
        puertaEnvio = nil
        puertaRecibir?.cerrar()
        puertaRecibir = nil
        anuncio.retirar()
        abierta = .nada
        qr = nil
        textoDelQr = nil
        llamadas = []
    }

    private func ponerQR(_ texto: String) {
        textoDelQr = texto
        qr = imagenQR(texto)
    }
}

/// Como acabo un envio, en palabras.
public func textoDeFin(_ fin: FinEnvio) -> String {
    switch fin {
    case .cerrada: "Cerrado."
    case let .enviado(nombre): "Enviado a \(nombre)."
    case let .rechazado(nombre): "\(nombre) dijo que no."
    case let .sinAprobar(nombre): "No se aprobó a \(nombre)."
    case let .codigoMalo(intentos): "Alguien probó otro código (\(intentos) de 5)."
    case .quemado: "Demasiados códigos equivocados: se cerró."
    case let .fallo(detalle): detalle
    }
}

/// Lo recibido, en palabras: una linea por cosa, donde quedo (`rw-en-*` y
/// `rw-*-al-dia` de Windows).
public func textoDeRecibido(_ cosas: [RecibidoEnvio], de: String = "") -> String {
    guard !cosas.isEmpty else { return "No llegó nada." }
    let lineas = cosas.map { c -> String in
        switch c.donde {
        case "proyecto_al_dia": "«\(c.nombre)» puesto al día"
        case "guardados_al_dia": "«\(c.nombre)» puesto al día en Mensajes guardados"
        case "guardados": "\(c.nombre): en «Mensajes guardados»"
        case "carpeta": "\(c.nombre): solo en la carpeta «recibidos»"
        default: "\(c.nombre): en la lista de proyectos"
        }
    }
    return (de.isEmpty ? "Recibido" : "Recibido de \(de)") + "\n" + lineas.joined(separator: "\n")
}

/// **Lo que ofrece quien envia, esperando respuesta**: lo que se ve y como
/// contestarle. Contestar una vez basta; las demas no hacen nada (cerrar
/// la ventana contesta «No, gracias»: un hilo esperando para siempre
/// dejaria la puerta colgada).
public final class OfertaPendiente: Identifiable, @unchecked Sendable {
    public let id = UUID()
    public let oferta: OfertaRecibida
    private let responder: @Sendable ([String]?) -> Void
    private let cerrojo = NSLock()
    private var contestada = false

    init(_ oferta: OfertaRecibida, responder: @escaping @Sendable ([String]?) -> Void) {
        self.oferta = oferta
        self.responder = responder
    }

    /// Las identidades que se crean como nuevas, o `nil` («No, gracias»).
    public func contestar(_ comoNuevo: [String]?) {
        cerrojo.lock()
        let ya = contestada
        contestada = true
        cerrojo.unlock()
        if !ya { responder(comoNuevo) }
    }
}

/// Lo que se va eligiendo en la oferta: puro, para probarlo sin pantalla.
public struct EligiendoOferta: Equatable {
    public let oferta: OfertaRecibida
    /// Las identidades que se crean como nuevas (lo demas que ya se tiene
    /// se pone al dia, que es lo que viene puesto en el movil).
    public var comoNuevo: Set<String> = []

    public init(_ oferta: OfertaRecibida) {
        self.oferta = oferta
    }

    public var nadaCoincide: Bool { oferta.cosas.allSatisfy { $0.yaTienes == nil } }

    public mutating func elegir(_ identidad: String, nuevo: Bool) {
        // Solo se elige en lo que ya se tiene.
        guard oferta.cosas.contains(where: { $0.identidad == identidad && $0.yaTienes != nil }) else { return }
        if nuevo { comoNuevo.insert(identidad) } else { comoNuevo.remove(identidad) }
    }

    /// Lo que se contesta al pulsar «Aceptar».
    public var respuesta: [String] { oferta.cosas.map(\.identidad).filter { comoNuevo.contains($0) } }

    /// Lo que se lee de una cosa (`nombre_de` de Windows).
    public static func nombre(_ c: CosaOfrecida) -> String {
        switch c.tipo {
        case "proyecto": "Proyecto «\(c.nombre)»"
        case "lienzo": "Lienzo «\(c.nombre)»" + (c.proyectoNombre.map { " de «\($0)»" } ?? "")
        default: c.nombre
        }
    }

    /// La explicacion bajo las dos pastillas (`rw-encima-de-lo-tuyo`,
    /// `rw-entra-aparte`).
    public static func explicacion(nuevo: Bool) -> String {
        nuevo ? "Entra aparte, con códigos nuevos: desde ahora es otra cosa."
            : "Se escribe encima de lo tuyo. Lo de antes queda en «Copias de seguridad»."
    }
}

/// El `DecidirOferta` del nucleo para la ventana.
final class PuenteOferta: DecidirOferta, @unchecked Sendable {
    typealias Mostrar = @Sendable (OfertaRecibida, @escaping @Sendable ([String]?) -> Void) -> Void
    private let mostrar: Mostrar

    init(_ mostrar: @escaping Mostrar) {
        self.mostrar = mostrar
    }

    func decidir(oferta: OfertaRecibida) -> [String]? {
        final class Caja: @unchecked Sendable { var valor: [String]? }
        let caja = Caja()
        let listo = DispatchSemaphore(value: 0)
        mostrar(oferta) { r in
            caja.valor = r
            listo.signal()
        }
        listo.wait()
        return caja.valor
    }
}

/// «X quiere enviarte»: la lista de lo que llega, con «Actualizar el que
/// tengo» / «Crear como nuevo» en lo que ya se tiene, y «No, gracias» /
/// «Aceptar».
struct VistaOferta: View {
    let pendiente: OfertaPendiente
    @State private var el: EligiendoOferta
    @Environment(\.dismiss) private var cerrar

    init(_ pendiente: OfertaPendiente) {
        self.pendiente = pendiente
        _el = State(initialValue: EligiendoOferta(pendiente.oferta))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("\(pendiente.oferta.de) quiere enviarte").font(.headline)
            if !pendiente.oferta.deCodigo.isEmpty {
                Text("Aparato \(pendiente.oferta.deCodigo)").font(.caption).foregroundStyle(.secondary)
            }
            List {
                ForEach(pendiente.oferta.cosas, id: \.identidad) { c in
                    fila(c).padding(.vertical, 2)
                }
            }
            .frame(minHeight: 160)
            if el.nadaCoincide {
                Text("Nada de esto coincide con lo que tienes: entra como nuevo, sin tocar nada tuyo.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            HStack {
                Spacer()
                Button("No, gracias") {
                    pendiente.contestar(nil)
                    cerrar()
                }
                .keyboardShortcut(.cancelAction)
                Button("Aceptar") {
                    pendiente.contestar(el.respuesta)
                    cerrar()
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(20)
        .frame(width: 460)
        // Cerrar la hoja de cualquier otra forma es decir que no.
        .onDisappear { pendiente.contestar(nil) }
    }

    private func fila(_ c: CosaOfrecida) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(EligiendoOferta.nombre(c))
                Spacer()
                Text(ByteCountFormatter.string(fromByteCount: max(c.bytes, 0), countStyle: .file))
                    .foregroundStyle(.secondary)
            }
            if let suyo = c.yaTienes {
                let nuevo = el.comoNuevo.contains(c.identidad)
                Text("Ya tienes «\(suyo)» con los mismos códigos").font(.caption)
                    .foregroundStyle(Color.accentColor)
                Picker("", selection: Binding(
                    get: { nuevo },
                    set: { el.elegir(c.identidad, nuevo: $0) }
                )) {
                    Text("Actualizar el que tengo").tag(false)
                    Text("Crear como nuevo").tag(true)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                Text(EligiendoOferta.explicacion(nuevo: nuevo)).font(.caption).foregroundStyle(.secondary)
            }
        }
    }
}

/// La ventana «Enviar y recibir por Wi-Fi».
public struct VistaEnvio: View {
    @StateObject private var modelo: ModeloEnvio
    @State private var codigo = ""
    /// Elegir un proyecto: para abrir la puerta (`false`) o para mandarlo
    /// a quien espera con el codigo escrito (`true`).
    @State private var eligiendoProyecto: Bool?
    /// La sincronizacion, para los aparatos del grupo (sin codigo).
    let sincro: Sincronizador?

    /// Archivos abierto, y para que: abrir la puerta (`false`) o mandarlos
    /// a quien espera con el codigo escrito (`true`). El para que no se
    /// borra al cerrarse Archivos: lo elegido llega despues (se copia antes).
    @State private var eligiendoArchivos = false
    @State private var archivosAQuienEspera = false
    @State private var escaneando = false

    public init(nucleo: Nucleo, sincro: Sincronizador? = nil, nombre: String) {
        _modelo = StateObject(wrappedValue: ModeloEnvio(nucleo: nucleo, nombre: nombre))
        self.sincro = sincro
    }

    public var body: some View {
        Form {
            switch modelo.abierta {
            case .nada:
                if let sincro, sincro.codigoLegible != nil {
                    TusAparatos(sincro: sincro)
                }
                Section {
                    Button("Elegir archivos y enviar…") {
                        archivosAQuienEspera = false
                        eligiendoArchivos = true
                    }
                    .help("Un lienzo (.excalidraw) sale como lienzo: el móvil lo abre en su editor.")
                    Button("Enviar un proyecto…") { eligiendoProyecto = false }
                    Button("Esperar para recibir") { Task { await modelo.esperarParaRecibir() } }
                } header: {
                    Text("Sin grupo, con un código de seis cifras")
                }
                Section {
                    HStack {
                        TextField("Código (482 913) o texto del QR", text: $codigo)
                            .font(.system(.body, design: .monospaced))
                            .keyboardType(.numbersAndPunctuation)
                            .autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                        Button {
                            escaneando = true
                        } label: {
                            Label("Escanear", systemImage: "qrcode.viewfinder")
                        }
                        .disabled(modelo.fase != nil)
                    }
                    HStack(spacing: 18) {
                        Button("Recibir") { Task { await modelo.usarCodigo(codigo) } }
                        Button("Enviarle archivos…") {
                            archivosAQuienEspera = true
                            eligiendoArchivos = true
                        }
                        Button("Enviarle un proyecto…") { eligiendoProyecto = true }
                    }
                    .disabled(leerCodigoDeEnvio(texto: codigo) == nil || modelo.fase != nil)
                } header: {
                    Text("Tengo el código de otro aparato")
                }
            case let .enviando(c, archivos):
                abierta(codigo: c, titulo: "Enviando \(archivos.count == 1 ? archivos[0] : "\(archivos.count) archivos")")
                Section("Quien llega") {
                    if modelo.llamadas.isEmpty {
                        Text("Esperando a que alguien teclee el código…").foregroundStyle(.secondary)
                    }
                    ForEach(modelo.llamadas) { l in
                        FilaLlamada(llamada: l, aprobar: { modelo.aprobar(l.id, $0) })
                    }
                    if modelo.llamadas.contains(where: \.esperaAprobacion) {
                        Button("Aprobar a todos") { modelo.aprobarTodos() }
                    }
                }
            case let .esperando(c):
                abierta(codigo: c, titulo: "Esperando para recibir")
            }

            if modelo.fase != nil || modelo.aviso != nil || modelo.error != nil {
                Section("Actividad") {
                    if let f = modelo.fase { HStack { ProgressView().controlSize(.small); Text(f) } }
                    if let a = modelo.aviso { Text(a) }
                    if let e = modelo.error {
                        Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                    }
                }
            }
        }
        .formStyle(.grouped)
        // iPad: varios botones en una fila de `Form`, cada uno con su toque.
        .buttonStyle(.borderless)
        .navigationTitle("Enviar y recibir por Wi‑Fi")
        .onDisappear { modelo.cerrar() }
        .elegirArchivosParaEnviar($eligiendoArchivos) { urls in
            let aQuienEspera = archivosAQuienEspera
            Task {
                if aQuienEspera {
                    await modelo.usarCodigo(codigo, archivos: urls)
                } else {
                    await modelo.enviar(urls)
                }
            }
        }
        .fullScreenCover(isPresented: $escaneando) {
            HojaEscanerQR(titulo: "Escanear el código",
                          ayuda: "Apunta al QR de «Enviar» o «Recibir» del otro aparato.",
                          acepta: { leerCodigoDeEnvio(texto: $0) != nil }) { texto in
                codigo = texto
                // El de quien envia se recibe ya; el de quien espera pide
                // elegir que mandarle.
                if leerCodigoDeEnvio(texto: texto)?.esperaRecibir == false {
                    Task { await modelo.usarCodigo(texto) }
                }
            }
        }
        .sheet(item: $modelo.oferta) { o in VistaOferta(o) }
        .sheet(isPresented: Binding(get: { eligiendoProyecto != nil },
                                    set: { if !$0 { eligiendoProyecto = nil } })) {
            ElegirProyecto(nucleo: modelo.nucleo) { ficha, nombre in
                let aQuienEspera = eligiendoProyecto == true
                eligiendoProyecto = nil
                Task {
                    if aQuienEspera {
                        await modelo.usarCodigo(codigo, mandar: .proyecto(ficha: ficha, nombre: nombre))
                    } else {
                        await modelo.enviar(.proyecto(ficha: ficha, nombre: nombre))
                    }
                }
            }
        }
    }

    @ViewBuilder private func abierta(codigo: String, titulo: String) -> some View {
        Section {
            VStack(spacing: 10) {
                Text(codigoDeEnvioLegible(codigo: codigo))
                    .font(.system(size: 34, weight: .semibold, design: .monospaced))
                    .textSelection(.enabled)
                if let qr = modelo.qr {
                    Image(uiImage: qr).interpolation(.none).resizable().frame(width: 220, height: 220)
                }
                Button("Cerrar") { modelo.cerrar() }
            }
            .frame(maxWidth: .infinity)
        } header: {
            Text(titulo)
        } footer: {
            Text("En el otro aparato: Enviar o Recibir por Wi-Fi, con este código o escaneando el QR. Vale mientras esta ventana esté abierta.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }

}

struct FilaLlamada: View {
    let llamada: ModeloEnvio.Llamada
    let aprobar: (Bool) -> Void

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(llamada.nombre).font(.headline)
                if let fin = llamada.fin {
                    Text(fin).font(.caption).foregroundStyle(.secondary)
                } else if llamada.total > 0 {
                    ProgressView(value: Double(llamada.hechos), total: Double(max(llamada.total, 1)))
                }
            }
            Spacer()
            if llamada.esperaAprobacion {
                Button("No") { aprobar(false) }
                Button("Aprobar") { aprobar(true) }.keyboardShortcut(.defaultAction)
                    .buttonStyle(.borderedProminent)
            }
        }
    }
}

/// «Elige el proyecto» (`ew-elige-proyecto`): la lista de proyectos para
/// mandar uno entero.
struct ElegirProyecto: View {
    let nucleo: Nucleo
    let elegido: (String, String) -> Void
    @State private var lista: [FichaChat] = []
    @State private var leida = false
    @Environment(\.dismiss) private var cerrar

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Elige el proyecto").font(.headline)
            if leida && lista.isEmpty {
                Text("Todavía no hay proyectos.").foregroundStyle(.secondary)
            }
            List(lista, id: \.id) { f in
                Button {
                    elegido(f.id, f.nombre)
                    cerrar()
                } label: {
                    Text(f.nombre).frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.plain)
            }
            .frame(minHeight: 220)
            HStack {
                Spacer()
                Button("Cancelar") { cerrar() }.keyboardShortcut(.cancelAction)
            }
        }
        .padding(20)
        .frame(width: 380)
        .task {
            let nucleo = self.nucleo
            lista = (try? await Task.detached(priority: .userInitiated) { try nucleo.chats() }.value) ?? []
            leida = true
        }
    }
}


/// **Tus aparatos · sin código** (Windows, 17-oct): los del grupo de
/// sincronizacion. Se envia directo por el canal cifrado, sin escanear;
/// llega a lo que tengan abierto (un chat; un lienzo solo acepta fotos) o a
/// Mensajes guardados.
struct TusAparatos: View {
    @ObservedObject var sincro: Sincronizador
    /// A quien se manda lo que se elija en Archivos.
    @State private var destinos: [String] = []
    @State private var eligiendo = false

    private var visibles: [String] {
        sincro.filas.filter { $0.direccion != nil }.map(\.id)
    }

    var body: some View {
        Section {
            if sincro.filas.isEmpty {
                Text("Ningún aparato del grupo en esta Wi-Fi").foregroundStyle(.secondary)
            }
            ForEach(sincro.filas, id: \.id) { fila in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(fila.nombre)
                        Text(estado(fila.id, seVe: fila.direccion != nil))
                            .font(.caption).foregroundStyle(.secondary).lineLimit(2)
                    }
                    Spacer()
                    Button("Enviar…") {
                        destinos = [fila.id]
                        eligiendo = true
                    }
                        .disabled(fila.direccion == nil)
                }
            }
            if visibles.count > 1 {
                Button("Enviar a todos los que se ven…") {
                    destinos = visibles
                    eligiendo = true
                }
            }
        } header: {
            Text("Tus aparatos · sin código")
        } footer: {
            Text("Están en tu grupo: se envía directo, sin escanear. Llega a lo que tengan abierto (un chat; un lienzo solo acepta fotos) o a Mensajes guardados.")
                .font(.caption).foregroundStyle(.secondary)
        }
        .elegirArchivosParaEnviar($eligiendo) { urls in
            let ids = destinos
            // Uno detras de otro: el nucleo deja una conexion por aparato.
            Task { for id in ids { await sincro.enviar(urls, a: id) } }
        }
    }

    private func estado(_ id: String, seVe: Bool) -> String {
        if let e = sincro.envios[id], let linea = e.texto.split(separator: "\n").first {
            return String(linea)
        }
        return seVe ? "En la misma Wi-Fi" : "No se ve ahora: abre PixPin en ese aparato"
    }
}
