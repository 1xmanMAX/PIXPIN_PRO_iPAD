// **La hoja de compartir comun** del iPad (D1, `compartir/ventana.rs` de
// Windows, `HojaDeCompartir.kt` del movil y `Compartir/HojaCompartir.swift`
// del Mac): una sola para toda la app. El chat (un mensaje, lo elegido), la
// lista de proyectos y quien quiera la abren con lo que comparten
// (`CosaCompartida`), y de aqui en adelante todo va igual.
//
// Tres partes, de arriba abajo, como en Windows:
// - **los formatos**, en redondeles (HTML, PDF, PNG, JPG, SVG y lo que va
//   entero: Original, Editable, .pixpin, Texto);
// - **que paginas**, cuando hay para elegir (ver `EstadoHoja`);
// - **el peso y las salidas**: el fichero de lo elegido se hace en cuanto
//   se elige, para decir cuanto pesa de verdad, y es ese el que se manda.
//
// Las salidas, lo que en el iPad hace las veces de las de Windows:
// **Compartir** (el principal) es la hoja del sistema (AirDrop, Mensajes,
// Mail, WhatsApp, «Guardar en Archivos»...), lo que en Windows es el panel
// Compartir; **Guardar en Archivos…** es el «Guardar como…»; **Copiar** lo
// deja en el portapapeles; **Imprimir…** imprime las paginas marcadas.
//
// Teclado del iPad: ← → cambian de formato, ↩ comparte, ⌘S guarda, ⌘C
// copia, ⌘P imprime, ⎋ cierra.

import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

@MainActor
enum HojaCompartir {
    /// **Abre la hoja** para `cosa`, encima de lo que se este viendo.
    /// `almacen` hace falta para leer los mensajes y los proyectos.
    static func abrir(_ cosa: CosaCompartida, almacen: Almacen?) {
        let modelo = ModeloHojaCompartir()
        let host = ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 580, height: 540)) { cerrar in
            VistaHojaCompartir(modelo: modelo, cerrar: cerrar)
        }
        guard host != nil else { return }
        Task { await modelo.preparar(cosa, almacen: almacen) }
    }

    /// Lo ya preparado (un visor con sus formatos propios), en la hoja.
    static func abrir(_ preparado: PreparadoCompartir) {
        abrir(.preparado(preparado), almacen: nil)
    }

    /// **Imprimir** lo que se comparte: todas sus paginas marcadas, cada una
    /// en su hoja (el «Imprimir…» de Windows, `Accion::Imprimir`).
    static func imprimir(_ cosa: CosaCompartida, almacen: Almacen?) {
        Task {
            let p = await PrepararCompartir.preparar(cosa, almacen: almacen)
            let marcadas = p.compartible().marcadas
            let piezas = p.piezas.filter { marcadas?.contains($0.pagina.clave) ?? true }
            guard let pdf = PreparadoCompartir.pdf(piezas) else { return }
            SalidasDeLaHoja.imprimir(pdf, titulo: p.titulo, alAcabar: { _ in })
        }
    }
}

// MARK: - El modelo

/// Las salidas del pie.
enum SalidaHoja {
    /// La hoja del sistema (AirDrop, Mensajes, Mail...).
    case compartir
    /// «Guardar en Archivos…».
    case guardar
    case copiar
    case imprimir
}

@MainActor
final class ModeloHojaCompartir: ObservableObject {
    @Published private(set) var preparado: PreparadoCompartir?
    @Published private(set) var compartible: Compartible?
    @Published var estado: EstadoHoja?
    /// El fichero de lo elegido, hecho; `nil` mientras se hace.
    @Published private(set) var hecho: [URL]?
    @Published private(set) var fallo: String?
    @Published private(set) var aviso: String?
    /// Una salida pulsada antes de que estuviera hecho: se hace en cuanto
    /// este.
    private var pendiente: SalidaHoja?
    private var turno = 0
    private var turnoAviso = 0
    /// La carpeta de lo hecho; lo de mas de un dia se borra al abrir otra.
    private let carpetaBase = FileManager.default.temporaryDirectory
        .appendingPathComponent("pixpin-compartir", isDirectory: true)
    var cerrar: () -> Void = {}
    /// La vista desde la que sale la hoja del sistema (la pone la vista).
    weak var anclaje: UIView?

