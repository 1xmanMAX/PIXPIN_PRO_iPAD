// **Las carpetas de paginas** («Dia 1, Dia 2…», Windows
// `ventana_chat/carpetas.rs`, 26-oct-2026): un cuaderno con nombre dentro
// del chat que junta lienzos, fotos y paginas sin moverlas, en su orden.
//
// - En el chat, la carpeta es su tarjeta (`BurbujaCarpeta`): el nombre,
//   cuantas paginas lleva y las primeras en pequeno. Un toque la abre.
// - Al abrirla (`VistaCarpeta`, la ventana de Windows): arriba el nombre,
//   «Nueva página» (la plantilla de la ultima otra vez, vacia, con el titulo
//   siguiente) y «Exportar PDF» (la hoja de compartir con sus paginas en
//   orden: PDF, imprimir, guardar en Archivos); debajo, las paginas en
//   rejilla con su miniatura, su numero y su titulo. Se abren con un toque,
//   se cambian de sitio arrastrandolas (o con «Antes» y «Después»), se
//   renombran y se quitan de la carpeta (la pagina sigue en el chat).
// - En el menu de un mensaje, «Añadir a carpeta…» (`MenuEstudio.swift`).
//
// Todo lo que cambia se escribe en el cuaderno por el nucleo, con las
// mismas llamadas que el Mac (`carpetas.rs`: `crearCarpetaDePaginas`,
// `carpetaDePaginas`, `anadirACarpeta`, `cambiarCarpeta`,
// `paginaSiguienteDeCarpeta`, `adjuntarALaClase`), con el formato de
// Windows: viaja igual al PC, al Mac y al movil.
//
// **La carpeta de una grabacion** (Windows e1150e8, 25-oct-2026; del Mac,
// `Chat/Carpetas.swift`): la que deja «Grabar clase» se ve en el chat como
// UNA tarjeta (`BurbujaGrabacion`: la primera captura, «● Grabación», lo que
// duro, cuantas capturas y «Estudiar»; tocarla abre «Estudiar la clase»).
// Su ventana ensena lo grabado con su minuto en la esquina, en el orden del
// tiempo (no se mueve), y detras, en ambar y con la chapa «Adjunto · no es
// de la grabación», lo adjuntado a la clase; arriba, «Estudiar» y «Adjuntar
// a la clase». Y **lo que esta en cualquier carpeta no sale suelto en el
// chat** (`CarpetasDelChat.sinLoDeDentro`): lo representa su tarjeta; con la
// lupa si se encuentra, y al ir a el desde su carpeta.

import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

// MARK: - Lo puro

/// Los textos de la carpeta de una grabacion (`carpeta-grabacion*`,
/// `carpeta-adjunt*` de Windows).
enum TextosCarpeta {
    static let grabacion = "Grabación"
    static let estudiar = "Estudiar"
    static let adjuntoInsignia = "Adjunto · no es de la grabación"
    static let adjuntar = "Adjuntar a la clase"
    static func adjuntado(_ nombre: String) -> String { "Adjuntado a la clase: \(nombre)" }
    static let grabacionPista = "Toca una pieza para estudiarla en su minuto · «Adjuntar a la clase» suma un archivo, una foto o una nota sin mezclarlo con lo grabado"
    static let noSePudo = "No se pudo: mira el registro de PixPin"
    static let noEsta = "Esa carpeta ya no está en el chat"
}

/// El color de lo adjuntado a una clase (no es de lo grabado): un ambar que
/// se lee igual en claro y en oscuro (`COLOR_ADJUNTO` de Windows).
let colorAdjunto = Color(red: 0.80, green: 0.45, blue: 0.05)

extension CarpetasDelChat {
    /// Si el mensaje es la carpeta de una grabacion.
    nonisolated static func esGrabacion(_ m: MensajeChat) -> Bool {
        esCarpeta(m) && grabacionDeCarpeta(texto: m.texto) != nil
    }

    /// **Lo que se ve en el chat**: sin lo que esta dentro de alguna de las
    /// carpetas de `lista` (`marcar_lo_de_dentro` y `se_ve` de Windows), que
    /// lo representa su tarjeta. `salvo` (el mensaje al que se fue desde su
    /// carpeta) si se ve. Es una regla de lo que se ensena: el mensaje sigue
    /// en el chat (viaja, se busca, sale en la galeria).
    nonisolated static func sinLoDeDentro(_ lista: [MensajeChat], salvo: String?) -> [MensajeChat] {
        let textos = lista.filter(esCarpeta).map(\.texto)
        guard !textos.isEmpty else { return lista }
        let dentro = Set(dentroDeCarpetas(textos: textos))
        guard !dentro.isEmpty else { return lista }
        return lista.filter { !dentro.contains($0.id) || $0.id == salvo }
    }

