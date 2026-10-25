// La seccion «Asistente» del iPad, como el chat «Asistente» de Windows y la
// ventana del asistente del Mac: las conversaciones a la izquierda (se
// vuelve a una y se sigue), las burbujas con lo que penso (plegable), lo
// que hizo y lo que dibujo (abrirlo en el lienzo o guardarlo en «Mensajes
// guardados»), la pregunta de «¿seguro?» con «Sí» y «No», el altavoz de
// cada respuesta, y la caja con el clip (Archivos), las fotos, el
// microfono y enviar o «Parar». Arriba, quien contesta (Claude, DeepSeek o
// Apple Intelligence) y las claves; si no hay nadie, se dice y se ofrece
// poner la clave.
//
// La firma publica (`VistaAsistenteIPad(almacen:)`) la usa `Raiz/RaizIPad`.

import PhotosUI
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

public struct VistaAsistenteIPad: View {
    let almacen: Almacen
    @StateObject private var modelo: ModeloAsistente
    @StateObject private var dictado = DictadoAsistente()
    @State private var texto = ""
    @State private var guardado: UUID?
    @State private var lateral = true
    @State private var encima = false
    @State private var poniendoClave = false
    @State private var viendoAutomatizaciones = false
    @State private var eligiendoFicheros = false
    @State private var fotos: [PhotosPickerItem] = []
    @FocusState private var escribiendo: Bool
    @Environment(\.horizontalSizeClass) private var ancho

    public init(almacen: Almacen) {
        self.almacen = almacen
        _modelo = StateObject(wrappedValue: ModeloAsistente(almacen: almacen, historial: HistorialAsistente()))
        AsistenteDelIPad.activar()
    }