    func preparar(_ cosa: CosaCompartida, almacen: Almacen?) async {
        let base = carpetaBase
        await Task.detached(priority: .utility) { ModeloHojaCompartir.limpiarViejos(base) }.value
        usar(await PrepararCompartir.preparar(cosa, almacen: almacen))
    }

    /// Lo ya preparado, en la hoja.
    func usar(_ p: PreparadoCompartir) {
        let c = p.compartible()
        preparado = p
        compartible = c
        guard !c.formatos.isEmpty else {
            fallo = ErrorCompartir.nada.localizedDescription
            return
        }
        var e = EstadoHoja(c)
        if let id = p.formatoInicial, let i = c.formatos.firstIndex(where: { $0.id == id }) {
            e.elegirFormato(c, i)
        }
        estado = e
        rehacer()
    }

    func cambiar(_ hacer: (inout EstadoHoja, Compartible) -> Void) {
        guard var e = estado, let c = compartible else { return }
        hacer(&e, c)
        guard e != estado else { return }
        estado = e
        rehacer()
    }

    func moverFormato(_ paso: Int) { cambiar { e, c in e.moverFormato(c, paso) } }

    /// Si hay paginas que imprimir (lo que va entero no se imprime).
    var sePuedeImprimir: Bool { !(preparado?.piezas.isEmpty ?? true) }

    /// **Hace el fichero de lo elegido**, para decir cuanto pesa: el que se
    /// manda despues es este mismo.
    private func rehacer() {
        guard let p = preparado, let c = compartible, let e = estado, let f = e.formato(c) else { return }
        turno += 1
        let t = turno
        hecho = nil
        fallo = nil
        guard e.hayQuePreparar(c) else {
            fallo = ErrorCompartir.sinPaginas.localizedDescription
            return
        }
        let carpeta = carpetaBase.appendingPathComponent(UUID().uuidString, isDirectory: true)
        Task {
            // Un respiro: que tocar varias paginas seguidas no haga cinco PDF.
            try? await Task.sleep(nanoseconds: 150_000_000)
            guard t == turno else { return }
            do {
                let urls = try await p.generar(f.id, claves: e.elegidas(c), en: carpeta)
                guard t == turno else { return }
                hecho = urls
                if let s = pendiente {
                    pendiente = nil
                    salir(s)
                }
            } catch {
                guard t == turno else { return }
                fallo = error.localizedDescription
            }
        }
    }

    /// «1,2 MB · 3 archivos», «Preparando…», «Marca alguna página».
    var pie: String {
        if let fallo { return fallo }
        guard let hecho else { return "Preparando…" }
        let bytes = hecho.reduce(Int64(0)) { s, u in
            s + Int64((try? u.resourceValues(forKeys: [.fileSizeKey]))?.fileSize ?? 0)
        }
        return TextosCompartir.peso(bytes, ficheros: hecho.count)
    }

    func avisar(_ texto: String) {
        aviso = texto
        turnoAviso += 1
        let t = turnoAviso
        Task {
            try? await Task.sleep(nanoseconds: 3_500_000_000)
            if t == turnoAviso { aviso = nil }
        }
    }

    /// **Una salida pulsada.**
    func salir(_ s: SalidaHoja) {
        if s == .imprimir { return imprimir() }
        if let fallo { return avisar(fallo) }
        guard let hecho else {
            pendiente = s
            return avisar("En cuanto esté preparado")
        }
        switch s {
        case .compartir:
            SalidasDeLaHoja.compartir(hecho, desde: anclaje) { [weak self] bien in
                if bien { self?.cerrar() }
            }
        case .guardar:
            SalidasDeLaHoja.guardar(hecho) { [weak self] guardados in
                guard let self, !guardados.isEmpty else { return }
                avisar(guardados.count == 1 ? "Guardado: \(guardados[0].lastPathComponent)" : "Guardado")
            }
        case .copiar:
            SalidasDeLaHoja.copiar(hecho)
            avisar("Copiado al portapapeles")
        case .imprimir:
            break
        }
    }