    /// Lo que pone debajo del nombre: cuantas lleva (`fila_de` de Windows);
    /// la de una grabacion, lo que duro y cuantas capturas.
    nonisolated static func detalle(_ m: MensajeChat) -> String {
        if let g = grabacionDeCarpeta(texto: m.texto) { return g.detalle }
        return CarpetasChat.cuantas(paginasDeCarpeta(texto: m.texto).count)
    }

    /// **Un toque en la tarjeta de una grabacion**: la clase en «Estudiar la
    /// clase» (su vista natural); sin nota que estudiar, su carpeta.
    static func abrirGrabacion(_ m: MensajeChat, chat: String, almacen: Almacen) {
        guard let g = grabacionDeCarpeta(texto: m.texto), !g.voz.isEmpty else {
            return VistaCarpeta.abrir(chat: chat, carpeta: m.id, almacen: almacen)
        }
        EstudiarClase.abrir(chat: chat, mensaje: g.voz, ms: 0, almacen: almacen)
    }
}

enum CarpetasChat {
    /// Los textos de Windows (`carpeta-*`).
    static let nombreDeSerie = "Cuaderno"
    static let pagina = "Página"

    /// La palabra de `miniapp` de una carpeta (`carpeta_de_paginas::MINIAPP`).
    static let palabra = "carpeta"

    /// Si un mensaje es una carpeta (`es_carpeta`).
    static func esCarpeta(_ m: MensajeChat) -> Bool {
        m.clase == "MINIAPP" && m.miniapp == palabra
    }

    /// Los `id` de sus paginas, en orden (vacio si el documento no se
    /// entiende): lo de la tarjeta, sin leer el chat.
    static func paginas(_ m: MensajeChat) -> [String] {
        guard esCarpeta(m) else { return [] }
        return paginasDeCarpeta(texto: m.texto)
    }

    /// «1 página», «3 páginas» o «Vacía» (`carpeta-paginas`).
    static func cuantas(_ n: Int) -> String {
        switch n {
        case 0: "Vacía"
        case 1: "1 página"
        default: "\(n) páginas"
        }
    }

    /// Si el mensaje se ofrece para «Añadir a carpeta…» (`se_ofrece`): lo
    /// que puede ser una pagina, no otra carpeta ni un mensaje de sistema.
    static func seOfrece(_ m: MensajeChat) -> Bool {
        !esCarpeta(m) && !m.id.isEmpty && !m.id.hasPrefix("auto-")
    }

    /// Lo que se ve a la vista en el menu (un lienzo, una pagina, una foto);
    /// lo demas, en «Más», como Windows.
    static func vaALaVista(_ m: MensajeChat) -> Bool {
        ["DIBUJO", "PAGINA", "IMAGEN"].contains(m.clase)
    }

    /// El titulo de una pagina: el suyo en la carpeta (el nucleo ya pone el
    /// nombre del mensaje si no lleva) o, sin ninguno, el de su fila.
    static func titulo(_ p: PaginaDeCarpeta) -> String {
        let t = p.titulo.trimmingCharacters(in: .whitespaces)
        if !t.isEmpty { return t }
        return TextosCompartir.nombreDeFila(nombre: p.mensaje.nombre, resumen: p.mensaje.resumen)
    }

    /// Las carpetas de lo cargado del chat, la mas nueva abajo.
    static func delChat(_ mensajes: [MensajeChat]) -> [MensajeChat] {
        mensajes.filter(esCarpeta)
    }

    /// El icono de una pagina sin miniatura (`icono_de`): un video (una foto
    /// o un archivo con extension de video, `es_video`), la camara; lo demas,
    /// el de su clase.
    static func icono(_ m: MensajeChat?) -> String {
        let ext = ((m?.nombre ?? "") as NSString).pathExtension.lowercased()
        let videos = ["mp4", "mov", "avi", "mkv", "webm", "3gp", "m4v", "wmv", "flv"]
        if m?.clase == "IMAGEN" || m?.clase == "ARCHIVO", videos.contains(ext) { return "video" }
        return switch m?.clase ?? "" {
        case "DIBUJO": "scribble.variable"
        case "PAGINA": "book"
        case "IMAGEN": "photo"
        case "VOZ": "mic"
        case "NOTA", "": "square.and.pencil"
        default: "doc"
        }
    }
}

// MARK: - Lo que hace (con el nucleo)

@MainActor
enum AccionesCarpeta {
    /// **Una carpeta nueva al final del chat**, con `primera` dentro si se
    /// da. El nombre lo pone el nucleo, sin repetir («Cuaderno», «Cuaderno
    /// 2»…).
    static func crear(en chat: String, con primera: MensajeChat?, almacen: Almacen) async -> MensajeChat? {
        let nucleo = almacen.nucleo
        let id = primera?.id
        let r = await Task.detached(priority: .userInitiated) { () -> Result<MensajeChat, Error> in
            Result { try nucleo.crearCarpetaDePaginas(chat: chat, primera: id) }
        }.value
        switch r {
        case let .success(m):
            await almacen.alFinal(m, en: chat)
            return m
        case let .failure(e):
            almacen.ponerError("\(e)")
            return nil
        }
    }

