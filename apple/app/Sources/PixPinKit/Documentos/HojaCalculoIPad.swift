// **La hoja de calculo** del iPad: una tabla del PC (mensaje `MINIAPP`
// tabla), una del movil (`TABLA`, `tablas/<ref>.json`) o un libro del chat
// (Excel, OpenDocument, CSV) con todas sus hojas en pestanas, guardado donde
// lo guarda Windows (`HojaDeCalculo` de Rust, el `hoja_pin` de Windows):
// cada hoja de un libro pasa a ser una tabla del chat y el archivo no cambia.
// Las formulas las calcula el motor de `pixpin-proyecto` (el mismo
// `=SUMA(...)` que Windows). Es `HojaCalculo.swift` del Mac con SwiftUI.
//
// Como en Windows se abre para ver; el lapiz de la barra trae la barra de
// formulas y el formato (negrita, cursiva, subrayado, tachado, alinear,
// ajustar, formato de numero, resaltar, color de letra, borrar formato),
// deshacer y rehacer, copiar, cortar y pegar (con Excel y Sheets: texto con
// tabuladores y tabla HTML). Con el teclado del iPad, ⌘B, ⌘I, ⌘U, ⌘Z...
//
// Rendimiento (§4bis): la rejilla es perezosa (solo existen las filas que se
// ven) y las celdas se piden a Rust por bloques de 100 filas, ya calculadas;
// un cambio vacia los bloques (una formula puede depender de cualquier celda)
// y se piden otra vez solo los que se ven. Guardar va 1 s despues del
// ultimo cambio, fuera del hilo principal.

import Combine
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// El texto de una celda con su formato de numero (`nf`: numero, moneda,
/// porcentaje, fecha), como lo ensena el pin de hoja de Windows.
public func textoDeCelda(_ c: CeldaHoja, locale: Locale = .current) -> String {
    guard let n = c.numero, let nf = c.estilo?.formatoNumero, !c.error else { return c.mostrado }
    let f = NumberFormatter()
    f.locale = locale
    switch nf {
    case "numero":
        f.numberStyle = .decimal
        f.minimumFractionDigits = 2
        f.maximumFractionDigits = 2
        f.usesGroupingSeparator = true
    case "moneda":
        f.numberStyle = .currency
    case "porcentaje":
        f.numberStyle = .percent
        f.minimumFractionDigits = 2
        f.maximumFractionDigits = 2
    case "fecha":
        // El numero es el serial de Excel: dias desde el 30-12-1899.
        let base = DateComponents(calendar: Calendar(identifier: .gregorian), timeZone: TimeZone(identifier: "UTC"),
                                  year: 1899, month: 12, day: 30).date ?? Date(timeIntervalSince1970: 0)
        let d = base.addingTimeInterval(n * 86_400)
        let df = DateFormatter()
        df.locale = locale
        df.timeZone = TimeZone(identifier: "UTC")
        df.dateStyle = .short
        df.timeStyle = .none
        return df.string(from: d)
    default:
        return c.mostrado
    }
    return f.string(from: NSNumber(value: n)) ?? c.mostrado
}

/// El nombre de una columna (`A`, `Z`, `AA`...), como `tabla::ref_a`.
public func letrasDeColumna(_ i: Int) -> String {
    var n = i + 1
    var s = ""
    while n > 0 {
        let r = (n - 1) % 26
        s = String(UnicodeScalar(UInt8(65 + r))) + s
        n = (n - 1) / 26
    }
    return s
}

/// Un color `#rrggbb` (el de los estilos de celda).
func colorDeHojaHex(_ hex: String?) -> Color? {
    guard var h = hex?.trimmingCharacters(in: .whitespaces), h.hasPrefix("#") else { return nil }
    h.removeFirst()
    guard h.count == 6, let v = UInt32(h, radix: 16) else { return nil }
    return Color(hex: v)
}

/// «B3» o «A1:C4»: lo elegido como lo escribe Excel.
public func referenciaDeRango(f0: Int, c0: Int, f1: Int, c1: Int) -> String {
    let a = "\(letrasDeColumna(min(c0, c1)))\(min(f0, f1) + 1)"
    if f0 == f1 && c0 == c1 { return a }
    return a + ":\(letrasDeColumna(max(c0, c1)))\(max(f0, f1) + 1)"
}