    /// Imprime las paginas marcadas (o todas, con un formato que va entero).
    private func imprimir() {
        guard let p = preparado, let c = compartible, let e = estado else { return }
        var claves = e.elegidas(c)
        if claves.isEmpty { claves = c.paginas.map(\.clave).filter { e.marcadas.contains($0) } }
        let piezas = claves.compactMap { p.pieza($0) }
        guard let pdf = PreparadoCompartir.pdf(piezas.isEmpty ? p.piezas : piezas) else {
            return avisar(ErrorCompartir.sinPaginas.localizedDescription)
        }
        SalidasDeLaHoja.imprimir(pdf, titulo: p.titulo) { [weak self] bien in
            if bien { self?.avisar("Enviado a la impresora") }
        }
    }

    func limpiar() { turno += 1 }

    /// Borra lo compartido de hace mas de un dia (`limpiar_viejos`).
    nonisolated static func limpiarViejos(_ base: URL, edad: TimeInterval = 24 * 3600) {
        let fm = FileManager.default
        guard let hijas = try? fm.contentsOfDirectory(at: base, includingPropertiesForKeys: [.contentModificationDateKey])
        else { return }
        for h in hijas {
            let fecha = (try? h.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate ?? .distantPast
            if Date().timeIntervalSince(fecha) > edad { try? fm.removeItem(at: h) }
        }
    }
}

// MARK: - Las salidas de iOS

/// Lo que hace cada salida con los ficheros hechos, con las hojas de iOS.
@MainActor
enum SalidasDeLaHoja {
    /// Quien atiende el selector de «Guardar en Archivos» mientras se ve.
    private static var delegado: DelegadoGuardar?

    /// La hoja del sistema. `alAcabar(true)` si se mando a algun sitio.
    static func compartir(_ urls: [URL], desde anclaje: UIView?, alAcabar: @escaping (Bool) -> Void) {
        guard let arriba = ArribaUIKit.controladorDeArriba() else { return }
        let hoja = UIActivityViewController(activityItems: urls, applicationActivities: nil)
        hoja.completionWithItemsHandler = { _, hecho, _, _ in alAcabar(hecho) }
        if let pop = hoja.popoverPresentationController {
            if let anclaje {
                pop.sourceView = anclaje
                pop.sourceRect = anclaje.bounds
            } else {
                pop.sourceView = arriba.view
                pop.sourceRect = CGRect(x: arriba.view.bounds.midX, y: arriba.view.bounds.maxY - 40, width: 1, height: 1)
            }
        }
        arriba.present(hoja, animated: true)
    }

    /// «Guardar en Archivos…»: el selector de carpetas de iOS, con copia.
    static func guardar(_ urls: [URL], alAcabar: @escaping ([URL]) -> Void) {
        guard let arriba = ArribaUIKit.controladorDeArriba() else { return }
        let selector = UIDocumentPickerViewController(forExporting: urls, asCopy: true)
        let d = DelegadoGuardar { guardados in
            alAcabar(guardados)
            SalidasDeLaHoja.delegado = nil
        }
        delegado = d
        selector.delegate = d
        arriba.present(selector, animated: true)
    }

    /// **Al portapapeles**: cada fichero con su tipo (una foto se pega como
    /// foto; un texto, como texto).
    static func copiar(_ urls: [URL]) {
        var cosas: [[String: Any]] = []
        for u in urls {
            guard let datos = try? Data(contentsOf: u) else { continue }
            let tipo = UTType(filenameExtension: u.pathExtension) ?? .data
            var cosa: [String: Any] = [tipo.identifier: datos]
            if tipo.conforms(to: .plainText), let texto = String(data: datos, encoding: .utf8) {
                cosa[UTType.utf8PlainText.identifier] = texto
            }
            cosas.append(cosa)
        }
        guard !cosas.isEmpty else { return }
        UIPasteboard.general.setItems(cosas)
    }

    /// El dialogo de imprimir de iOS con un PDF. `alAcabar(true)` si se
    /// mando a imprimir.
    static func imprimir(_ pdf: Data, titulo: String, alAcabar: @escaping (Bool) -> Void) {
        let impresora = UIPrintInteractionController.shared
        let info = UIPrintInfo.printInfo()
        info.jobName = titulo
        info.outputType = .general
        impresora.printInfo = info
        impresora.printingItem = pdf
        impresora.present(animated: true) { _, hecho, _ in alAcabar(hecho) }
    }