    /// **Cambia una carpeta** y pone su tarjeta al dia en el chat. `nil` si
    /// ya no esta.
    static func cambiar(_ carpeta: String, en chat: String, _ cambio: CambioCarpeta,
                        almacen: Almacen) async -> MensajeChat? {
        let nucleo = almacen.nucleo
        let r = await Task.detached(priority: .userInitiated) { () -> MensajeChat? in
            (try? nucleo.cambiarCarpeta(chat: chat, carpeta: carpeta, cambio: cambio)) ?? nil
        }.value
        if let r { almacen.sustituir(r, en: chat) }
        return r
    }

    /// **«Añadir a carpeta»**: mete `m` en `carpeta`, al final y con el
    /// titulo que le toca, y dice como entro (o que ya estaba).
    static func anadir(_ m: MensajeChat, a carpeta: MensajeChat, en chat: String,
                       almacen: Almacen, avisar: @escaping (String) -> Void) {
        let (nucleo, id, cid) = (almacen.nucleo, m.id, carpeta.id)
        _ = Task<Void, Never> {
            let r = await Task.detached(priority: .userInitiated) { () -> Result<AnadidaACarpeta?, Error> in
                Result { try nucleo.anadirACarpeta(chat: chat, carpeta: cid, pagina: id) }
            }.value
            switch r {
            case let .success(a?):
                almacen.sustituir(a.carpeta, en: chat)
                avisar(a.yaEstaba ? "Ya está en «\(a.nombre)»" : "Añadida a «\(a.nombre)» como «\(a.titulo)»")
            case .success(nil):
                avisar("Esa carpeta ya no está en el chat")
            case .failure:
                avisar("No se pudo: mira el registro de PixPin")
            }
        }
    }
}

// MARK: - La tarjeta del chat

/// **La carpeta en el chat**: su nombre, cuantas paginas lleva y las
/// primeras en pequeno (`ids_de_la_tarjeta` de Windows).
struct BurbujaCarpeta: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @Environment(\.escalaChat) private var escala

    static func esCarpeta(_ m: MensajeChat) -> Bool { CarpetasChat.esCarpeta(m) }
    /// Cuantas paginas se ven en la tarjeta, una al lado de otra (Windows y
    /// el Mac: tres, y «+N»).
    static let enLaTarjeta = 3

    var body: some View {
        let paginas = CarpetasChat.paginas(mensaje)
        VStack(alignment: .leading, spacing: 8 * escala) {
            HStack(spacing: 10 * escala) {
                Image(systemName: "book.closed.fill")
                    .font(.system(size: 20 * escala))
                    .foregroundStyle(.white)
                    .frame(width: 40 * escala, height: 40 * escala)
                    .background(Color(hex: 0xB86B1E), in: RoundedRectangle(cornerRadius: 10))
                VStack(alignment: .leading, spacing: 1) {
                    Text(mensaje.nombre.isEmpty ? CarpetasChat.nombreDeSerie : mensaje.nombre)
                        .font(.system(size: 14 * escala, weight: .semibold)).lineLimit(1)
                    Text("Carpeta de páginas · \(CarpetasChat.cuantas(paginas.count))")
                        .font(.caption).opacity(0.7)
                }
            }
            if !paginas.isEmpty {
                HStack(spacing: 6 * escala) {
                    ForEach(Array(paginas.prefix(Self.enLaTarjeta).enumerated()), id: \.element) { i, p in
                        MiniaturaDePagina(pagina: mensajeDe(p), chat: chat, almacen: almacen,
                                          lado: 64 * escala)
                            .overlay(alignment: .bottomLeading) {
                                Text("\(i + 1)")
                                    .font(.system(size: 9 * escala, weight: .bold))
                                    .padding(.horizontal, 4).padding(.vertical, 1)
                                    .background(.black.opacity(0.55), in: Capsule())
                                    .foregroundStyle(.white)
                                    .padding(3)
                            }
                    }
                    if paginas.count > Self.enLaTarjeta {
                        Text("+\(paginas.count - Self.enLaTarjeta)").font(.caption.weight(.semibold)).opacity(0.7)
                    }
                }
            }
        }
        .frame(maxWidth: 340, alignment: .leading)
    }

    private func mensajeDe(_ id: String) -> MensajeChat? {
        almacen.mensajes[chat]?.first { $0.id == id }
    }
}

// MARK: - La tarjeta de una grabacion

