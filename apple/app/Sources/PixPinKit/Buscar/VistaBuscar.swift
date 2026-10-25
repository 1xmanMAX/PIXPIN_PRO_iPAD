// «Buscar en PixPin» en el iPad (`VentanaBuscar` del Mac, `buscar_todo.rs`
// de Windows): la caja arriba, las pestanas con su cuenta y la lista con el
// mejor resultado arriba y el resto por grupos, por paginas (al llegar al
// final se pide mas). Con la caja vacia: las busquedas recientes, lo
// abierto hace poco y las letras rapidas (t, n, l, g, u, a).
//
// Tocar una fila hace lo suyo (abrir el fichero con su visor, marcar la
// tarea, ir al chat...); «Abrir en Proyectos» lleva a su chat, en su
// mensaje (`Navegacion.irAlChat`). Mantenerla pulsada da el resto: copiar,
// compartir, conservar o borrar una captura (con deshacer)... Con el
// teclado del iPad: ↑↓, Tab entre pestanas, ↩ y ⌘↩ (ver en el chat).
//
// Lo que se busca lo hace `ModeloBuscar` (espera 200 ms, cancela lo de
// antes); la logica de la lista, `EstadoBuscar` (del Mac tal cual).

import PixPinCore
import SwiftUI
import UIKit