// MARK: - El modelo

@MainActor
final class ModeloHojaIPad: ObservableObject {
    let hoja: HojaDeCalculo
    /// Sube con cada cambio: la rejilla se vuelve a pintar.
    @Published private(set) var version = 0
    @Published private(set) var fila = 0
    @Published private(set) var columna = 0
    @Published private(set) var anclaFila = 0
    @Published private(set) var anclaColumna = 0
    /// Lo que se escribe en la barra de formulas.
    @Published var texto = ""
    /// Ver o editar (`visor_hoja.rs` de Windows).
    @Published var editando = false
    /// El proximo toque alarga lo elegido (el ⇧ + clic del Mac).
    @Published var extender = false
    @Published var aviso: String?
    @Published private(set) var nombres: [String] = []
    @Published private(set) var activa = 0
    @Published private(set) var puedeDeshacer = false
    @Published private(set) var puedeRehacer = false

    private(set) var columnas = 26
    private(set) var filas = 200
    private var bloques: [Int: [Int: [Int: CeldaHoja]]] = [:]
    private static let filasPorBloque = 100
    private var anchos: [Int: CGFloat] = [:]
    private let colaGuardar = DispatchQueue(label: "pixpin.hoja.guardar", qos: .utility)
    private var guardado: DispatchWorkItem?

    static let altoFila: CGFloat = 30
    static let anchoNumeros: CGFloat = 46

    init(hoja: HojaDeCalculo) {
        self.hoja = hoja
        recalcularTamano()
        nombres = hoja.hojas()
        activa = Int(hoja.activa())
        let s = hoja.seleccion()
        anclaFila = Int(s.anclaFila)
        anclaColumna = Int(s.anclaColumna)
        elegir(Int(s.fila), Int(s.columna), extender: true)
    }

    var rango: (f0: Int, c0: Int, f1: Int, c1: Int) {
        (min(fila, anclaFila), min(columna, anclaColumna), max(fila, anclaFila), max(columna, anclaColumna))
    }

    var referencia: String {
        referenciaDeRango(f0: anclaFila, c0: anclaColumna, f1: fila, c1: columna)
    }

    func ancho(_ c: Int) -> CGFloat { anchos[c] ?? 96 }

    /// El ancho puesto a mano (pixeles, los de Windows), si lo hay.
    func anchoPuesto(_ c: Int) -> CGFloat? { anchos[c] }

    /// Donde empieza la columna `c` en el contenido de la rejilla (despues
    /// de los numeros de las filas).
    func izquierda(_ c: Int) -> CGFloat {
        var x = ModeloHojaIPad.anchoNumeros
        for k in 0..<max(min(c, columnas), 0) { x += ancho(k) }
        return x
    }

    /// Lo que se ensena: lo escrito y un margen para seguir escribiendo.
    private func recalcularTamano() {
        let t = hoja.tamano()
        columnas = min(max(Int(t.first ?? 0) + 5, 26), 16_384)
        filas = max(Int(t.count > 1 ? t[1] : 0) + 50, 200)
        anchos = Dictionary(hoja.anchos().map { (Int($0.columna), CGFloat($0.ancho)) }, uniquingKeysWith: { a, _ in a })
    }

    func celda(_ f: Int, _ c: Int) -> CeldaHoja? {
        let b = f / ModeloHojaIPad.filasPorBloque
        if bloques[b] == nil {
            let desde = UInt32(b * ModeloHojaIPad.filasPorBloque)
            var mapa: [Int: [Int: CeldaHoja]] = [:]
            for x in hoja.celdas(desdeFila: desde, hastaFila: desde + UInt32(ModeloHojaIPad.filasPorBloque),
                                 desdeColumna: 0, hastaColumna: UInt32(columnas)) {
                mapa[Int(x.fila), default: [:]][Int(x.columna)] = x
            }
            bloques[b] = mapa
        }
        return bloques[b]?[f]?[c]
    }

    func dentro(_ f: Int, _ c: Int) -> Bool {
        let r = rango
        return (r.f0...r.f1).contains(f) && (r.c0...r.c1).contains(c)
    }