/// **La tarjeta de una clase grabada** (`pintar_ojeada_grabacion` de
/// Windows): una sola en el chat por toda la grabacion. La primera captura de
/// fondo (sin capturas, el icono del video o del microfono), arriba la chapa
/// «● Grabación», abajo lo que duro y cuantas capturas lleva y la pastilla
/// «Estudiar». Un toque en ella abre la clase (`VistaChat.abrir`).
struct BurbujaGrabacion: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @State private var foto: CGImage?
    @Environment(\.escalaChat) private var escala

    private var nombre: String {
        let n = mensaje.nombre.trimmingCharacters(in: .whitespaces)
        return n.isEmpty ? TextosCarpeta.grabacion : n
    }

    var body: some View {
        let g = grabacionDeCarpeta(texto: mensaje.texto)
        let ancho = ladoDeBurbuja * escala
        VStack(alignment: .leading, spacing: 6 * escala) {
            HStack(spacing: 8 * escala) {
                Image(systemName: "mic")
                    .font(.system(size: 18 * escala))
                    .foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 1) {
                    Text(nombre).font(.system(size: 14 * escala, weight: .semibold)).lineLimit(2)
                    Text(g?.detalle ?? "").font(.caption).opacity(0.7)
                }
            }
            ZStack {
                Color.white.opacity(0.55)
                if let foto {
                    Image(decorative: foto, scale: 2).resizable().interpolation(.high).scaledToFill()
                } else {
                    Image(systemName: (g?.video.isEmpty ?? true) ? "mic" : "video")
                        .font(.system(size: 44 * escala))
                        .foregroundStyle(Color(white: 0.45))
                }
            }
            .frame(width: ancho, height: ancho * 0.5625)
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .overlay(alignment: .topLeading) {
                HStack(spacing: 5 * escala) {
                    Circle().fill(Color(red: 0xE5 / 255, green: 0x39 / 255, blue: 0x35 / 255))
                        .frame(width: 8 * escala, height: 8 * escala)
                    Text(TextosCarpeta.grabacion).font(.system(size: 11 * escala, weight: .medium))
                }
                .foregroundStyle(.white)
                .padding(.horizontal, 7 * escala)
                .padding(.vertical, 3 * escala)
                .background(Color.black.opacity(0.65), in: Capsule())
                .padding(6 * escala)
            }
            .overlay(alignment: .bottomLeading) {
                if let g {
                    Text(g.detalle)
                        .font(.system(size: 11 * escala, weight: .medium))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 7 * escala)
                        .padding(.vertical, 3 * escala)
                        .background(Color.black.opacity(0.65), in: Capsule())
                        .padding(6 * escala)
                }
            }
            .overlay(alignment: .bottomTrailing) {
                // «Estudiar», si hay nota que estudiar.
                if let g, !g.voz.isEmpty {
                    Label(TextosCarpeta.estudiar, systemImage: "book")
                        .font(.system(size: 12.5 * escala, weight: .medium))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 10 * escala)
                        .padding(.vertical, 5 * escala)
                        .background(Color.accentColor, in: Capsule())
                        .padding(8 * escala)
                }
            }
        }
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityHint("Estudiar la clase")
        .task(id: mensaje.texto) {
            // La foto de la tarjeta: la primera captura que este en el chat.
            let capturas = Set(g?.capturas ?? [])
            let paginas = await CarpetasDelChat.leer(mensaje.id, chat: chat, almacen: almacen)?.paginas ?? []
            guard let m = paginas.first(where: { capturas.contains($0.mensaje.id) && $0.mensaje.clase == "IMAGEN" })?
                .mensaje,
                let url = await resolverFichero(m, chat, almacen) else {
                foto = nil
                return
            }
            foto = await Miniaturas.compartidas.cargar(url, lado: Int(ladoDeBurbuja * 2 * escala))
        }
    }
}

/// La miniatura de una pagina: un lienzo dibujado, una foto, o su icono.
struct MiniaturaDePagina: View {
    let pagina: MensajeChat?
    let chat: String
    let almacen: Almacen
    let lado: CGFloat
    @State private var dibujo: CGImage?
    @State private var foto: URL?
    @Environment(\.displayScale) private var escalaPantalla