public struct VistaBuscar: View {
    @StateObject private var modelo: ModeloBuscar
    @State private var texto = ""
    @State private var lectorDocumentos: LectorTextoDocumentos?
    @State private var lectorCapturas: LectorTextoGaleria?

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloBuscar(almacen: almacen))
    }

    private var almacen: Almacen { modelo.almacen }
    private var nucleo: Nucleo { modelo.almacen.nucleo }

    public var body: some View {
        VStack(spacing: 0) {
            Cabecera("Buscar", subtitulo: subtitulo,
                     buscador: BuscadorCabecera($texto, pista: "Buscar en todo PixPin, o una letra y espacio", enfocar: true)) {
                if modelo.buscando {
                    ProgressView().controlSize(.small).tint(Paleta.v2.gris)
                }
            }
            pestanas
            Paleta.v2.linea.frame(height: 1)
            if modelo.estado.esInicio {
                InicioBuscar(modelo: modelo, poner: { texto = $0 }, hacer: hacer)
            } else {
                lista
            }
        }
        .ventanaConCabecera()
        .overlay(alignment: .bottom) {
            if let a = modelo.aviso {
                AvisoAbajo(texto: a, deshacer: modelo.estado.seDeshace ? deshacer : nil)
            } else if let e = modelo.error {
                AvisoAbajo(texto: e, esError: true)
            }
        }
        .onChange(of: texto) { _, t in modelo.cambiar(t) }
        .task {
            ArranqueOrganizar.apuntar()
            texto = modelo.estado.consulta
            if modelo.estado.resultados.isEmpty { modelo.refrescar() }
            empezarLectores()
        }
        .onDisappear {
            modelo.cancelar()
            lectorDocumentos?.parar()
            lectorCapturas?.parar()
            lectorDocumentos = nil
            lectorCapturas = nil
        }
        .background { teclas }
    }

    private var subtitulo: String {
        if modelo.estado.esInicio { return "Chats, archivos, tareas, capturas…" }
        let n = modelo.estado.cuenta(.todo)
        return n == 1 ? "1 resultado" : "\(n)\(modelo.hayMas ? "+" : "") resultados"
    }

    // MARK: Pestanas

    private var pestanas: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(PestanaBuscar.allCases) { p in
                    let n = modelo.estado.cuenta(p)
                    let activa = modelo.estado.pestana == p
                    Button { modelo.ir(a: p) } label: {
                        HStack(spacing: 5) {
                            Text(p.rotulo).font(.system(size: 13.5, weight: activa ? .semibold : .regular))
                            if !modelo.estado.esInicio, p != .todo || n > 0 {
                                Text("\(n)").font(.system(size: 11.5).monospacedDigit())
                                    .foregroundStyle(activa ? Color.white.opacity(0.85) : Paleta.v2.gris)
                            }
                        }
                        .foregroundStyle(activa ? Color.white : Paleta.v2.texto)
                        .padding(.horizontal, 12)
                        .frame(height: 34)
                        .background(activa ? Paleta.v2.acento : Paleta.v2.caja, in: Capsule())
                        .contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 8)
        }
        .background(Paleta.v2.fondo)
    }

    // MARK: La lista

    private var lista: some View {
        let lineas = modelo.estado.lineas()
        let visibles = modelo.estado.visibles()
        return ScrollViewReader { lector in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(lineas.enumerated()), id: \.offset) { _, linea in
                        switch linea {
                        case let .cabecera(g):
                            Titulillo(g.rotulo).padding(.top, 12).padding(.bottom, 4)
                        case let .fila(f):
                            let i = visibles.firstIndex(of: f) ?? 0
                            FilaBuscarVista(fila: f, elegida: i == modelo.estado.elegido,
                                            verEnChat: { hacer(.hacer(f, verEnChat: true)) })
                                .id(f.id)
                                .onTapGesture {
                                    modelo.estado.elegido = i
                                    hacer(.hacer(f, verEnChat: false))
                                }
                                .contextMenu { menu(f) }
                        }
                    }
                    if modelo.hayMas {
                        ProgressView().frame(maxWidth: .infinity).padding()
                            .task { await modelo.cargarMas() }
                    }
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 40)
            }
            .scrollDismissesKeyboard(.immediately)
            .onChange(of: modelo.estado.elegido) { _, _ in
                if let f = modelo.estado.filaElegida { lector.scrollTo(f.id) }
            }
        }
        .overlay {
            if lineas.isEmpty, !modelo.buscando {
                ContentUnavailableView.search(text: modelo.estado.consulta)
            }
        }
    }

    /// Lo que se puede hacer con una fila (mantenerla pulsada).
    @ViewBuilder
    private func menu(_ f: FilaBuscar) -> some View {
        Button { hacer(.hacer(f, verEnChat: false)) } label: { Text(rotuloPrincipal(f)) }
        if let r = f.resultado {
            if r.chat != nil, r.clase != .chat {
                Button { hacer(.hacer(f, verEnChat: true)) } label: {
                    Label("Abrir en Proyectos", systemImage: "bubble.left.and.bubble.right")
                }
            }
            if let ruta = r.ruta {
                let url = URL(fileURLWithPath: ruta)
                Button { Navegacion.compartida.abrir(.vistaRapida(url: url)) } label: {
                    Label("Vista rápida", systemImage: "eye")
                }
                ShareLink(item: url) { Label("Compartir…", systemImage: "square.and.arrow.up") }
            }
            if tieneCopiar(f) {
                Button { copiar(r) } label: { Label("Copiar", systemImage: "doc.on.doc") }
            }
            if r.clase == .captura {
                if !r.conservada {
                    Button { conservar(r) } label: { Label("Conservar", systemImage: "checkmark.seal") }
                }
                Button { Navegacion.compartida.seccion = .galeria } label: {
                    Label("Abrir la galería", systemImage: "photo.on.rectangle")
                }
                Divider()
                Button(role: .destructive) { borrar(r) } label: { Label("Borrar", systemImage: "trash") }
            }
        }
    }

    /// Las teclas del teclado del iPad (las de Windows).
    private var teclas: some View {
        ZStack {
            Button("") { modelo.estado.mover(-1) }.keyboardShortcut(.upArrow, modifiers: [])
            Button("") { modelo.estado.mover(1) }.keyboardShortcut(.downArrow, modifiers: [])
            Button("") { modelo.ir(a: modelo.estado.pestana.siguiente(atras: false)) }
                .keyboardShortcut(.tab, modifiers: [])
            Button("") { modelo.ir(a: modelo.estado.pestana.siguiente(atras: true)) }
                .keyboardShortcut(.tab, modifiers: [.shift])
            Button("") { hacer(modelo.estado.boton(.verEnChat)) }.keyboardShortcut(.return, modifiers: [.command])
            Button("") { deshacer() }.keyboardShortcut("z").disabled(!modelo.estado.seDeshace)
        }
        .opacity(0)
        .allowsHitTesting(false)
    }

    private func empezarLectores() {
        guard lectorDocumentos == nil else { return }
        let refrescar: () -> Void = { [weak modelo = self.modelo] in
            guard let modelo, !modelo.estado.esInicio else { return }
            modelo.refrescar(mantener: true)
        }
        let d = LectorTextoDocumentos(nucleo: nucleo, alLeer: refrescar)
        let c = LectorTextoGaleria(nucleo: nucleo, alLeer: refrescar)
        lectorDocumentos = d
        lectorCapturas = c
        d.despertar()
        c.despertar()
    }

    // MARK: Hacer

    private func hacer(_ o: OrdenBuscar) {
        switch o {
        case let .hacer(f, verEnChat):
            if f.resultado != nil { modelo.recordar(f) }
            if verEnChat, let r = f.resultado {
                saltar(r)
            } else {
                principal(f)
            }
        case let .copiar(f):
            if let r = f.resultado { copiar(r) }
        case let .borrarCaptura(r):
            borrar(r)
        case .deshacer:
            deshacer()
        case .buscar:
            modelo.refrescar()
        case .vaciar:
            texto = ""
        default:
            break
        }
    }

    /// Lo de cada clase, como el doble clic de Windows.
    private func principal(_ f: FilaBuscar) {
        switch f {
        case let .accion(a):
            accion(a)
        case let .resultado(r):
            switch r.clase {
            case .chat:
                if let chat = r.chat { Navegacion.compartida.irAlChat(chat) }
            case .mensaje:
                if r.tipo == "NOTA" { abrirNota(r) } else { saltar(r) }
            case .fichero:
                abrirFichero(r)
            case .tarea:
                marcar(r, hecha: !r.hecha)
            case .leccion:
                Navegacion.compartida.seccion = .lecciones
            case .captura:
                if let ruta = r.ruta {
                    Navegacion.compartida.abrirFichero(URL(fileURLWithPath: ruta), titulo: r.titulo)
                }
            case .momento:
                Navegacion.compartida.seccion = .timeline
            }
        }
    }

    /// «Abrir en Proyectos»: el chat, en su mensaje y marcado.
    private func saltar(_ r: ResultadoBusqueda) {
        guard let chat = r.chat else { return }
        Navegacion.compartida.irAlChat(chat, mensaje: r.mensaje)
    }

    /// El mensaje entero (su texto, su referencia), leido del chat.
    private func mensaje(_ r: ResultadoBusqueda) async -> MensajeChat? {
        guard let chat = r.chat, let pos = r.posicion else { return nil }
        let nucleo = self.nucleo
        let pagina = try? await Task.detached(priority: .userInitiated) {
            try nucleo.mensajes(chat: chat, desde: pos, cuantos: 1)
        }.value
        return pagina?.mensajes.first { $0.id == r.mensaje }
    }

    private func abrirNota(_ r: ResultadoBusqueda) {
        Task {
            guard let chat = r.chat, let m = await mensaje(r) else { return saltar(r) }
            Navegacion.compartida.abrir(.nota(chat: chat, mensaje: m))
        }
    }

    /// Un fichero, como desde su burbuja: el visor que toca.
    private func abrirFichero(_ r: ResultadoBusqueda) {
        Task {
            let m = await mensaje(r)
            if r.tipo == "DIBUJO", let chat = r.chat, let m {
                Navegacion.compartida.abrir(.lienzo(chat: chat, mensaje: m, foto: nil))
                return
            }
            guard let ruta = r.ruta, FileManager.default.fileExists(atPath: ruta) else {
                // De otro aparato o aun sin llegar: su sitio en el chat.
                return saltar(r)
            }
            Navegacion.compartida.abrirFichero(URL(fileURLWithPath: ruta), titulo: r.titulo.isEmpty ? nil : r.titulo,
                                               chat: r.chat, mensaje: m)
        }
    }

    private func marcar(_ r: ResultadoBusqueda, hecha: Bool) {
        guard let chat = r.chat, let msg = r.mensaje, let i = r.tarea else { return }
        let nucleo = self.nucleo
        let crudo = r.crudo
        Task {
            let sigue = try? await Task.detached(priority: .userInitiated) {
                try nucleo.marcarTarea(chat: chat, mensaje: msg, indice: i, crudo: crudo, hecha: hecha)
            }.value
            if sigue == true {
                modelo.avisar(hecha ? "Hecha" : "Pendiente otra vez")
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: chat)
            } else {
                modelo.avisar("La lista cambió: se ha vuelto a leer")
            }
            modelo.refrescar(mantener: true)
        }
    }

    private func accion(_ a: AccionBuscar) {
        switch a {
        case let .apuntarTarea(t):
            guard !t.trimmingCharacters(in: .whitespaces).isEmpty else { return }
            let nucleo = self.nucleo
            let desfase = desfaseLocalMs()
            Task {
                do {
                    let l = try await Task.detached(priority: .userInitiated) {
                        try nucleo.apuntarTarea(texto: t, desfaseMs: desfase)
                    }.value
                    NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                    NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: l.chat)
                    modelo.avisar("Apuntada en el Inbox")
                    texto = ""
                } catch {
                    modelo.avisar("No se pudo apuntar: \(error)")
                }
            }
        case let .nuevaNota(t):
            Task {
                guard let guardados = try? nucleo.chatGuardados(),
                      let m = await almacen.escribir(nota: t.isEmpty ? "# Nota nueva" : t, en: guardados.id) else {
                    modelo.avisar("No se pudo crear la nota")
                    return
                }
                Navegacion.compartida.abrir(.nota(chat: guardados.id, mensaje: m))
            }
        case .nuevoLienzo:
            guard let guardados = try? nucleo.chatGuardados() else { return }
            Navegacion.compartida.abrir(.lienzo(chat: guardados.id, mensaje: nil, foto: nil))
        case .capturar:
            // iOS no deja capturar otras apps: la galeria (las fotos y la
            // camara entran por Proyectos).
            Navegacion.compartida.seccion = .galeria
        case .ultimaCaptura:
            let p = try? nucleo.galeria(desde: 0, cuantos: 1, dias: AjusteCaducidad.dias, desfaseMs: desfaseLocalMs())
            guard let c = p?.capturas.first else {
                modelo.avisar("Aún no hay capturas")
                return
            }
            Navegacion.compartida.abrirFichero(URL(fileURLWithPath: c.ruta), titulo: c.nombre)
        case let .ventana(id, _):
            ir(id)
        }
    }

    /// Una «ventana» de Windows es una seccion del iPad.
    private func ir(_ id: String) {
        if let s = VistaBuscar.seccion(de: id) { Navegacion.compartida.seccion = s }
    }

    static func seccion(de id: String) -> Seccion? {
        switch id {
        case IdVentana.galeria: return .galeria
        case IdVentana.timeline: return .timeline
        case IdVentana.tareas: return .tareas
        case IdVentana.proyectos: return .proyectos
        case "lecciones": return .lecciones
        case "asistente": return .asistente
        default: return nil
        }
    }

    private func copiar(_ r: ResultadoBusqueda) {
        if r.clase == .captura || r.tipo == "IMAGEN", let ruta = r.ruta, let img = UIImage(contentsOfFile: ruta) {
            UIPasteboard.general.image = img
        } else if r.clase == .fichero, let ruta = r.ruta {
            UIPasteboard.general.url = URL(fileURLWithPath: ruta)
        } else {
            let t = r.clase == .tarea ? r.titulo : (r.fragmento.isEmpty ? r.titulo : r.fragmento)
            UIPasteboard.general.string = t
        }
        modelo.avisar("Copiado")
    }

    private func conservar(_ r: ResultadoBusqueda) {
        let nombre = r.titulo
        let nucleo = self.nucleo
        Task {
            let hecho = await Task.detached(priority: .userInitiated) {
                (try? nucleo.conservarCaptura(nombre: nombre)) != nil
            }.value
            modelo.avisar(hecho ? "Conservada: está en Mensajes guardados" : "No se pudo conservar")
            if hecho { NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil) }
            modelo.refrescar(mantener: true)
        }
    }

    /// A la papelera de PixPin, con deshacer.
    private func borrar(_ r: ResultadoBusqueda) {
        let nombre = r.titulo
        let nucleo = self.nucleo
        Task {
            let borradas = await Task.detached(priority: .userInitiated) {
                (try? nucleo.borrarCapturas(nombres: [nombre])) ?? []
            }.value
            guard !borradas.isEmpty else {
                modelo.avisar("No se pudo borrar")
                return
            }
            modelo.borradas = borradas
            modelo.estado.seDeshace = true
            modelo.avisar("Captura llevada a la papelera de PixPin")
            NotificationCenter.default.post(name: .pixpinCapturasBarridas, object: nil)
            modelo.refrescar(mantener: true)
        }
    }

    private func deshacer() {
        let borradas = modelo.borradas
        guard !borradas.isEmpty else { return }
        modelo.borradas = []
        modelo.estado.seDeshace = false
        let nucleo = self.nucleo
        Task {
            let n = await Task.detached(priority: .userInitiated) {
                (try? nucleo.devolverCapturas(borradas: borradas)) ?? 0
            }.value
            modelo.avisar(n == 0 ? "No se pudo deshacer" : "Recuperada")
            NotificationCenter.default.post(name: .pixpinCapturasBarridas, object: nil)
            modelo.refrescar(mantener: true)
        }
    }
}