    // MARK: Elegir y escribir

    /// Elige una celda; con `extender` alarga lo elegido desde el ancla.
    func elegir(_ f: Int, _ c: Int, extender: Bool = false) {
        fila = max(0, min(f, filas - 1))
        columna = max(0, min(c, columnas - 1))
        if !extender {
            anclaFila = fila
            anclaColumna = columna
        }
        hoja.elegir(s: SeleccionHoja(anclaFila: UInt32(anclaFila), anclaColumna: UInt32(anclaColumna),
                                     fila: UInt32(fila), columna: UInt32(columna)))
        texto = hoja.contenido(fila: UInt32(fila), columna: UInt32(columna))
    }

    /// Un toque en una celda. `true` si ya era la activa (editando, se
    /// empieza a escribir en ella).
    @discardableResult
    func tocar(_ f: Int, _ c: Int) -> Bool {
        let era = f == fila && c == columna && anclaFila == fila && anclaColumna == columna
        elegir(f, c, extender: extender)
        extender = false
        return era
    }

    /// Lo de la barra de formulas a la celda; `bajar` pasa a la de abajo.
    func confirmar(bajar: Bool = true) {
        guard editando else { return }
        if hoja.poner(fila: UInt32(fila), columna: UInt32(columna), contenido: texto) {
            trasCambio()
        }
        if bajar { elegir(fila + 1, columna) }
    }

    func cancelar() {
        texto = hoja.contenido(fila: UInt32(fila), columna: UInt32(columna))
    }

    func mover(dx: Int, dy: Int, extender: Bool = false) {
        elegir(fila + dy, columna + dx, extender: extender)
    }

    // MARK: Hacer

    func formato(_ f: FormatoHoja) {
        guard editando else { return }
        if hoja.formato(f: f) { trasCambio() }
    }

    func deshacer() { deshacerORehacer(true) }
    func rehacer() { deshacerORehacer(false) }

    private func deshacerORehacer(_ atras: Bool) {
        guard editando else { return }
        let antes = hoja.activa()
        guard atras ? hoja.deshacer() : hoja.rehacer() else { return }
        if hoja.activa() != antes {
            trasCambioDeHoja()
        } else {
            let s = hoja.seleccion()
            anclaFila = Int(s.anclaFila)
            anclaColumna = Int(s.anclaColumna)
            trasCambio()
            elegir(Int(s.fila), Int(s.columna), extender: true)
        }
    }

    func copiar(cortar: Bool = false) {
        let copia = hoja.copiar()
        UIPasteboard.general.setItems([[
            UTType.utf8PlainText.identifier: copia.texto,
            UTType.html.identifier: copia.html,
        ]])
        if cortar, editando, hoja.borrarSeleccion() {
            trasCambio()
            texto = ""
        }
    }

    func pegar() {
        guard editando else { return }
        let pb = UIPasteboard.general
        let html = pb.data(forPasteboardType: UTType.html.identifier).flatMap { String(data: $0, encoding: .utf8) }
            ?? pb.value(forPasteboardType: UTType.html.identifier) as? String
        if hoja.pegar(html: html, texto: pb.string) {
            let s = hoja.seleccion()
            anclaFila = Int(s.anclaFila)
            anclaColumna = Int(s.anclaColumna)
            trasCambio()
            elegir(Int(s.fila), Int(s.columna), extender: true)
        }
    }

    func borrar() {
        guard editando else { return }
        if hoja.borrarSeleccion() { trasCambio() }
        texto = ""
    }

    func elegirTodo() {
        let t = hoja.tamano()
        anclaFila = 0
        anclaColumna = 0
        elegir(max(Int(t.count > 1 ? t[1] : 1) - 1, 0), max(Int(t.first ?? 1) - 1, 0), extender: true)
    }

    func cambiarAncho(_ c: Int, _ ancho: CGFloat) {
        guard editando else { return }
        let a = max(ancho, 24).rounded()
        anchos[c] = a
        if hoja.ponerAncho(columna: UInt32(c), ancho: UInt32(a)) { programarGuardado() }
        version += 1
    }