    public var body: some View {
        HStack(spacing: 0) {
            if lateral, ancho != .compact {
                ListaConversaciones(modelo: modelo)
                    .frame(width: 260)
                Divider()
            }
            VStack(spacing: 0) {
                cabecera
                Divider()
                conversacion
                Divider()
                caja
            }
        }
        .task {
            await modelo.comprobarMotor()
            modelo.prepararDocumentos()
        }
        .onDrop(of: [.fileURL, .image, .pdf], isTargeted: $encima) { proveedores in
            soltar(proveedores)
            return true
        }
        .overlay {
            if encima {
                RoundedRectangle(cornerRadius: 12).stroke(Color.accentColor, lineWidth: 3).padding(4)
                    .allowsHitTesting(false)
            }
        }
        .sheet(isPresented: $poniendoClave, onDismiss: { Task { await modelo.comprobarMotor() } }) {
            HojaClaves(almacen: almacen)
        }
        .sheet(isPresented: $viendoAutomatizaciones) {
            NavigationStack {
                VistaAutomatizaciones(almacen: almacen)
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button("Cerrar") { viendoAutomatizaciones = false }
                        }
                    }
            }
        }
        .fileImporter(isPresented: $eligiendoFicheros, allowedContentTypes: [.pdf, .image, .plainText, .item],
                      allowsMultipleSelection: true) { r in
            if case let .success(urls) = r { modelo.adjuntar(urls) }
        }
        .onChange(of: fotos) { _, elegidas in
            guard !elegidas.isEmpty else { return }
            fotos = []
            for f in elegidas {
                Task {
                    if let datos = try? await f.loadTransferable(type: Data.self) {
                        modelo.adjuntar(imagen: datos)
                    }
                }
            }
        }
    }

    // MARK: Cabecera

    private var cabecera: some View {
        HStack(spacing: 12) {
            if ancho != .compact {
                Button { withAnimation { lateral.toggle() } } label: { Image(systemName: "sidebar.left") }
                    .help(lateral ? "Ocultar las conversaciones" : "Ver las conversaciones")
            }
            Menu {
                Button { elegir("claude") } label: { etiqueta("Claude Haiku (nube)", "claude") }
                Button { elegir("deepseek") } label: { etiqueta("DeepSeek (nube)", "deepseek") }
                Button { elegir("") } label: { etiqueta("Apple Intelligence (en el iPad)", "") }
                Divider()
                Button { poniendoClave = true } label: { Label("Claves de la nube…", systemImage: "key") }
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: icono)
                        .foregroundStyle(modelo.motor?.faltaClave == true ? Color.orange : Color.accentColor)
                    Text(modelo.motor?.nombre ?? "Buscando motor…")
                        .font(.subheadline)
                        .lineLimit(1)
                    Image(systemName: "chevron.down").font(.caption2)
                }
                .padding(.horizontal, 10).padding(.vertical, 5)
                .background(Color.accentColor.opacity(0.12), in: Capsule())
            }
            .help("Quién contesta (el mismo ajuste que Ajustes → Asistente)")
            Spacer()
            Button { viendoAutomatizaciones = true } label: { Image(systemName: "gearshape.2") }
                .help("Automatizaciones")
            Button { poniendoClave = true } label: { Image(systemName: "key") }
                .help("Claves de Claude y DeepSeek")
            Button {
                modelo.nuevaConversacion()
            } label: {
                Label("Nueva", systemImage: "square.and.pencil")
            }
            .disabled(modelo.burbujas.isEmpty || modelo.ocupado)
            .keyboardShortcut("n", modifiers: [.command, .shift])
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
    }

    @ViewBuilder
    private func etiqueta(_ nombre: String, _ id: String) -> some View {
        if modelo.motorElegido == id { Label(nombre, systemImage: "checkmark") } else { Text(nombre) }
    }

    private func elegir(_ id: String) {
        Task {
            let m = await modelo.elegirMotor(id)
            if case let .nube(_, conClave) = m, !conClave { poniendoClave = true }
        }
    }

    private var icono: String {
        switch modelo.motor {
        case .ninguno: return "exclamationmark.triangle"
        case .nube: return "cloud"
        case .apple: return "apple.logo"
        default: return "sparkles"
        }
    }

    // MARK: Burbujas

    private var conversacion: some View {
        ScrollViewReader { lector in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    if modelo.burbujas.isEmpty {
                        vacia
                    }
                    ForEach(modelo.burbujas) { b in
                        BurbujaAsistente(
                            burbuja: b,
                            guardada: guardado == b.id,
                            pregunta: b.id == modelo.burbujas.last?.id ? modelo.pregunta : nil,
                            alGuardar: { abrir in
                                Task { if await modelo.guardarDibujo(b, abrir: abrir) { guardado = b.id } }
                            },
                            alLeer: { modelo.leerEnVoz(b) },
                            alResponder: { modelo.responder($0) },
                            alCitar: { modelo.abrir($0) },
                            alPonerClave: { poniendoClave = true })
                            .id(b.id)
                    }
                    if modelo.ocupado {
                        HStack {
                            ProgressView()
                            if modelo.sePuedeParar {
                                Button("Parar") { modelo.parar() }
                                    .buttonStyle(.bordered)
                                    .keyboardShortcut(".", modifiers: .command)
                            }
                        }
                        .padding(.leading, 8).id("pensando")
                    }
                }
                .padding(16)
                .frame(maxWidth: 860)
                .frame(maxWidth: .infinity)
            }
            .scrollDismissesKeyboard(.interactively)
            .onChange(of: modelo.burbujas.count) { _, _ in
                if let ultima = modelo.burbujas.last?.id {
                    withAnimation { lector.scrollTo(ultima, anchor: .bottom) }
                }
            }
            // La respuesta en vivo crece: se sigue su final (sin animar).
            .onChange(of: modelo.burbujas.last?.texto.count) { _, _ in
                if modelo.ocupado, let ultima = modelo.burbujas.last?.id {
                    lector.scrollTo(ultima, anchor: .bottom)
                }
            }
        }
    }

    @ViewBuilder
    private var vacia: some View {
        VStack(alignment: .leading, spacing: 12) {
            if modelo.motor?.faltaClave == true {
                if case let .nube(nombre, _)? = modelo.motor {
                    Text("Falta la clave de \(nombre). Se guarda cifrada en el Llavero de este iPad y no sale de él.")
                        .foregroundStyle(.secondary)
                } else {
                    Text(ModeloAsistente.textoSinMotor).foregroundStyle(.secondary)
                }
                Button {
                    poniendoClave = true
                } label: {
                    Label("Poner la clave", systemImage: "key")
                }
                .buttonStyle(.borderedProminent)
            } else {
                Text("Pregunta algo, pide un dibujo («dibuja una casa con un árbol»), apunta una tarea («recuérdame comprar pan mañana a las 10») o adjunta un PDF y pregunta qué dice.")
                    .foregroundStyle(.secondary)
            }
        }
        .padding()
    }

    // MARK: La caja

    private var caja: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !modelo.pendientes.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack {
                        ForEach(modelo.pendientes) { a in
                            FichaAdjunto(adjunto: a) { modelo.pendientes.removeAll { $0.id == a.id } }
                        }
                    }
                }
            }
            if let aviso = dictado.aviso {
                Text(aviso).font(.caption).foregroundStyle(.orange)
            }
            HStack(alignment: .bottom, spacing: 12) {
                Button { eligiendoFicheros = true } label: { Image(systemName: "paperclip") }
                    .help("Adjuntar un PDF o un archivo (también se pueden arrastrar)")
                PhotosPicker(selection: $fotos, maxSelectionCount: 5, matching: .images) {
                    Image(systemName: "photo")
                }
                .help("Adjuntar una foto")
                TextField(dictado.escuchando ? "Te escucho…" : "Escribe al asistente…", text: $texto, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1 ... 6)
                    .focused($escribiendo)
                    .onSubmit(enviar)
                    .padding(.horizontal, 12).padding(.vertical, 8)
                    .background(Color.secondary.opacity(0.10), in: RoundedRectangle(cornerRadius: 18))
                Button(action: microfono) {
                    Image(systemName: dictado.escuchando ? "mic.fill" : "mic")
                        .foregroundStyle(dictado.escuchando ? Color.red : Color.secondary)
                }
                .help(dictado.escuchando ? "Dejar de escuchar" : "Dictar")
                if modelo.sePuedeParar {
                    Button { modelo.parar() } label: { Image(systemName: "stop.circle.fill").font(.title) }
                        .help("Parar (⌘.)")
                } else {
                    Button(action: enviar) { Image(systemName: "arrow.up.circle.fill").font(.title) }
                        .disabled(texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || modelo.ocupado)
                        .keyboardShortcut(.return, modifiers: .command)
                }
            }
            .font(.title3)
            .buttonStyle(.borderless)
        }
        .padding(12)
    }

    private func enviar() {
        let t = texto
        guard !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, !modelo.ocupado else { return }
        texto = ""
        Task { await modelo.enviar(t) }
    }

    private func microfono() {
        if dictado.escuchando {
            dictado.parar()
            return
        }
        let revisar = modelo.revisarLoDicho
        Task {
            await dictado.empezar(caja: texto, alTexto: { texto = $0 }, alAcabar: { final in
                texto = final
                if !revisar { enviar() }
            })
        }
    }

    /// Lo soltado encima (de Archivos, Fotos u otra app con Split View):
    /// las imagenes por sus bytes; lo demas, copiado mientras dura el
    /// prestamo del sistema.
    private func soltar(_ proveedores: [NSItemProvider]) {
        let modelo = self.modelo
        for p in proveedores {
            if p.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                _ = p.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { datos, _ in
                    guard let datos else { return }
                    DispatchQueue.main.async { MainActor.assumeIsolated { modelo.adjuntar(imagen: datos) } }
                }
                continue
            }
            _ = p.loadFileRepresentation(forTypeIdentifier: UTType.item.identifier) { url, _ in
                guard let url else { return }
                // El fichero del sistema se borra al volver: se copia ya.
                let copia = FileManager.default.temporaryDirectory
                    .appendingPathComponent("pixpin-soltado-\(UUID().uuidString)", isDirectory: true)
                try? FileManager.default.createDirectory(at: copia, withIntermediateDirectories: true)
                let destino = copia.appendingPathComponent(url.lastPathComponent)
                guard (try? FileManager.default.copyItem(at: url, to: destino)) != nil else { return }
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        modelo.adjuntar([destino])
                        try? FileManager.default.removeItem(at: copia)
                    }
                }
            }
        }
    }
}