// MARK: - El inicio

/// Con la caja vacia: las busquedas recientes, lo abierto hace poco y las
/// letras rapidas.
struct InicioBuscar: View {
    @ObservedObject var modelo: ModeloBuscar
    let poner: (String) -> Void
    let hacer: (OrdenBuscar) -> Void

    /// En el iPad no se captura la pantalla de otras apps: sin «c».
    private var letras: [LetraRapida] { LetrasRapidas.todas.filter { $0.letra != "c" } }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                if !modelo.busquedas.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Titulillo("Búsquedas recientes")
                            Spacer()
                            Button("Borrar todas") { modelo.borrarBusquedas() }
                                .buttonStyle(.plain)
                                .font(.system(size: 13))
                                .foregroundStyle(Paleta.v2.acento)
                        }
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 8) {
                                ForEach(Array(modelo.busquedas.enumerated()), id: \.offset) { i, b in
                                    Button { poner(b) } label: {
                                        Text(b).font(.system(size: 13.5)).foregroundStyle(Paleta.v2.texto)
                                            .padding(.horizontal, 12).frame(height: 32)
                                            .background(Paleta.v2.caja, in: Capsule())
                                    }
                                    .buttonStyle(.plain)
                                    .contextMenu {
                                        Button("Quitar", role: .destructive) { modelo.quitarBusqueda(i) }
                                    }
                                }
                            }
                        }
                    }
                }
                if !modelo.estado.resultados.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Titulillo("Abierto hace poco")
                        ForEach(modelo.estado.visibles()) { f in
                            FilaBuscarVista(fila: f, elegida: false,
                                            verEnChat: { hacer(.hacer(f, verEnChat: true)) })
                                .onTapGesture { hacer(.hacer(f, verEnChat: false)) }
                        }
                    }
                }
                VStack(alignment: .leading, spacing: 8) {
                    Titulillo("Escribe una letra y espacio")
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 230), spacing: 8)], spacing: 8) {
                        ForEach(letras, id: \.letra) { l in
                            Button { poner("\(l.letra) ") } label: {
                                HStack(spacing: 10) {
                                    Text(String(l.letra))
                                        .font(.system(size: 14, weight: .bold, design: .monospaced))
                                        .foregroundStyle(Paleta.v2.acento)
                                        .frame(width: 30, height: 30)
                                        .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 7))
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(l.titulo).font(.system(size: 13.5, weight: .medium))
                                            .foregroundStyle(Paleta.v2.texto)
                                        Text(l.sub).font(.system(size: 12)).foregroundStyle(Paleta.v2.gris)
                                            .lineLimit(1)
                                    }
                                    Spacer(minLength: 0)
                                }
                                .padding(10)
                                .background(Paleta.v2.tarjeta, in: RoundedRectangle(cornerRadius: 10))
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
            .padding(20)
        }
        .scrollDismissesKeyboard(.immediately)
        .background(Paleta.v2.fondo)
    }
}