    func irAHoja(_ i: Int) {
        let n = hoja.hojas().count
        guard n > 0 else { return }
        if editando { confirmar(bajar: false) }
        hoja.cambiarHoja(hoja: UInt32((i % n + n) % n))
        trasCambioDeHoja()
    }

    /// Otra hoja (o deshacer llevo a otra): todo lo que se ve es nuevo.
    private func trasCambioDeHoja() {
        bloques = [:]
        recalcularTamano()
        nombres = hoja.hojas()
        activa = Int(hoja.activa())
        let s = hoja.seleccion()
        anclaFila = Int(s.anclaFila)
        anclaColumna = Int(s.anclaColumna)
        elegir(Int(s.fila), Int(s.columna), extender: true)
        estadoDeshacer()
        version += 1
    }

    /// Tras un cambio: todo se vuelve a calcular y se repinta lo que se ve.
    private func trasCambio() {
        bloques = [:]
        recalcularTamano()
        programarGuardado()
        estadoDeshacer()
        version += 1
        // La primera vez que se edita un libro: el archivo no cambia.
        if hoja.avisoAlEditar() {
            aviso = hoja.esSuelta()
                ? "El archivo original no cambia: comparte una copia con lo editado («Compartir una copia en Excel»)."
                : "El archivo original no cambia: lo editado queda en el chat como tablas (el móvil las ve igual)."
        }
    }

    private func estadoDeshacer() {
        puedeDeshacer = hoja.puedeDeshacer()
        puedeRehacer = hoja.puedeRehacer()
    }

    // MARK: Guardar

    private func programarGuardado() {
        guardado?.cancel()
        let hoja = self.hoja
        let tarea = DispatchWorkItem { ModeloHojaIPad.guardarYa(hoja) }
        guardado = tarea
        colaGuardar.asyncAfter(deadline: .now() + 1.0, execute: tarea)
    }

    /// Al cerrar: lo escrito en la barra y lo pendiente, ya.
    func cerrar() {
        if editando, texto != hoja.contenido(fila: UInt32(fila), columna: UInt32(columna)) {
            confirmar(bajar: false)
        }
        guardado?.cancel()
        let hoja = self.hoja
        colaGuardar.async { ModeloHojaIPad.guardarYa(hoja) }
    }

    private nonisolated static func guardarYa(_ hoja: HojaDeCalculo) {
        do {
            if try hoja.guardar() {
                DispatchQueue.main.async {
                    NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                }
            }
        } catch {
            NSLog("PixPin: no se pudo guardar la hoja: %@", "\(error)")
        }
    }
}

// MARK: - Las vistas

/// **La hoja del chat** (`TABLA` o `MINIAPP` tabla, o un libro adjunto).
public struct HojaCalculoIPad: View {
    let almacen: Almacen
    let chat: String
    let mensaje: MensajeChat
    /// Si no se puede abrir como hoja, se ensena este fichero con Vista
    /// Rapida (un libro adjunto que el nucleo no lee).
    var respaldo: URL?
    @State private var modelo: ModeloHojaIPad?
    @State private var error: String?

    public init(almacen: Almacen, chat: String, mensaje: MensajeChat) {
        self.almacen = almacen
        self.chat = chat
        self.mensaje = mensaje
    }

    init(almacen: Almacen, chat: String, mensaje: MensajeChat, respaldo: URL?) {
        self.init(almacen: almacen, chat: chat, mensaje: mensaje)
        self.respaldo = respaldo
    }

    private var titulo: String { mensaje.nombre.isEmpty ? "Tabla" : mensaje.nombre }

    public var body: some View {
        Group {
            if let modelo {
                VistaHojaIPad(modelo: modelo, titulo: titulo, nucleo: almacen.nucleo,
                              fichero: modelo.hoja.rutaOriginal().map { URL(fileURLWithPath: $0) } ?? respaldo)
            } else if error != nil, let respaldo {
                VistaRapida(url: respaldo)
            } else {
                VStack(spacing: 0) {
                    BarraVisorIPad(tipo: "Hoja de cálculo", icono: "tablecells", titulo: titulo)
                    Spacer()
                    if let error {
                        Label(error, systemImage: "exclamationmark.triangle")
                            .foregroundStyle(.secondary).padding()
                    } else {
                        ProgressView()
                    }
                    Spacer()
                }
            }
        }
        .task { await abrir() }
    }

