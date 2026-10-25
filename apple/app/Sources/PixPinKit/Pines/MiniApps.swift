// Los pines de herramienta: una nota en Markdown y las mini-apps del movil
// (tareas, contador, ruleta, gastos; los tres relojes en `RelojesPin.swift`),
// sueltas en la capa de pines (lo que en Windows es
// `pixpin/src/pines/herramienta.rs`). Copiado del Mac (`Herramientas.swift`)
// con lo de AppKit cambiado por UIKit: el pitido de error es una vibracion
// (`avisarFalloPin`) y la vista la monta `HerramientaPin.vista` en SwiftUI.
//
// **No hay formato nuevo.** El documento es el mismo texto que viaja por la
// sincronizacion (`pixpin_proyecto::mini`) y cada toque lo reescribe el
// nucleo de Rust (`miniTareasAlternar`, `miniContadorCambiar`...): Swift solo
// ensena y guarda el texto que le devuelven. Como el Mac no tiene el almacen
// de pines de Windows, una mini-app vive en un mensaje `MINIAPP` del chat
// (de fabrica, «Mensajes guardados»): asi no se pierde al cerrar y el movil
// la ve, como la lista «sacada a la pantalla» de Windows.
//
// Rendimiento: cada guardado va fuera del hilo principal y en orden (uno
// detras de otro); nada corre mientras no se toca.

import PixPinCore
import SwiftUI
import UIKit

/// El pitido del Mac en el iPad: una vibracion de error (y nada en un iPad
/// sin motor de vibracion).
@MainActor
func avisarFalloPin() {
    UINotificationFeedbackGenerator().notificationOccurred(.error)
}

/// La moneda del aparato para los gastos nuevos (el documento guarda la
/// suya, y esa manda al leer).
func monedaDelAparato() -> String {
    Locale.current.currency?.identifier ?? "EUR"
}

/// Una mini-app abierta: su documento y, si lo tiene, su mensaje del chat.
@MainActor
final class DocumentoMini: ObservableObject {
    let miniapp: String
    @Published private(set) var documento: String
    let moneda = monedaDelAparato()
    private let nucleo: Nucleo?
    let chat: String?
    let mensaje: String?
    /// Lo que hay en el chat (o se esta escribiendo): el `anterior` del
    /// siguiente guardado.
    private var guardado: String
    private var cola: Task<Void, Never>?
    /// Avisa de que cambio (para el panel de pines abiertos).
    var alCambiar: (() -> Void)?
    /// El identificador de su aviso (temporizador y alarma): el de Windows y
    /// el movil (`mini:<id del mensaje>`), o uno suelto si vive en memoria.
    let idAviso: String

    init(miniapp: String, documento: String, nucleo: Nucleo?, chat: String?, mensaje: String?) {
        self.miniapp = miniapp
        self.documento = documento
        self.nucleo = nucleo
        self.chat = chat
        self.mensaje = mensaje
        guardado = documento
        idAviso = mensaje.map { AvisosMini.prefijo + $0 } ?? "local:\(UUID().uuidString)"
    }

    var titulo: String { miniTitulo(documento: documento) }

    /// El nombre de la mini-app para ensenarlo.
    static func nombre(_ miniapp: String) -> String {
        switch miniapp {
        case "tareas": "Tareas"
        case "gastos": "Gastos"
        case "contador": "Contador"
        case "ruleta": "Ruleta"
        case "cronometro": "Cronómetro"
        case "temporizador": "Temporizador"
        case "alarma": "Alarma"
        default: miniapp
        }
    }

    var resumen: String {
        miniResumen(miniapp: miniapp, documento: documento, moneda: moneda)?.texto ?? ""
    }

    /// Aplica un cambio (documento entra, documento sale) y lo guarda.
    /// `false` si el cambio no valia (un importe que no se entiende).
    @discardableResult
    func cambiar(_ f: (String) throws -> String) -> Bool {
        guard let nuevo = try? f(documento) else {
            avisarFalloPin()
            return false
        }
        guard nuevo != documento else { return true }
        documento = nuevo
        alCambiar?()
        AvisosMini.compartido.avisar(de: self)
        guardar(nuevo)
        return true
    }