/// El titulito gris de una seccion.
struct Titulillo: View {
    let texto: String
    init(_ texto: String) { self.texto = texto }

    var body: some View {
        Text(texto.uppercased())
            .font(.system(size: 11.5, weight: .semibold))
            .tracking(0.6)
            .foregroundStyle(Paleta.v2.gris)
    }
}

// MARK: - Una fila

/// Una fila: su icono (la miniatura si es una foto), el titulo con lo
/// encontrado resaltado, de donde es y cuando, y sus botones: lo principal
/// y «Abrir en Proyectos».
struct FilaBuscarVista: View {
    let fila: FilaBuscar
    let elegida: Bool
    let verEnChat: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            IconoFila(fila: fila)
            VStack(alignment: .leading, spacing: 3) {
                titulo
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(Paleta.v2.texto)
                    .lineLimit(1)
                Text(subtitulo)
                    .font(.system(size: 12.5))
                    .foregroundStyle(Paleta.v2.gris)
                    .lineLimit(1)
                if case let .resultado(r) = fila, r.clase != .tarea, !r.fragmento.isEmpty, r.fragmento != r.titulo {
                    Text(r.fragmento).font(.system(size: 12.5)).foregroundStyle(Paleta.v2.cuerpo).lineLimit(2)
                }
            }
            Spacer(minLength: 8)
            if tieneVerEnChat(fila) {
                Button(action: verEnChat) {
                    Label("Abrir en Proyectos", systemImage: "bubble.left.and.bubble.right")
                        .labelStyle(.iconOnly)
                        .font(.system(size: 15))
                        .foregroundStyle(Paleta.v2.acento)
                        .frame(width: 40, height: 40)
                        .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 10))
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Abrir en Proyectos")
            }
            Text(rotuloPrincipal(fila))
                .font(.system(size: 12.5, weight: .medium))
                .foregroundStyle(elegida ? Color.white : Paleta.v2.cuerpo)
                .padding(.horizontal, 10).frame(height: 30)
                .background(elegida ? Paleta.v2.azulLleno : Paleta.v2.caja, in: Capsule())
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(elegida ? Paleta.v2.tarjetaEncima : Paleta.v2.tarjeta.opacity(0.0001),
                    in: RoundedRectangle(cornerRadius: 10))
        .contentShape(Rectangle())
        .hoverEffect(.highlight)
    }

    private var titulo: Text {
        switch fila {
        case let .accion(a): return Text(a.titulo)
        case let .resultado(r):
            let t = r.titulo.isEmpty ? r.nombreChat : r.titulo
            return textoResaltado(t, posiciones: r.titulo.isEmpty ? [] : r.resaltado,
                                  tachado: r.clase == .tarea && r.hecha)
        }
    }

    private var subtitulo: String {
        switch fila {
        case let .accion(a): return a.subtitulo
        case let .resultado(r): return subtituloDe(r)
        }
    }
}