    private func abrir() async {
        guard modelo == nil else { return }
        let nucleo = almacen.nucleo, chat = self.chat, id = mensaje.id
        let hecho = await Task.detached(priority: .userInitiated) { () -> Result<HojaDeCalculo, Error> in
            Result { try nucleo.abrirHojaDeCalculo(chat: chat, mensaje: id) }
        }.value
        switch hecho {
        case let .success(h): modelo = ModeloHojaIPad(hoja: h)
        case let .failure(e): error = "No se puede abrir la hoja: \(e)"
        }
    }
}

struct VistaHojaIPad: View {
    @ObservedObject var modelo: ModeloHojaIPad
    let titulo: String
    /// El libro que se ve (para «Fijar como pin», la tinta y los graficos).
    var fichero: URL?
    @FocusState private var escribiendo: Bool
    @ObservedObject private var noche = ModoNoche.compartido
    /// La tinta, los marcadores y los graficos (`TintaHojaIPad.swift`).
    @StateObject private var tinta: ControladorTintaHoja
    @State private var compartiendo = false

    init(modelo: ModeloHojaIPad, titulo: String, nucleo: Nucleo, fichero: URL?) {
        self.modelo = modelo
        self.titulo = titulo
        self.fichero = fichero
        _tinta = StateObject(wrappedValue: ControladorTintaHoja(url: fichero, nucleo: nucleo))
    }