    var body: some View {
        ZStack {
            Color.white
            if let dibujo {
                Image(decorative: dibujo, scale: max(1, escalaPantalla)).resizable().scaledToFit()
            } else if let foto {
                MiniaturaVista(url: foto, lado: lado).scaledToFill()
            } else {
                Image(systemName: CarpetasChat.icono(pagina))
                    .font(.system(size: lado * 0.32))
                    .foregroundStyle(Color(hex: 0x8A8A8E))
            }
        }
        .frame(width: lado, height: lado * 1.3)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .overlay { RoundedRectangle(cornerRadius: 6).strokeBorder(.separator.opacity(0.5)) }
        .task(id: "\(pagina?.id ?? "")#\(lado)") { await leer() }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinLienzoGuardado)) { aviso in
            guard let r = aviso.object as? String, r == pagina?.referencia, let m = pagina else { return }
            MiniaturasLienzo.compartidas.olvidar(chat: chat, referencia: r)
            _ = Task<Void, Never> { await leerDibujo(m) }
        }
    }

    private func leer() async {
        guard let m = pagina else { return }
        switch m.clase {
        case "DIBUJO":
            await leerDibujo(m)
        case "IMAGEN":
            foto = await resolverFichero(m, chat, almacen)
        default:
            break
        }
    }

    private func leerDibujo(_ m: MensajeChat) async {
        guard let r = m.referencia, !r.isEmpty else { return }
        let px = CGSize(width: lado * max(1, escalaPantalla), height: lado * 1.3 * max(1, escalaPantalla))
        dibujo = await MiniaturasLienzo.compartidas.imagen(chat: chat, referencia: r, pixeles: px, almacen: almacen)
    }
}

// MARK: - La ventana de la carpeta

/// Lo que se ve de una carpeta abierta, leido del disco por el nucleo
/// (`carpetaDePaginas`: sus paginas vivas, en orden, con su mensaje).
@MainActor
final class ModeloCarpeta: ObservableObject {
    let chat: String
    let id: String
    let almacen: Almacen
    @Published private(set) var nombre = ""
    @Published private(set) var paginas: [PaginaDeCarpeta] = []
    /// La carpeta leida entera (para su PDF).
    private(set) var carpeta: CarpetaDePaginas?
    @Published private(set) var cargada = false
    @Published private(set) var noEsta = false
    @Published var aviso: String?
    @Published var trabajando = false
    private var turnoAviso = 0

    init(chat: String, id: String, almacen: Almacen) {
        self.chat = chat
        self.id = id
        self.almacen = almacen
    }

    /// Las paginas que siguen en el chat (el nucleo ya salta lo borrado).
    var vivas: [PaginaDeCarpeta] { paginas }

    /// Si es la carpeta de una grabacion: lo grabado no se reordena (es el
    /// orden del tiempo) y se estudia en su minuto.
    var esGrabacion: Bool { carpeta?.grabacion ?? false }
    /// La nota de voz de la grabacion (vacio si no entro).
    var voz: String { carpeta?.voz ?? "" }

    func recargar() async {
        let (nucleo, chat, id) = (almacen.nucleo, chat, id)
        let c = await Task.detached(priority: .userInitiated) {
            (try? nucleo.carpetaDePaginas(chat: chat, id: id)) ?? nil
        }.value
        cargada = true
        carpeta = c
        guard let c else {
            noEsta = true
            paginas = []
            return
        }
        noEsta = false
        nombre = c.nombre.isEmpty ? CarpetasChat.nombreDeSerie : c.nombre
        paginas = c.paginas
    }