/// «Captura · hoy 09:12», «Tarea · Obra · Inbox»…
func subtituloDe(_ r: ResultadoBusqueda) -> String {
    var trozos: [String] = []
    switch r.clase {
    case .chat: trozos.append("Chat")
    case .mensaje: trozos.append(r.tipo == "NOTA" ? "Nota" : "Mensaje")
    case .fichero:
        switch r.tipo {
        case "IMAGEN": trozos.append("Foto")
        case "VOZ": trozos.append("Audio")
        case "VIDEO": trozos.append("Vídeo")
        case "DIBUJO": trozos.append("Lienzo")
        case "PAGINA": trozos.append("Hoja")
        default: trozos.append("Archivo")
        }
    case .tarea: trozos.append(r.hecha ? "Tarea hecha" : "Tarea")
    case .leccion: trozos.append("Lección")
    case .captura: trozos.append("Captura")
    case .momento: trozos.append("Momento")
    }
    if !r.nombreChat.isEmpty, r.clase != .chat { trozos.append(r.nombreChat) }
    if r.clase == .tarea, !r.fragmento.isEmpty { trozos.append(r.fragmento) }
    if r.dentro { trozos.append(r.clase == .captura ? "en su texto" : "dentro del documento") }
    if r.cuando > 0 { trozos.append(fechaCorta(r.cuando)) }
    if r.clase == .captura {
        if r.conservada {
            trozos.append("conservada")
        } else if let d = r.diasQueQuedan {
            trozos.append(d <= 0 ? "caduca hoy" : "caduca en \(d) \(d == 1 ? "día" : "días")")
        }
    }
    return trozos.joined(separator: " · ")
}