    var body: some View {
        VStack(spacing: 0) {
            BarraVisorIPad(tipo: "Hoja de cálculo", icono: "tablecells", titulo: titulo, alFijar: {
                if let u = fichero, GestorPinesIPad.compartido.pinear(url: u) { return }
                GestorPinesIPad.compartido.pinearLoQueSeVe()
            }) {
                Button {
                    if modelo.editando { modelo.confirmar(bajar: false) }
                    modelo.editando.toggle()
                    escribiendo = false
                } label: {
                    Label(modelo.editando ? "Ver" : "Editar", systemImage: modelo.editando ? "eye" : "pencil")
                }
                .help(modelo.editando ? "Dejar de editar" : "Editar la hoja")
                Button { modelo.copiar() } label: { Image(systemName: "doc.on.doc") }
                    .keyboardShortcut("c", modifiers: .command)
                    .help("Copiar (⌘C)")
                Button { compartirCopia() } label: { Image(systemName: "square.and.arrow.up") }
                    .disabled(compartiendo)
                    .help("Compartir una copia en Excel")
                    .accessibilityLabel("Compartir una copia en Excel")
            }
            barraDeFormulas
            if modelo.editando { formatos }
            if let aviso = modelo.aviso {
                HStack {
                    Text(aviso).font(.caption).foregroundStyle(.secondary)
                    Spacer()
                    Button { modelo.aviso = nil } label: { Image(systemName: "xmark") }.buttonStyle(.borderless)
                }
                .padding(.horizontal, 12).padding(.vertical, 4)
            }
            Divider()
            RejillaHojaIPad(modelo: modelo, tinta: tinta) {
                if modelo.editando { escribiendo = true }
            }
            .overlay { CapaTintaHojaVista(controlador: tinta).allowsHitTesting(false) }
            .overlay(alignment: .topTrailing) {
                RielHojaIPad(c: tinta, modelo: tinta.modelo)
                    .padding(.top, 34)
                    .padding(.trailing, 12)
            }
            .overlay(alignment: .bottom) {
                TiraDeMarcaDeHoja(tinta: tinta, modelo: tinta.modelo).padding(.bottom, 24)
            }
            if modelo.nombres.count > 1 { pestanas }
        }
        .background(Color(uiColor: .systemBackground).ignoresSafeArea())
        .modifier(NocheDelVisor(activo: noche.activo))
        .onAppear {
            tinta.hoja = modelo
            tinta.cambioDeHoja()
        }
        .onChange(of: modelo.activa) { _, _ in tinta.cambioDeHoja() }
        .onChange(of: modelo.version) { _, _ in tinta.cambioDeAnchos() }
        .onDisappear {
            modelo.cerrar()
            tinta.guardarYa()
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.willResignActiveNotification)) { _ in
            modelo.cerrar()
            tinta.guardarYa()
        }
    }

    /// **Una copia del libro en Excel** con lo editado (los valores ya
    /// calculados, el formato y los anchos), por la hoja del sistema.
    private func compartirCopia() {
        if modelo.editando { modelo.confirmar(bajar: false) }
        compartiendo = true
        let hoja = modelo.hoja
        let nombre = ((fichero?.deletingPathExtension().lastPathComponent).flatMap { $0.isEmpty ? nil : $0 }
            ?? (titulo as NSString).deletingPathExtension)
            .replacingOccurrences(of: "/", with: "-")
        let vista = tinta.capa
        _ = Task<Void, Never> {
            let url = await Task.detached(priority: .userInitiated) { () -> URL? in
                guard let datos = try? hoja.comoXlsx() else { return nil }
                let dir = FileManager.default.temporaryDirectory
                    .appendingPathComponent("hoja-\(UUID().uuidString)", isDirectory: true)
                try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
                let u = dir.appendingPathComponent("\(nombre.isEmpty ? "Hoja" : nombre).xlsx")
                return (try? datos.write(to: u, options: .atomic)) != nil ? u : nil
            }.value
            compartiendo = false
            guard let url else {
                modelo.aviso = "No se pudo preparar la copia en Excel."
                return
            }
            SalidaDocumentoIPad.compartir([url], desde: vista) {
                try? FileManager.default.removeItem(at: url.deletingLastPathComponent())
            }
        }
    }

    private var barraDeFormulas: some View {
        HStack(spacing: 8) {
            Text(modelo.referencia)
                .font(.system(size: 13, weight: .medium).monospaced())
                .frame(minWidth: 56)
                .padding(.vertical, 6).padding(.horizontal, 8)
                .background(Color(uiColor: .secondarySystemBackground), in: RoundedRectangle(cornerRadius: 6))
            Image(systemName: "function").foregroundStyle(.secondary)
            TextField(modelo.editando ? "Valor o =fórmula" : "Pulsa Editar para escribir", text: $modelo.texto)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .focused($escribiendo)
                .disabled(!modelo.editando)
                .onSubmit {
                    modelo.confirmar()
                    escribiendo = true
                }
            if escribiendo {
                Button("Cancelar") {
                    modelo.cancelar()
                    escribiendo = false
                }
                .buttonStyle(.borderless)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
    }

    private var formatos: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 2) {
                boton("arrow.uturn.backward", "Deshacer (⌘Z)", activo: modelo.puedeDeshacer) { modelo.deshacer() }
                    .keyboardShortcut("z", modifiers: .command)
                boton("arrow.uturn.forward", "Rehacer (⇧⌘Z)", activo: modelo.puedeRehacer) { modelo.rehacer() }
                    .keyboardShortcut("z", modifiers: [.command, .shift])
                Divider().frame(height: 22)
                boton("bold", "Negrita (⌘B)") { modelo.formato(.negrita) }
                    .keyboardShortcut("b", modifiers: .command)
                boton("italic", "Cursiva (⌘I)") { modelo.formato(.cursiva) }
                    .keyboardShortcut("i", modifiers: .command)
                boton("underline", "Subrayado (⌘U)") { modelo.formato(.subrayado) }
                    .keyboardShortcut("u", modifiers: .command)
                boton("strikethrough", "Tachado (⌘5)") { modelo.formato(.tachado) }
                    .keyboardShortcut("5", modifiers: .command)
                Divider().frame(height: 22)
                boton("text.alignleft", "Izquierda") { modelo.formato(.alinear(alineacion: "i")) }
                boton("text.aligncenter", "Centro") { modelo.formato(.alinear(alineacion: "c")) }
                boton("text.alignright", "Derecha") { modelo.formato(.alinear(alineacion: "d")) }
                boton("text.word.spacing", "Ajustar texto") { modelo.formato(.ajustar) }
                Divider().frame(height: 22)
                Menu {
                    Button("General") { modelo.formato(.numero(formato: nil)) }
                    Button("Número") { modelo.formato(.numero(formato: "numero")) }
                    Button("Moneda") { modelo.formato(.numero(formato: "moneda")) }
                    Button("Porcentaje") { modelo.formato(.numero(formato: "porcentaje")) }
                    Button("Fecha") { modelo.formato(.numero(formato: "fecha")) }
                } label: {
                    Image(systemName: "number").frame(width: 36, height: 32)
                }
                .help("Formato del número")
                menuDeColores("Resaltar", "highlighter", hojaFondos(), sinColor: "Sin color") {
                    modelo.formato(.fondo(color: $0))
                }
                menuDeColores("Color de letra", "paintbrush", hojaColoresDeLetra(), sinColor: "Automático") {
                    modelo.formato(.letra(color: $0))
                }
                boton("eraser", "Borrar formato") { modelo.formato(.borrar) }
                Divider().frame(height: 22)
                boton("scissors", "Cortar (⌘X)") { modelo.copiar(cortar: true) }
                    .keyboardShortcut("x", modifiers: .command)
                boton("doc.on.clipboard", "Pegar (⌘V)") { modelo.pegar() }
                    .keyboardShortcut("v", modifiers: .command)
                boton("trash", "Vaciar lo elegido") { modelo.borrar() }
                boton("rectangle.dashed", "Elegir un rango: toca la otra esquina", encendido: modelo.extender) {
                    modelo.extender.toggle()
                }
                boton("square.grid.3x3.fill", "Elegir todo") { modelo.elegirTodo() }
                Divider().frame(height: 22)
                boton("arrow.left.and.right", "Columna más ancha") {
                    modelo.cambiarAncho(modelo.columna, modelo.ancho(modelo.columna) + 24)
                }
                boton("arrow.right.and.line.vertical.and.arrow.left", "Columna más estrecha") {
                    modelo.cambiarAncho(modelo.columna, modelo.ancho(modelo.columna) - 24)
                }
            }
            .padding(.horizontal, 10)
        }
        .frame(height: 40)
    }

    private func boton(_ simbolo: String, _ ayuda: String, activo: Bool = true, encendido: Bool = false,
                       _ accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            Image(systemName: simbolo)
                .frame(width: 36, height: 32)
                .foregroundStyle(encendido ? Color.white : Color.primary)
                .background(RoundedRectangle(cornerRadius: 7).fill(encendido ? Paleta.v2.azulLleno : Color.clear))
        }
        .buttonStyle(.borderless)
        .disabled(!activo)
        .help(ayuda)
        .accessibilityLabel(ayuda)
    }

    private func menuDeColores(_ titulo: String, _ simbolo: String, _ colores: [ColorHoja], sinColor: String,
                               _ poner: @escaping (String?) -> Void) -> some View {
        Menu {
            Button(sinColor) { poner(nil) }
            ForEach(colores, id: \.hex) { c in
                Button { poner(c.hex) } label: {
                    Label {
                        Text(c.nombre)
                    } icon: {
                        Image(systemName: "circle.fill")
                            .foregroundStyle(colorDeHojaHex(c.hex) ?? .clear, .primary)
                    }
                }
            }
        } label: {
            Image(systemName: simbolo).frame(width: 36, height: 32)
        }
        .help(titulo)
    }

    private var pestanas: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 2) {
                ForEach(Array(modelo.nombres.enumerated()), id: \.offset) { i, n in
                    Button { modelo.irAHoja(i) } label: {
                        Text(n).font(.callout)
                            .padding(.horizontal, 12).padding(.vertical, 6)
                            .background(RoundedRectangle(cornerRadius: 6)
                                .fill(i == modelo.activa ? Color(uiColor: .secondarySystemBackground) : .clear))
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 8)
        }
        .frame(height: 38)
        .background(Color(uiColor: .systemGroupedBackground))
    }
}