    /// Escribe en el chat en orden. Si el mensaje cambio por otro lado (el
    /// movil), no se pisa: se relee y se ensena lo de alli.
    private func guardar(_ nuevo: String) {
        guard let nucleo, let chat, let mensaje else { return }
        let anterior = guardado
        guardado = nuevo
        let previa = cola
        cola = Task { [weak self] in
            await previa?.value
            let resultado = await Task.detached(priority: .utility) { () -> (Bool, String?) in
                do {
                    let ok = try nucleo.guardarMiniapp(chat: chat, mensaje: mensaje, anterior: anterior, documento: nuevo)
                    if ok { return (true, nil) }
                    return (false, try? nucleo.documentoMiniapp(chat: chat, mensaje: mensaje))
                } catch {
                    return (false, nil)
                }
            }.value
            guard let self, !resultado.0 else { return }
            avisarFalloPin()
            if let actual = resultado.1 {
                self.guardado = actual
                self.documento = actual
                self.alCambiar?()
                AvisosMini.compartido.avisar(de: self)
            }
        }
    }
}

/// Una nota suelta en Markdown.
@MainActor
final class ModeloNota: ObservableObject {
    @Published var texto: String { didSet { alCambiar?() } }
    @Published var editando: Bool
    var alCambiar: (() -> Void)?

    init(texto: String) {
        self.texto = texto
        editando = texto.isEmpty
    }

    /// La primera linea, sin las marcas de titulo: el nombre en el panel.
    var nombre: String {
        let primera = texto.split(separator: "\n", omittingEmptySubsequences: true).first.map(String.init) ?? ""
        let limpia = primera.drop { $0 == "#" || $0 == " " }
        return String(limpia.prefix(60))
    }
}

/// Lo que va dentro de un pin de herramienta.
@MainActor
final class HerramientaPin {
    enum Clase {
        case nota(ModeloNota)
        case mini(DocumentoMini)
    }

    let clase: Clase

    init(_ clase: Clase) {
        self.clase = clase
    }

    /// Avisar de los cambios (nombre y resumen del panel).
    var alCambiar: (() -> Void)? {
        didSet {
            switch clase {
            case let .nota(m): m.alCambiar = alCambiar
            case let .mini(d): d.alCambiar = alCambiar
            }
        }
    }

    /// Lo de dentro del pin (sin la barra de agarrar, que la pone el pin).
    func vista() -> AnyView {
        let raiz: AnyView
        switch clase {
        case let .nota(m): raiz = AnyView(VistaNotaPin(modelo: m))
        case let .mini(d):
            switch d.miniapp {
            case "tareas": raiz = AnyView(VistaTareasPin(doc: d))
            case "contador": raiz = AnyView(VistaContadorPin(doc: d))
            case "ruleta": raiz = AnyView(VistaRuletaPin(doc: d))
            case "gastos": raiz = AnyView(VistaGastosPin(doc: d))
            case "cronometro": raiz = AnyView(VistaCronometroPin(doc: d))
            case "temporizador": raiz = AnyView(VistaTemporizadorPin(doc: d))
            case "alarma": raiz = AnyView(VistaAlarmaPin(doc: d))
            default: raiz = AnyView(VistaTextoMini(doc: d))
            }
        }
        return raiz
    }

    var nombre: String {
        switch clase {
        case let .nota(m): m.nombre.isEmpty ? "Nota" : m.nombre
        case let .mini(d): d.titulo.isEmpty ? DocumentoMini.nombre(d.miniapp) : d.titulo
        }
    }

    var detalle: String {
        switch clase {
        case .nota: "Nota"
        case let .mini(d):
            d.resumen.isEmpty ? DocumentoMini.nombre(d.miniapp) : "\(DocumentoMini.nombre(d.miniapp)) · \(d.resumen)"
        }
    }

    /// Lo que se copia: el texto tal cual (se entiende pegado en cualquier
    /// sitio, que es la gracia del formato).
    var texto: String {
        switch clase {
        case let .nota(m): m.texto
        case let .mini(d): d.documento
        }
    }
}

// MARK: - Vistas

/// La nota: se lee con su Markdown y se edita con un clic en el lapiz.
struct VistaNotaPin: View {
    @ObservedObject var modelo: ModeloNota

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Spacer()
                Button {
                    modelo.editando.toggle()
                } label: {
                    Image(systemName: modelo.editando ? "eye" : "pencil")
                }
                .buttonStyle(.borderless)
                .help(modelo.editando ? "Ver" : "Editar")
            }
            if modelo.editando {
                TextEditor(text: $modelo.texto)
                    .font(.system(size: 15))
                    .scrollContentBackground(.hidden)
            } else {
                ScrollView {
                    Text(VistaNotaPin.markdown(modelo.texto))
                        .font(.system(size: 15))
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .padding(10)
        .background(Color(uiColor: .systemBackground))
    }

    static func markdown(_ texto: String) -> AttributedString {
        let opciones = AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        return (try? AttributedString(markdown: texto, options: opciones)) ?? AttributedString(texto)
    }
}