/// «hoy 09:12», «ayer», «3 sep», «3 sep 2024».
func fechaCorta(_ ms: Int64, ahora: Date = Date()) -> String {
    let f = fechaDe(ms: ms)
    let cal = Calendar.current
    if cal.isDateInToday(f) { return "hoy " + f.formatted(date: .omitted, time: .shortened) }
    if cal.isDateInYesterday(f) { return "ayer" }
    if cal.isDate(f, equalTo: ahora, toGranularity: .year) {
        return f.formatted(.dateTime.day().month(.abbreviated))
    }
    return f.formatted(.dateTime.day().month(.abbreviated).year())
}

/// `texto` con las letras de `posiciones` (en UTF-16) resaltadas.
func textoResaltado(_ texto: String, posiciones: [UInt32], tachado: Bool = false) -> Text {
    var a = AttributedString(texto)
    if tachado { a.strikethroughStyle = .single }
    let ns = texto as NSString
    let marcadas = Set(posiciones.map(Int.init))
    var i = 0
    while i < ns.length {
        guard marcadas.contains(i) else { i += 1; continue }
        var j = i
        while j < ns.length, marcadas.contains(j) { j += 1 }
        if let r = Range(NSRange(location: i, length: j - i), in: texto),
           let desde = AttributedString.Index(r.lowerBound, within: a),
           let hasta = AttributedString.Index(r.upperBound, within: a) {
            a[desde ..< hasta].foregroundColor = Paleta.v2.acento
            a[desde ..< hasta].inlinePresentationIntent = .stronglyEmphasized
        }
        i = j
    }
    return Text(a)
}