/// **La rejilla**: perezosa a lo alto, con las letras de las columnas
/// arriba (fijas al desplazar) y los numeros de las filas a la izquierda.
struct RejillaHojaIPad: View {
    @ObservedObject var modelo: ModeloHojaIPad
    /// La tinta y los graficos de encima (`TintaHojaIPad.swift`).
    var tinta: ControladorTintaHoja?
    /// Un toque en la celda que ya estaba elegida.
    let alRetocar: () -> Void

    var body: some View {
        ScrollView([.horizontal, .vertical]) {
            LazyVStack(alignment: .leading, spacing: 0, pinnedViews: [.sectionHeaders]) {
                Section {
                    ForEach(0..<modelo.filas, id: \.self) { f in
                        FilaHojaIPad(modelo: modelo, fila: f, version: modelo.version, alRetocar: alRetocar)
                    }
                } header: {
                    cabecera
                }
            }
            .background(alignment: .topLeading) {
                if let tinta { MarcaRejillaHoja(controlador: tinta) }
            }
            .overlay(alignment: .topLeading) {
                if let tinta { GraficosDeLaHoja(tinta: tinta) }
            }
        }
    }

    private var cabecera: some View {
        HStack(spacing: 0) {
            Color(uiColor: .secondarySystemBackground)
                .frame(width: ModeloHojaIPad.anchoNumeros, height: 26)
            ForEach(0..<modelo.columnas, id: \.self) { c in
                Text(letrasDeColumna(c))
                    .font(.system(size: 12, weight: c >= modelo.rango.c0 && c <= modelo.rango.c1 ? .bold : .regular))
                    .foregroundStyle(.secondary)
                    .frame(width: modelo.ancho(c), height: 26)
                    .background(Color(uiColor: .secondarySystemBackground))
                    .overlay(alignment: .trailing) { Rectangle().fill(Color(uiColor: .separator)).frame(width: 0.5) }
            }
        }
    }
}