/// Las conversaciones guardadas.
struct ListaConversaciones: View {
    @ObservedObject var modelo: ModeloAsistente

    var body: some View {
        List {
            Section("Conversaciones") {
                if modelo.guardadas.isEmpty {
                    Text("Aún no hay ninguna.").foregroundStyle(.secondary).font(.caption)
                }
                ForEach(modelo.guardadas) { c in
                    Button {
                        modelo.abrir(c)
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(c.rotulo).lineLimit(2).foregroundStyle(.primary)
                            Text(c.cambiada, style: .date).font(.caption2).foregroundStyle(.secondary)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .listRowBackground(c.id == modelo.conversacion.id ? Color.accentColor.opacity(0.15) : nil)
                    .swipeActions {
                        Button("Borrar", role: .destructive) { modelo.borrar(c) }
                    }
                    .contextMenu {
                        Button("Borrar la conversación", role: .destructive) { modelo.borrar(c) }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
    }
}

/// Un adjunto en la caja o en la burbuja.
struct FichaAdjunto: View {
    let adjunto: ModeloAsistente.Adjunto
    var quitar: (() -> Void)?

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: adjunto.esImagen ? "photo" : "doc.text")
            Text(adjunto.nombre).lineLimit(1).truncationMode(.middle)
            if let quitar {
                Button(action: quitar) { Image(systemName: "xmark.circle.fill") }
                    .buttonStyle(.borderless)
            }
        }
        .font(.caption)
        .padding(.horizontal, 8).padding(.vertical, 4)
        .background(Color.secondary.opacity(0.12), in: Capsule())
        .onTapGesture(count: 2) {
            Navegacion.compartida.abrirFichero(URL(fileURLWithPath: adjunto.ruta), titulo: adjunto.nombre)
        }
    }
}

struct BurbujaAsistente: View {
    let burbuja: ModeloAsistente.Burbuja
    let guardada: Bool
    var pregunta: ModeloAsistente.Pregunta?
    let alGuardar: (_ abrir: Bool) -> Void
    var alLeer: () -> Void = {}
    var alResponder: (Bool) -> Void = { _ in }
    var alCitar: (ModeloAsistente.Cita) -> Void = { _ in }
    var alPonerClave: () -> Void = {}
    @State private var pensadoAbierto = false

    var body: some View {
        HStack {
            if burbuja.delUsuario { Spacer(minLength: 60) }
            VStack(alignment: .leading, spacing: 6) {
                if !burbuja.adjuntos.isEmpty {
                    HStack { ForEach(burbuja.adjuntos) { FichaAdjunto(adjunto: $0) } }
                }
                if !burbuja.pensamiento.isEmpty {
                    DisclosureGroup(isExpanded: $pensadoAbierto) {
                        Text(burbuja.pensamiento).font(.callout).foregroundStyle(.secondary).textSelection(.enabled)
                    } label: {
                        Text(burbuja.segundosPensando.map { "Pensó \(max(1, Int($0.rounded()))) s" } ?? "Pensando…")
                            .font(.callout).foregroundStyle(.secondary)
                    }
                }
                if let llamando = burbuja.llamando {
                    Label(llamando, systemImage: "wand.and.stars").font(.callout).foregroundStyle(.secondary)
                }
                Text(burbuja.texto.isEmpty ? AttributedString("…") : textoBonito).textSelection(.enabled)
                ForEach(Array(burbuja.herramientas.enumerated()), id: \.offset) { _, h in
                    Label(h.nombre + ": " + h.resumen, systemImage: h.ok ? "checkmark.circle" : "xmark.circle")
                        .font(.caption)
                        .lineLimit(3)
                        .foregroundStyle(h.ok ? Color.secondary : Color.orange)
                }
                ForEach(burbuja.citas, id: \.numero) { c in
                    Button { alCitar(c) } label: {
                        Label("[\(c.numero)] \(c.titulo)", systemImage: "doc.text.magnifyingglass")
                            .font(.caption).lineLimit(1)
                    }
                    .buttonStyle(.borderless)
                }
                if let pregunta {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(pregunta.texto).font(.callout.weight(.semibold))
                        HStack {
                            Button("Sí") { alResponder(true) }
                                .buttonStyle(.bordered).tint(.red)
                            Button("No") { alResponder(false) }
                                .buttonStyle(.borderedProminent)
                                .keyboardShortcut(.cancelAction)
                        }
                    }
                    .padding(10)
                    .background(Color.orange.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
                }
                if burbuja.esAviso, burbuja.texto.contains("clave") || burbuja.texto == ModeloAsistente.textoSinMotor {
                    Button(action: alPonerClave) { Label("Poner la clave", systemImage: "key") }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                }
                if burbuja.lienzo != nil {
                    HStack {
                        Button("Abrir en el lienzo") { alGuardar(true) }
                            .buttonStyle(.bordered)
                        Button(guardada ? "Guardado en Mensajes guardados" : "Guardar en Mensajes guardados") { alGuardar(false) }
                            .buttonStyle(.bordered)
                            .disabled(guardada)
                    }
                    .controlSize(.small)
                }
                if !burbuja.delUsuario, !burbuja.esAviso, !burbuja.texto.isEmpty {
                    HStack(spacing: 8) {
                        Button(action: alLeer) { Image(systemName: "speaker.wave.2") }
                            .buttonStyle(.borderless)
                            .help("Leer en voz alta")
                        Button { UIPasteboard.general.string = burbuja.texto } label: { Image(systemName: "doc.on.doc") }
                            .buttonStyle(.borderless)
                            .help("Copiar")
                        if !burbuja.motor.isEmpty {
                            Text(burbuja.motor).font(.caption2).foregroundStyle(.tertiary)
                        }
                    }
                    .font(.caption)
                }
            }
            .padding(12)
            .background(fondo, in: RoundedRectangle(cornerRadius: 14))
            if !burbuja.delUsuario { Spacer(minLength: 60) }
        }
    }

    /// Las respuestas del modelo con sus listas y negritas; lo del usuario
    /// tal cual.
    private var textoBonito: AttributedString {
        burbuja.delUsuario || burbuja.esAviso ? AttributedString(burbuja.texto) : NotaDelAsistente.formateado(burbuja.texto)
    }

    private var fondo: Color {
        if burbuja.esAviso { return Color.orange.opacity(0.15) }
        return burbuja.delUsuario ? Color.accentColor.opacity(0.18) : Color.secondary.opacity(0.12)
    }
}