/// El icono de una fila (la foto de una captura, en pequeno).
struct IconoFila: View {
    let fila: FilaBuscar

    var body: some View {
        Group {
            if let r = fila.resultado, r.clase == .captura || r.tipo == "IMAGEN", let ruta = r.ruta {
                MiniaturaCaptura(url: URL(fileURLWithPath: ruta), esVideo: false)
            } else {
                Image(systemName: nombre)
                    .font(.system(size: 16))
                    .foregroundStyle(color)
            }
        }
        .frame(width: 38, height: 38)
        .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 8))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private var nombre: String {
        switch fila {
        case let .accion(a): return a.icono
        case let .resultado(r): return iconoDe(r)
        }
    }

    private var color: Color {
        guard let r = fila.resultado else { return Paleta.v2.acento }
        switch r.clase {
        case .tarea: return r.hecha ? Paleta.v2.verde : Paleta.v2.texto
        case .leccion: return Paleta.v2.amarillo
        case .momento: return Paleta.v2.naranja
        default: return Paleta.v2.cuerpo
        }
    }
}

func iconoDe(_ r: ResultadoBusqueda) -> String {
    switch r.clase {
    case .chat: return "bubble.left.and.bubble.right"
    case .tarea: return r.hecha ? "checkmark.square" : "square"
    case .momento: return "clock"
    case .leccion: return "lightbulb"
    case .captura: return "camera.viewfinder"
    case .mensaje: return r.tipo == "NOTA" ? "text.bubble" : "bubble.left"
    case .fichero:
        let ext = ((r.ruta ?? r.titulo) as NSString).pathExtension.lowercased()
        switch ext {
        case "pdf": return "doc.richtext"
        case "doc", "docx", "odt", "rtf", "md", "txt": return "doc.text"
        case "xls", "xlsx", "ods", "csv": return "tablecells"
        case "ppt", "pptx", "odp", "key": return "rectangle.on.rectangle"
        case "epub": return "book"
        case "dwg", "dxf": return "square.grid.3x3"
        default:
            switch r.tipo {
            case "IMAGEN": return "photo"
            case "VOZ": return "waveform"
            case "VIDEO": return "film"
            case "DIBUJO": return "scribble.variable"
            default: return "doc"
            }
        }
    }
}