    func avisar(_ texto: String) {
        aviso = texto
        turnoAviso += 1
        let turno = turnoAviso
        _ = Task<Void, Never> { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 2_600_000_000)
            if self?.turnoAviso == turno { self?.aviso = nil }
        }
    }

    private func aplicar(_ cambio: CambioCarpeta) async {
        if await AccionesCarpeta.cambiar(id, en: chat, cambio, almacen: almacen) == nil {
            avisar("No se pudo")
        }
        await recargar()
    }

    /// Lleva la pagina `pagina` al puesto `puesto` (entre las vivas). En una
    /// grabacion no: su orden es el del tiempo.
    func mover(_ pagina: String, a puesto: Int) async {
        guard !esGrabacion, let i = paginas.firstIndex(where: { $0.mensaje.id == pagina }) else { return }
        let hasta = min(max(0, puesto), max(0, paginas.count - 1))
        guard hasta != i else { return }
        await aplicar(.mover(desde: UInt32(i), hasta: UInt32(hasta)))
    }

    /// Un puesto antes (`paso` -1) o despues (+1), entre las vivas.
    func correr(_ pagina: String, _ paso: Int) async {
        guard let i = paginas.firstIndex(where: { $0.mensaje.id == pagina }) else { return }
        let destino = i + paso
        guard destino >= 0, destino < paginas.count else { return }
        await mover(pagina, a: destino)
    }

    func quitar(_ pagina: String) async { await aplicar(.quitar(pagina: pagina)) }

    func renombrar(_ pagina: String, _ titulo: String) async {
        await aplicar(.renombrar(pagina: pagina, titulo: titulo))
    }

    /// **«Nueva página»**: el lienzo nuevo, ya en el chat y en la carpeta.
    func nuevaPagina() async -> MensajeChat? {
        trabajando = true
        defer { trabajando = false }
        let (nucleo, chat, id) = (almacen.nucleo, chat, id)
        let r = await Task.detached(priority: .userInitiated) { () -> Result<PaginaNuevaDeCarpeta, Error> in
            Result { try nucleo.paginaSiguienteDeCarpeta(chat: chat, carpeta: id) }
        }.value
        guard case let .success(n) = r else {
            avisar("No se pudo")
            return nil
        }
        await almacen.alFinal(n.lienzo, en: chat)
        almacen.sustituir(n.carpeta, en: chat)
        await recargar()
        return n.lienzo
    }

    /// Las paginas, en orden, para la hoja de compartir.
    var mensajesEnOrden: [MensajeChat] { paginas.map(\.mensaje) }

    /// **«Estudiar»**: la clase de esta grabacion en su pantalla.
    func estudiar(ms: Int64 = 0) {
        guard !voz.isEmpty else { return }
        EstudiarClase.abrir(chat: chat, mensaje: voz, ms: ms, almacen: almacen)
    }

    /// **«Adjuntar a la clase»** (`Nucleo.adjuntarALaClase`, como el Mac):
    /// los ficheros elegidos entran en el chat (como si se soltaran en el) y
    /// en la carpeta como adjuntos, aparte de lo grabado. No salen sueltos en
    /// el chat: estan en la carpeta. Los de Archivos se leen con su permiso
    /// (`startAccessingSecurityScopedResource`) mientras el nucleo los copia.
    func adjuntar(_ urls: [URL]) async {
        guard !urls.isEmpty else { return }
        trabajando = true
        defer { trabajando = false }
        let (nucleo, chat, id) = (almacen.nucleo, chat, id)
        let hechos = await Task.detached(priority: .userInitiated) { () -> [AdjuntadoALaClase] in
            urls.compactMap { url in
                let permiso = url.startAccessingSecurityScopedResource()
                defer { if permiso { url.stopAccessingSecurityScopedResource() } }
                return (try? nucleo.adjuntarALaClase(chat: chat, carpeta: id, ruta: url.path)) ?? nil
            }
        }.value
        if let ultimo = hechos.last {
            if almacen.mensajes[chat] != nil { almacen.anadir(hechos.map(\.mensaje), en: chat) }
            almacen.sustituir(ultimo.carpeta, en: chat)
            avisar(TextosCarpeta.adjuntado(hechos.map(\.nombre).joined(separator: ", ")))
        } else {
            avisar(TextosCarpeta.noSePudo)
        }
        await recargar()
    }
}

/// **La ventana de una carpeta** (`carpetas::abrir` de Windows).
struct VistaCarpeta: View {
    @StateObject private var modelo: ModeloCarpeta
    let cerrar: () -> Void
    @State private var renombrando: PaginaDeCarpeta?
    @State private var titulo = ""
    @State private var arrastrada: String?
    @State private var compartiendo: CosasACompartir?
    @State private var exportando = false
    @State private var eligiendoAdjuntos = false

    init(chat: String, carpeta: String, almacen: Almacen, cerrar: @escaping () -> Void) {
        _modelo = StateObject(wrappedValue: ModeloCarpeta(chat: chat, id: carpeta, almacen: almacen))
        self.cerrar = cerrar
    }