    private final class DelegadoGuardar: NSObject, UIDocumentPickerDelegate {
        let alAcabar: ([URL]) -> Void
        init(_ alAcabar: @escaping ([URL]) -> Void) { self.alAcabar = alAcabar }

        func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
            alAcabar(urls)
        }

        func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
            alAcabar([])
        }
    }
}

// MARK: - La vista

private enum ColoresHoja {
    static let fondo = Color(red: 0x1c / 255, green: 0x1d / 255, blue: 0x22 / 255)
    static let redondel = Color(red: 0x2a / 255, green: 0x2b / 255, blue: 0x31 / 255)
    static let encima = Color(red: 0x34 / 255, green: 0x35 / 255, blue: 0x3c / 255)
    static let texto = Color(red: 0xe8 / 255, green: 0xe8 / 255, blue: 0xec / 255)
    static let apagado = Color(red: 0x9a / 255, green: 0x9a / 255, blue: 0xa2 / 255)
    static let encendido = Color(red: 0x1e / 255, green: 0x88 / 255, blue: 0xe5 / 255)
}

struct VistaHojaCompartir: View {
    @ObservedObject var modelo: ModeloHojaCompartir
    let cerrar: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text(modelo.compartible?.titulo ?? "Compartir")
                    .font(.system(size: 17, weight: .semibold))
                    .foregroundStyle(ColoresHoja.texto)
                    .lineLimit(1)
                Spacer()
                Button(action: cerrar) {
                    Image(systemName: "xmark")
                        .font(.system(size: 14, weight: .semibold))
                        .frame(width: 34, height: 34)
                        .background(ColoresHoja.redondel, in: Circle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(ColoresHoja.texto)
                .accessibilityLabel("Cerrar (⎋)")
            }
            if let c = modelo.compartible, let e = modelo.estado {
                formatos(c, e)
                paginas(c, e)
            } else if let fallo = modelo.fallo {
                Text(fallo).foregroundStyle(ColoresHoja.apagado)
            } else {
                HStack(spacing: 8) { ProgressView(); Text("Preparando…") }
                    .foregroundStyle(ColoresHoja.apagado)
            }
            Spacer(minLength: 0)
            pieDeHoja
        }
        .padding(20)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(ColoresHoja.fondo.ignoresSafeArea())
        .environment(\.colorScheme, .dark)
        .background { atajos }
        .onAppear { modelo.cerrar = cerrar }
        .onDisappear { modelo.limpiar() }
    }

    private func formatos(_ c: Compartible, _ e: EstadoHoja) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                ForEach(e.visibles(c), id: \.self) { i in
                    let f = c.formatos[i]
                    Button { modelo.cambiar { e, c in e.elegirFormato(c, i) } } label: {
                        VStack(spacing: 6) {
                            Image(systemName: f.simbolo)
                                .font(.system(size: 22))
                                .frame(width: 60, height: 60)
                                .background(i == e.formato ? ColoresHoja.encendido : ColoresHoja.redondel, in: Circle())
                            Text(f.nombre).font(.system(size: 12))
                                .lineLimit(1)
                        }
                        .foregroundStyle(ColoresHoja.texto)
                        .frame(width: 82)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .hoverEffect(.highlight)
                    .accessibilityAddTraits(i == e.formato ? .isSelected : [])
                }
            }
        }
    }

    @ViewBuilder
    private func paginas(_ c: Compartible, _ e: EstadoHoja) -> some View {
        if let f = e.formato(c), !c.paginas.isEmpty {
            if f.cuantas == .ninguna {
                Text("Va entero, tal cual").font(.system(size: 13)).foregroundStyle(ColoresHoja.apagado)
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text(f.cuantas == .una ? "Qué página"
                            : TextosCompartir.queIncluir(e.elegidas(c).count, de: c.paginas.count))
                            .font(.system(size: 13, weight: .medium)).foregroundStyle(ColoresHoja.apagado)
                        Spacer()
                        if f.cuantas == .varias, c.paginas.count > 1 {
                            Button("Todas") { modelo.cambiar { e, c in e.todas(c) } }
                            Button("Ninguna") { modelo.cambiar { e, _ in e.ninguna() } }
                        }
                    }
                    .buttonStyle(.borderless)
                    .tint(ColoresHoja.encendido)
                    ScrollView {
                        VStack(spacing: 2) {
                            ForEach(c.paginas) { p in
                                fila(p, marcada: e.marcadas.contains(p.clave), una: f.cuantas == .una)
                            }
                        }
                    }
                    .frame(maxHeight: min(CGFloat(c.paginas.count) * 46, 6 * 46))
                }
            }
        }
    }

    private func fila(_ p: PaginaCompartir, marcada: Bool, una: Bool) -> some View {
        Button { modelo.cambiar { e, c in e.tocarPagina(c, p.clave) } } label: {
            HStack(spacing: 10) {
                Image(systemName: una ? (marcada ? "largecircle.fill.circle" : "circle")
                    : (marcada ? "checkmark.square.fill" : "square"))
                    .font(.system(size: 18))
                    .foregroundStyle(marcada ? ColoresHoja.encendido : ColoresHoja.apagado)
                VStack(alignment: .leading, spacing: 1) {
                    Text(p.nombre).font(.system(size: 14)).foregroundStyle(ColoresHoja.texto).lineLimit(1)
                    if !p.detalle.isEmpty {
                        Text(p.detalle).font(.system(size: 11)).foregroundStyle(ColoresHoja.apagado).lineLimit(1)
                    }
                }
                Spacer()
            }
            .padding(.leading, CGFloat(p.nivel) * 22)
            .padding(.horizontal, 8)
            .frame(height: 44)
            .background(marcada ? ColoresHoja.encima : Color.clear, in: RoundedRectangle(cornerRadius: 8))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var pieDeHoja: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(modelo.aviso ?? modelo.pie)
                .font(.system(size: 13)).foregroundStyle(ColoresHoja.apagado)
                .lineLimit(1)
            HStack(spacing: 8) {
                Button { modelo.salir(.copiar) } label: { Label("Copiar", systemImage: "doc.on.doc") }
                    .accessibilityHint("⌘C")
                Button { modelo.salir(.guardar) } label: { Label("Guardar en Archivos…", systemImage: "folder") }
                    .accessibilityHint("⌘S")
                if modelo.sePuedeImprimir {
                    Button { modelo.salir(.imprimir) } label: { Label("Imprimir…", systemImage: "printer") }
                        .accessibilityHint("⌘P")
                }
                Spacer(minLength: 0)
                Button { modelo.salir(.compartir) } label: {
                    Label("Compartir", systemImage: "square.and.arrow.up")
                        .font(.system(size: 15, weight: .semibold))
                }
                .buttonStyle(.borderedProminent)
                .tint(ColoresHoja.encendido)
                .background(AnclajeHoja { modelo.anclaje = $0 })
                .accessibilityHint("AirDrop, Mensajes, Mail… (↩)")
            }
            .buttonStyle(.bordered)
            .controlSize(.large)
            .labelStyle(.titleAndIcon)
            .lineLimit(1)
        }
    }

    /// Las teclas del teclado del iPad (botones escondidos).
    private var atajos: some View {
        Group {
            Button("Formato anterior") { modelo.moverFormato(-1) }
                .keyboardShortcut(.leftArrow, modifiers: [])
            Button("Formato siguiente") { modelo.moverFormato(1) }
                .keyboardShortcut(.rightArrow, modifiers: [])
            Button("Compartir") { modelo.salir(.compartir) }
                .keyboardShortcut(.defaultAction)
            Button("Guardar") { modelo.salir(.guardar) }
                .keyboardShortcut("s", modifiers: .command)
            Button("Copiar") { modelo.salir(.copiar) }
                .keyboardShortcut("c", modifiers: .command)
            Button("Imprimir") { modelo.salir(.imprimir) }
                .keyboardShortcut("p", modifiers: .command)
            Button("Cerrar", action: cerrar)
                .keyboardShortcut(.cancelAction)
        }
        .opacity(0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// La `UIView` de un sitio de SwiftUI (para colgar de ella la hoja del
/// sistema en el iPad, que sale en globo).
private struct AnclajeHoja: UIViewRepresentable {
    let poner: (UIView) -> Void

    func makeUIView(context: Context) -> UIView {
        let v = UIView()
        v.isUserInteractionEnabled = false
        v.backgroundColor = .clear
        DispatchQueue.main.async { poner(v) }
        return v
    }

    func updateUIView(_ v: UIView, context: Context) {}
}