/// La cabecera comun: el titulo de la mini-app.
struct CabeceraMini: View {
    @ObservedObject var doc: DocumentoMini
    var body: some View {
        Text(doc.titulo.isEmpty ? DocumentoMini.nombre(doc.miniapp) : doc.titulo)
            .font(.headline)
            .lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct VistaTareasPin: View {
    @ObservedObject var doc: DocumentoMini
    @State private var nueva = ""

    private var desfase: Int64 { Int64(TimeZone.current.secondsFromGMT()) * 1000 }

    var body: some View {
        let tareas = miniTareasLeer(documento: doc.documento, desfaseMs: desfase)
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                CabeceraMini(doc: doc)
                Text("\(tareas.filter(\.hecha).count) de \(tareas.count)")
                    .font(.caption).foregroundStyle(.secondary)
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(tareas, id: \.indice) { t in
                        HStack(alignment: .firstTextBaseline) {
                            Button {
                                doc.cambiar { miniTareasAlternar(documento: $0, indice: t.indice) }
                            } label: {
                                Image(systemName: t.hecha ? "checkmark.square.fill" : "square")
                            }
                            .buttonStyle(.borderless)
                            Text(t.texto)
                                .strikethrough(t.hecha)
                                .foregroundStyle(t.hecha ? .secondary : .primary)
                            Spacer(minLength: 4)
                            if let dias = t.dias, !t.hecha, dias > 0 {
                                Text("\(dias) d").font(.caption2).foregroundStyle(.secondary)
                            }
                            Button {
                                doc.cambiar { miniTareasQuitar(documento: $0, indice: t.indice) }
                            } label: {
                                Image(systemName: "xmark").font(.caption2)
                            }
                            .buttonStyle(.borderless)
                            .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            TextField("Añadir tarea", text: $nueva)
                .textFieldStyle(.roundedBorder)
                .onSubmit {
                    let texto = nueva
                    nueva = ""
                    doc.cambiar { miniTareasAnadir(documento: $0, texto: texto, desfaseMs: desfase) }
                }
        }
        .padding(10)
    }
}

struct VistaContadorPin: View {
    @ObservedObject var doc: DocumentoMini

    var body: some View {
        let cuenta = miniContadorLeer(documento: doc.documento)
        VStack(spacing: 8) {
            CabeceraMini(doc: doc)
            Spacer(minLength: 0)
            Text("\(cuenta.valor)")
                .font(.system(size: 48, weight: .semibold, design: .rounded))
                .monospacedDigit()
                .minimumScaleFactor(0.3)
                .lineLimit(1)
            HStack(spacing: 16) {
                Button {
                    doc.cambiar { miniContadorCambiar(documento: $0, accion: .menos) }
                } label: {
                    Image(systemName: "minus").frame(width: 44, height: 32)
                }
                Button {
                    doc.cambiar { miniContadorCambiar(documento: $0, accion: .mas) }
                } label: {
                    Image(systemName: "plus").frame(width: 64, height: 32)
                }
                .keyboardShortcut(.space, modifiers: [])
                .buttonStyle(.borderedProminent)
            }
            Spacer(minLength: 0)
            HStack {
                Stepper("Paso: \(cuenta.paso)", onIncrement: {
                    doc.cambiar { miniContadorCambiar(documento: $0, accion: .paso(paso: cuenta.paso + 1)) }
                }, onDecrement: {
                    doc.cambiar { miniContadorCambiar(documento: $0, accion: .paso(paso: max(1, cuenta.paso - 1))) }
                })
                .font(.caption)
                Spacer()
                Button("Reiniciar") {
                    doc.cambiar { miniContadorCambiar(documento: $0, accion: .reiniciar) }
                }
                .font(.caption)
            }
        }
        .padding(10)
    }
}

struct VistaRuletaPin: View {
    @ObservedObject var doc: DocumentoMini
    @State private var giro: Double = 0
    @State private var elegido: String?
    @State private var nuevo = ""

    private static let colores: [Color] = [.red, .orange, .yellow, .green, .teal, .blue, .purple, .pink]

    var body: some View {
        let nombres = miniRuletaLeer(documento: doc.documento)
        VStack(spacing: 6) {
            CabeceraMini(doc: doc)
            ZStack(alignment: .top) {
                Canvas { ctx, tam in
                    let lado = min(tam.width, tam.height)
                    let centro = CGPoint(x: tam.width / 2, y: tam.height / 2)
                    let n = max(nombres.count, 1)
                    let porcion = 360.0 / Double(n)
                    for (i, nombre) in nombres.enumerated() {
                        // La porcion 0 empieza arriba y va en el sentido del reloj.
                        let a0 = Angle.degrees(Double(i) * porcion - 90)
                        let a1 = Angle.degrees(Double(i + 1) * porcion - 90)
                        var p = Path()
                        p.move(to: centro)
                        p.addArc(center: centro, radius: lado / 2, startAngle: a0, endAngle: a1, clockwise: false)
                        p.closeSubpath()
                        ctx.fill(p, with: .color(VistaRuletaPin.colores[i % VistaRuletaPin.colores.count].opacity(0.8)))
                        let medio = Angle.degrees((Double(i) + 0.5) * porcion - 90)
                        let r = lado * 0.32
                        let a: Double = medio.radians
                        let punto = CGPoint(x: centro.x + r * CGFloat(Darwin.cos(a)), y: centro.y + r * CGFloat(Darwin.sin(a)))
                        ctx.draw(Text(nombre).font(.caption2).bold(), at: punto)
                    }
                }
                .rotationEffect(.degrees(giro))
                Image(systemName: "arrowtriangle.down.fill").foregroundStyle(.primary)
            }
            .aspectRatio(1, contentMode: .fit)
            if let elegido {
                Text(elegido).font(.title3.bold())
            }
            HStack {
                TextField("Añadir nombre", text: $nuevo)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit {
                        let texto = nuevo
                        nuevo = ""
                        doc.cambiar { miniRuletaAnadir(documento: $0, texto: texto) }
                    }
                Button("Girar") { girar() }
                    .disabled(nombres.count < 2)
                    .buttonStyle(.borderedProminent)
            }
            if !nombres.isEmpty {
                Menu("Quitar") {
                    ForEach(Array(nombres.enumerated()), id: \.offset) { i, n in
                        Button(n) { doc.cambiar { miniRuletaQuitar(documento: $0, indice: UInt32(i)) } }
                    }
                }
                .font(.caption)
            }
        }
        .padding(10)
    }

    private func girar() {
        guard let g = miniRuletaGirar(documento: doc.documento, azar: Double.random(in: 0..<1)) else { return }
        elegido = nil
        // El angulo es de la rueda; se suma a las vueltas ya dadas para que
        // siempre gire hacia delante.
        let base = (giro / 360).rounded(.down) * 360 + 360
        withAnimation(.easeOut(duration: 3)) {
            giro = base + Double(g.angulo)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { elegido = g.nombre }
    }
}

struct VistaGastosPin: View {
    @ObservedObject var doc: DocumentoMini
    @State private var concepto = ""
    @State private var importe = ""

    var body: some View {
        let libro = miniGastosLeer(documento: doc.documento, moneda: doc.moneda)
        VStack(alignment: .leading, spacing: 6) {
            CabeceraMini(doc: doc)
            ScrollView {
                VStack(spacing: 3) {
                    ForEach(Array(libro.gastos.enumerated()), id: \.offset) { i, g in
                        HStack {
                            Text(g.concepto.isEmpty ? "—" : g.concepto).lineLimit(1)
                            Spacer()
                            Text(g.importe).monospacedDigit()
                            Button {
                                doc.cambiar { miniGastosBorrar(documento: $0, moneda: doc.moneda, indice: UInt32(i)) }
                            } label: {
                                Image(systemName: "xmark").font(.caption2)
                            }
                            .buttonStyle(.borderless)
                            .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            Divider()
            HStack {
                Text("Total").bold()
                Spacer()
                Text(libro.totalTexto).bold().monospacedDigit()
            }
            HStack {
                TextField("Concepto", text: $concepto).textFieldStyle(.roundedBorder)
                TextField(libro.simbolo, text: $importe)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 80)
                    .onSubmit { anadir() }
            }
        }
        .padding(10)
    }

    private func anadir() {
        let c = concepto, i = importe
        guard doc.cambiar({ try miniGastosAnadir(documento: $0, moneda: doc.moneda, concepto: c, importe: i) })
        else { return }
        concepto = ""
        importe = ""
    }
}

/// Una mini-app que este pin aun no sabe pintar: su texto, que se entiende.
struct VistaTextoMini: View {
    @ObservedObject var doc: DocumentoMini
    var body: some View {
        ScrollView {
            Text(doc.documento).font(.system(size: 12, design: .monospaced)).textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(10)
    }
}