private struct FilaHojaIPad: View {
    @ObservedObject var modelo: ModeloHojaIPad
    let fila: Int
    /// Solo para que la fila se pinte otra vez cuando cambia la hoja.
    let version: Int
    let alRetocar: () -> Void

    var body: some View {
        HStack(spacing: 0) {
            Text("\(fila + 1)")
                .font(.system(size: 11).monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(width: ModeloHojaIPad.anchoNumeros, height: ModeloHojaIPad.altoFila)
                .background(Color(uiColor: .secondarySystemBackground))
            ForEach(0..<modelo.columnas, id: \.self) { c in
                CeldaHojaIPad(celda: modelo.celda(fila, c), ancho: modelo.ancho(c),
                              activa: fila == modelo.fila && c == modelo.columna,
                              elegida: modelo.dentro(fila, c))
                    .onTapGesture {
                        if modelo.tocar(fila, c) { alRetocar() }
                    }
            }
        }
    }
}

private struct CeldaHojaIPad: View {
    let celda: CeldaHoja?
    let ancho: CGFloat
    let activa: Bool
    let elegida: Bool

    private var alineacion: Alignment {
        switch celda?.estilo?.alineacion {
        case "c": .center
        case "d": .trailing
        case "i": .leading
        default: celda?.numero != nil ? .trailing : .leading
        }
    }

    var body: some View {
        let e = celda?.estilo
        let texto = celda.map { textoDeCelda($0) } ?? ""
        Text(texto)
            .font(.system(size: 13, weight: e?.negrita == true ? .bold : .regular))
            .italic(e?.cursiva == true)
            .underline(e?.subrayado == true)
            .strikethrough(e?.tachado == true)
            .foregroundStyle(celda?.error == true ? Paleta.v2.rojo : (colorDeHojaHex(e?.color) ?? .primary))
            .lineLimit(e?.ajustar == true ? 2 : 1)
            .truncationMode(.tail)
            .padding(.horizontal, 4)
            .frame(width: ancho, height: ModeloHojaIPad.altoFila, alignment: alineacion)
            .background(fondo(e))
            .overlay {
                Rectangle().strokeBorder(Color(uiColor: .separator).opacity(0.6), lineWidth: 0.5)
            }
            .overlay {
                if activa { Rectangle().strokeBorder(Paleta.v2.acento, lineWidth: 2) }
            }
            .contentShape(Rectangle())
    }

    private func fondo(_ e: EstiloCeldaHoja?) -> Color {
        if elegida && !activa { return Paleta.v2.acento.opacity(0.14) }
        return colorDeHojaHex(e?.fondo) ?? .clear
    }
}