    /// **Abre la carpeta** encima de todo.
    @MainActor
    static func abrir(chat: String, carpeta: String, almacen: Almacen) {
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 900, height: 720)) { cerrar in
            VistaCarpeta(chat: chat, carpeta: carpeta, almacen: almacen, cerrar: cerrar)
        }
    }

    var body: some View {
        NavigationStack {
            contenido
                .navigationTitle(modelo.nombre.isEmpty ? "Carpeta de páginas" : modelo.nombre)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cerrar", action: cerrar).keyboardShortcut(.cancelAction)
                    }
                    ToolbarItemGroup(placement: .primaryAction) {
                        Button { exportar() } label: { Label("Exportar PDF", systemImage: "doc.richtext") }
                            .disabled(modelo.vivas.isEmpty || exportando)
                        // En una grabacion, adjuntar y estudiar (si hay nota);
                        // en un cuaderno, una pagina nueva.
                        if modelo.esGrabacion {
                            Button { eligiendoAdjuntos = true } label: {
                                Label(TextosCarpeta.adjuntar, systemImage: "paperclip")
                            }
                            .disabled(modelo.noEsta || modelo.trabajando)
                            if !modelo.voz.isEmpty {
                                Button { modelo.estudiar() } label: {
                                    Label(TextosCarpeta.estudiar, systemImage: "book")
                                }
                                .buttonStyle(.borderedProminent)
                            }
                        } else {
                            Button { nuevaPagina() } label: { Label("Nueva página", systemImage: "plus") }
                                .disabled(modelo.noEsta || modelo.trabajando)
                                .keyboardShortcut("n", modifiers: .command)
                        }
                    }
                }
                .overlay(alignment: .bottom) {
                    if let aviso = modelo.aviso {
                        AvisoDelChat(texto: aviso).padding(.bottom, 24)
                    }
                }
                .animation(.easeOut(duration: 0.15), value: modelo.aviso)
        }
        .task { await modelo.recargar() }
        .fileImporter(isPresented: $eligiendoAdjuntos, allowedContentTypes: [.item],
                      allowsMultipleSelection: true) { r in
            guard case let .success(urls) = r else { return }
            _ = Task<Void, Never> { await modelo.adjuntar(urls) }
        }
        .sheet(item: $compartiendo) { c in
            ActividadCompartir(cosas: c.cosas).ignoresSafeArea()
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            _ = Task<Void, Never> { await modelo.recargar() }
        }
        .alert("Renombrar", isPresented: Binding(get: { renombrando != nil }, set: { if !$0 { renombrando = nil } })) {
            TextField("Título", text: $titulo)
            Button("Guardar") {
                if let p = renombrando {
                    let t = titulo
                    _ = Task<Void, Never> { await modelo.renombrar(p.mensaje.id, t) }
                }
                renombrando = nil
            }
            Button("Cancelar", role: .cancel) { renombrando = nil }
        } message: {
            Text("Escribe el título de la página")
        }
    }

    @ViewBuilder private var contenido: some View {
        if !modelo.cargada {
            ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if modelo.noEsta {
            aviso("Esa carpeta ya no está en el chat")
        } else if modelo.vivas.isEmpty {
            aviso("Esta carpeta está vacía. Pulsa «Nueva página», o en el chat elige «Añadir a carpeta…» en el menú de un lienzo o de una imagen.")
        } else {
            VStack(spacing: 0) {
                HStack {
                    Text(modelo.esGrabacion ? (modelo.carpeta?.detalle ?? "") : CarpetasChat.cuantas(modelo.vivas.count))
                        .font(.subheadline.weight(.medium))
                    Spacer()
                    Text(modelo.esGrabacion ? TextosCarpeta.grabacionPista
                        : "Mantén pulsada una página para moverla, renombrarla o quitarla (vuelve a verse en el chat)")
                        .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                .padding(.horizontal, 20).padding(.vertical, 8)
                rejilla
            }
        }
    }

    private func aviso(_ texto: String) -> some View {
        VStack(spacing: 14) {
            Image(systemName: "book.closed").font(.system(size: 40)).foregroundStyle(.secondary)
            Text(texto).multilineTextAlignment(.center).foregroundStyle(.secondary).frame(maxWidth: 420)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(24)
    }

    private var rejilla: some View {
        let vivas = modelo.vivas
        return ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 170, maximum: 220), spacing: 18)], spacing: 18) {
                ForEach(Array(vivas.enumerated()), id: \.element.mensaje.id) { i, p in
                    celda(p, numero: i + 1, total: vivas.count)
                }
            }
            .padding(20)
        }
    }

    private func celda(_ p: PaginaDeCarpeta, numero: Int, total: Int) -> some View {
        let m: MensajeChat? = p.mensaje
        let pid = p.mensaje.id
        // En una grabacion no se cambia el orden: es el del tiempo.
        let fijo = modelo.esGrabacion
        // En su esquina: el numero de la pagina, o su minuto en la grabacion.
        let esquina = p.esquina.isEmpty && !fijo ? "\(numero)" : p.esquina
        return VStack(alignment: .leading, spacing: 6) {
            MiniaturaDePagina(pagina: m, chat: modelo.chat, almacen: modelo.almacen, lado: 170)
                // Lo adjuntado a una clase, con otro color: no es de lo grabado.
                .overlay {
                    if p.adjunto {
                        RoundedRectangle(cornerRadius: 6).fill(colorAdjunto.opacity(0.18))
                        RoundedRectangle(cornerRadius: 6).strokeBorder(colorAdjunto, lineWidth: 2)
                    }
                }
                .overlay(alignment: .topLeading) {
                    if !esquina.isEmpty {
                        Text(esquina)
                            .font(.system(size: 12, weight: .bold).monospacedDigit())
                            .padding(.horizontal, 7).padding(.vertical, 2)
                            .background(Paleta.chat.enviar, in: Capsule())
                            .foregroundStyle(.white)
                            .padding(6)
                    }
                }
                // «Adjunto · no es de la grabación», abajo, sobre su color.
                .overlay(alignment: .bottomLeading) {
                    if p.adjunto {
                        Text(TextosCarpeta.adjuntoInsignia)
                            .font(.system(size: 10.5, weight: .medium))
                            .foregroundStyle(.white)
                            .lineLimit(1)
                            .minimumScaleFactor(0.8)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 3)
                            .background(colorAdjunto, in: Capsule())
                            .padding(6)
                    }
                }
                .opacity(arrastrada == pid ? 0.4 : 1)
            Text(CarpetasChat.titulo(p))
                .font(.system(size: 13, weight: .medium)).lineLimit(1)
        }
        .contentShape(Rectangle())
        .onTapGesture { abrir(p) }
        .contextMenu {
            Button { abrir(p) } label: { Label("Abrir", systemImage: "arrow.up.right.square") }
            Button {
                titulo = CarpetasChat.titulo(p)
                renombrando = p
            } label: { Label("Renombrar", systemImage: "pencil") }
            if numero > 1, !fijo {
                Button { _ = Task<Void, Never> { await modelo.correr(pid, -1) } } label: {
                    Label("Antes", systemImage: "arrow.left")
                }
            }
            if numero < total, !fijo {
                Button { _ = Task<Void, Never> { await modelo.correr(pid, 1) } } label: {
                    Label("Después", systemImage: "arrow.right")
                }
            }
            Button(role: .destructive) { _ = Task<Void, Never> { await modelo.quitar(pid) } } label: {
                Label("Quitar de la carpeta", systemImage: "minus.circle")
            }
        }
        // Arrastrar una pagina encima de otra la lleva a su sitio.
        .onDrag {
            arrastrada = pid
            return NSItemProvider(object: pid as NSString)
        }
        .onDrop(of: [UTType.plainText], delegate: SoltarPagina(destino: pid, modelo: modelo,
                                                                arrastrada: $arrastrada))
    }

    /// Abre una pagina; lo de una grabacion se estudia en su minuto, con el
    /// texto al lado (encima de la carpeta, que sigue abierta debajo).
    private func abrir(_ p: PaginaDeCarpeta) {
        if modelo.esGrabacion, !p.adjunto, !modelo.voz.isEmpty {
            return modelo.estudiar(ms: p.ms ?? 0)
        }
        abrir(p.mensaje)
    }

    private func abrir(_ m: MensajeChat?) {
        guard let m else { return }
        let (chat, almacen) = (modelo.chat, modelo.almacen)
        let nav = Navegacion.compartida
        switch m.clase {
        case "DIBUJO" where m.referencia != nil:
            cerrar()
            nav.abrir(.lienzo(chat: chat, mensaje: m, foto: nil))
        case "IMAGEN":
            _ = Task<Void, Never> {
                guard let url = await resolverFichero(m, chat, almacen) else { return }
                cerrar()
                nav.abrir(.lienzo(chat: chat, mensaje: m, foto: url))
            }
        default:
            // Lo demas, en el chat, en su sitio (como Windows).
            cerrar()
            nav.irAlChat(chat, mensaje: m.id)
        }
    }

    private func nuevaPagina() {
        _ = Task<Void, Never> {
            guard let lienzo = await modelo.nuevaPagina() else { return }
            cerrar()
            Navegacion.compartida.abrir(.lienzo(chat: modelo.chat, mensaje: lienzo, foto: nil))
        }
    }

    /// **«Exportar PDF»** (Windows y el Mac, `PdfDeCarpeta`): una pagina
    /// por elemento y, de cada lienzo, lo que va sobre su plantilla o sus
    /// marcos; con el titulo de cada pagina en el indice. Sale en la hoja de
    /// compartir (Archivos, AirDrop, imprimir).
    private func exportar() {
        guard let carpeta = modelo.carpeta, !modelo.vivas.isEmpty else {
            modelo.avisar(PdfDeCarpeta.noSeExporto)
            return
        }
        exportando = true
        modelo.avisar("Exportando el PDF…")
        _ = Task<Void, Never> {
            let url = await PdfDeCarpeta.exportar(carpeta, chat: modelo.chat, almacen: modelo.almacen)
            exportando = false
            guard let url else {
                modelo.avisar(PdfDeCarpeta.noSeExporto)
                return
            }
            compartiendo = CosasACompartir(cosas: [url])
        }
    }
}

/// Soltar una pagina arrastrada encima de otra: la pone en su puesto.
private struct SoltarPagina: DropDelegate {
    let destino: String
    let modelo: ModeloCarpeta
    @Binding var arrastrada: String?

    func performDrop(info: DropInfo) -> Bool {
        let movida = arrastrada
        arrastrada = nil
        guard let movida, movida != destino else { return false }
        let modelo = self.modelo
        let destino = self.destino
        _ = Task<Void, Never> { @MainActor in
            guard let j = modelo.paginas.firstIndex(where: { $0.mensaje.id == destino }) else { return }
            await modelo.mover(movida, a: j)
        }
        return true
    }

    func dropUpdated(info: DropInfo) -> DropProposal? { DropProposal(operation: .move) }
}
