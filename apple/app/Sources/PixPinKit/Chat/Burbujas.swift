// Las burbujas del chat, una por clase de mensaje, como en Windows y el Mac
// (`Vistas.swift`, `Chat/Adjuntos.swift` y `Chat/BurbujasQueLlegan.swift`
// del Mac):
//
// - `NOTA` (con su formato y lo buscado resaltado), la hoja nota del
//   proyecto y la del asistente (del otro color, como Windows);
// - `TABLA` y la mini-app tabla, la lista de tareas (`MINIAPP` tareas);
// - `IMAGEN` por su miniatura, `ARCHIVO` con el cuadro de color de su tipo,
//   `VOZ` con su onda (`Voz/VistasVoz.swift`), `DIBUJO` con la miniatura del
//   lienzo (`Lienzo/MiniaturaLienzo`);
// - lo que llega de Windows y el movil: `PAGINA` (la hoja del PDF),
//   `PROYECTO` (la puerta a ese proyecto) y `CROQUIS` (el croquis 3D).
//
// Debajo de cada una, el pie: de donde viene, de quien llego, el codigo
// (`#1·K7Q2`), la hora, las marcas (fijado, recordatorio, etiqueta) y el
// punto de proyecto.
//
// Cada burbuja resuelve su fichero UNA vez y pide su miniatura a la cache
// compartida: ni decodifica la foto entera ni toca el disco en el hilo
// principal. Lo que hace un toque (abrir) y el menu lo pone `VistaChat`.

import PDFKit
import PixPinCore
import SwiftUI
import UIKit

struct BurbujaChat: View {
    let mensaje: MensajeChat
    let chat: String
    /// Sin observar: una burbuja no se repinta por cada cambio del almacen.
    let almacen: Almacen
    /// Aquel al que responde (si lo hace y esta cargado) y como ir a el.
    var citado: MensajeChat?
    var irACita: () -> Void = {}
    /// Lo buscado en el chat, para resaltarlo en las notas.
    @Environment(\.agujaDelChat) private var aguja
    @Environment(\.escalaChat) private var escala

    private var delAsistente: Bool { MenuMensaje.esDelAsistente(mensaje) }

    var body: some View {
        VStack(alignment: .leading, spacing: 4 * escala) {
            if let r = mensaje.respondeA, !r.isEmpty {
                CitaDeMensaje(citado: citado, irA: irACita)
            }
            contenido
            PieDelMovil(mensaje: mensaje, chat: chat)
            pie
        }
        .foregroundStyle(delAsistente ? Paleta.proyectos.texto : Paleta.chat.textoMio)
        .padding(.horizontal, 10 * escala)
        .padding(.vertical, 7 * escala)
        .background(delAsistente ? Paleta.chat.burbujaOtra : Paleta.chat.burbujaMia,
                    in: RoundedRectangle(cornerRadius: 17 * escala, style: .continuous))
        .contentShape(.contextMenuPreview, RoundedRectangle(cornerRadius: 17 * escala, style: .continuous))
    }

    @ViewBuilder private var contenido: some View {
        switch mensaje.clase {
        case "NOTA" where mensaje.esDelAsistente:
            NotaDelAsistente(mensaje: mensaje, chat: chat, almacen: almacen)
        case "NOTA" where BurbujaHojaNota.esHoja(mensaje):
            BurbujaHojaNota(mensaje: mensaje)
        case "NOTA":
            Text(textoResaltado(mensaje.texto, aguja: aguja))
                .tint(Paleta.chat.enviar)
                .fixedSize(horizontal: false, vertical: true)
        // «TABLA» siempre es tabla; una «MINIAPP» solo si lo que trae lo es.
        case "TABLA":
            BurbujaTablaChat(mensaje: mensaje)
        case "MINIAPP" where BurbujaTareas.esTareas(mensaje):
            BurbujaTareas(mensaje: mensaje)
        case "MINIAPP" where BurbujaTablaChat.esTabla(mensaje):
            BurbujaTablaChat(mensaje: mensaje)
        case "IMAGEN" where mensaje.ruta != nil:
            BurbujaImagen(mensaje: mensaje, chat: chat, almacen: almacen)
        case "VOZ" where mensaje.ruta != nil:
            BurbujaVoz(mensaje: mensaje, chat: chat, almacen: almacen)
        case "ARCHIVO" where mensaje.ruta != nil:
            BurbujaArchivo(mensaje: mensaje, chat: chat, almacen: almacen)
        case "DIBUJO" where mensaje.referencia != nil:
            BurbujaDibujo(mensaje: mensaje, chat: chat, almacen: almacen)
        case "PAGINA":
            BurbujaPagina(mensaje: mensaje, chat: chat, almacen: almacen)
        case "PROYECTO":
            BurbujaProyecto(mensaje: mensaje, almacen: almacen)
        case "CROQUIS":
            BurbujaCroquis(mensaje: mensaje, chat: chat, almacen: almacen)
        default:
            Label(mensaje.resumen.isEmpty ? mensaje.nombre : mensaje.resumen,
                  systemImage: BurbujaChat.icono(mensaje.clase))
        }
    }

    /// El codigo, la hora, las marcas y el punto (abajo a la derecha, como
    /// Windows).
    private var pie: some View {
        HStack(spacing: 6 * escala) {
            if let punto = mensaje.punto { PuntoDeProyecto(verde: punto) }
            Spacer(minLength: 0)
            MarcasDeMensaje(mensaje: mensaje)
            if let codigo = mensaje.codigoChat, !codigo.isEmpty {
                Text("#\(codigo)")
                    .font(.system(size: 9 * escala, weight: .medium))
                    .padding(.horizontal, 4 * escala)
                    .padding(.vertical, 1)
                    .background(Color.primary.opacity(0.08), in: RoundedRectangle(cornerRadius: 4))
            }
            Text(Date(timeIntervalSince1970: TimeInterval(mensaje.cuando) / 1000),
                 format: .dateTime.hour().minute())
                .font(.system(size: 11 * escala))
        }
        .foregroundStyle(delAsistente ? Paleta.proyectos.apagado : Paleta.chat.horaMia)
    }

    static func icono(_ clase: String) -> String {
        switch clase {
        case "IMAGEN": "photo"
        case "VOZ": "waveform"
        case "DIBUJO": "scribble.variable"
        case "PAGINA": "doc.richtext"
        case "MINIAPP": "checklist"
        case "PROYECTO": "folder"
        case "CROQUIS": "cube.transparent"
        case "TABLA": "tablecells"
        default: "doc"
        }
    }
}

// MARK: - Foto

struct BurbujaImagen: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @State private var url: URL?
    /// La escala del chat: la foto crece con la letra.
    @Environment(\.escalaChat) private var escala

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            // Con lo dibujado encima, si la foto esta anotada (en el movil,
            // en Windows o aqui): `Chat/FotoAnotadaChat.swift`.
            FotoConLoAnotado(mensaje: mensaje, chat: chat, almacen: almacen, lado: ladoDeBurbuja * escala) {
                if let url {
                    MiniaturaVista(url: url, lado: ladoDeBurbuja * escala)
                } else {
                    Label(mensaje.nombre, systemImage: "photo")
                }
            }
            // El pie de la foto (el `texto` de un IMAGEN, como en el movil).
            if !mensaje.texto.isEmpty {
                Text(mensaje.texto)
            }
        }
        .task(id: mensaje.id) { url = await resolverFichero(mensaje, chat, almacen) }
    }
}

// MARK: - Archivo

struct BurbujaArchivo: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @State private var esta = true
    @Environment(\.escalaChat) private var escala

    private var nombre: String { mensaje.nombre.isEmpty ? mensaje.resumen : mensaje.nombre }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 10 * escala) {
                let ext = ((mensaje.nombre.isEmpty ? (mensaje.ruta ?? "") : mensaje.nombre) as NSString).pathExtension
                CuadroExtensionChat(ext: ext, lado: 40 * escala)
                VStack(alignment: .leading, spacing: 1) {
                    Text(nombre)
                        .lineLimit(2).truncationMode(.middle)
                    Group {
                        if !esta {
                            Text("No está en este iPad")
                        } else if mensaje.bytes > 0 {
                            Text(ByteCountFormatter.string(fromByteCount: mensaje.bytes, countStyle: .file))
                        }
                    }
                    .font(.caption).opacity(0.7)
                }
            }
            .frame(maxWidth: 320, alignment: .leading)
            // El pie, o lo que se dijo en una nota de voz.
            if !mensaje.texto.isEmpty {
                Text(mensaje.texto)
            } else if mensaje.resumen != mensaje.nombre && !mensaje.resumen.isEmpty {
                Text(mensaje.resumen).opacity(0.7)
            }
        }
        .task(id: mensaje.id) {
            guard let u = await resolverFichero(mensaje, chat, almacen) else {
                esta = false
                return
            }
            let ruta = u.path
            esta = await Task.detached(priority: .utility) { FileManager.default.fileExists(atPath: ruta) }.value
        }
    }
}

/// **El cuadro del tipo de archivo** con su extension dentro, del color de
/// Windows y del movil (`pixpin_ui::color_de_extension`).
struct CuadroExtensionChat: View {
    let ext: String
    var lado: CGFloat = 34

    private static let tabla: [(UInt32, Bool, [String])] = [
        (0xE5484D, false, ["pdf"]),
        (0x2F7BF5, false, ["doc", "docx", "docm", "dot", "dotx", "odt", "rtf", "pages", "wps"]),
        (0x2E9E4F, false, ["xls", "xlsx", "xlsm", "xlsb", "csv", "tsv", "ods", "numbers"]),
        (0xF2711C, false, ["ppt", "pptx", "pptm", "pps", "ppsx", "pot", "potx", "odp", "key"]),
        (0xF5B800, true, ["dwg", "dxf", "dwf", "dwfx", "dgn", "dwt", "plt", "rvt", "rfa", "ifc", "nwd", "nwc"]),
        (0x0FA3A3, false, ["obj", "stl", "fbx", "3ds", "blend", "skp", "gltf", "glb", "dae", "ply", "step", "stp",
                           "iges", "igs", "3dm", "usdz"]),
        (0xD6336C, false, ["jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "bmp", "tif", "tiff", "svg",
                           "psd", "ai", "raw", "dng", "avif", "ico"]),
        (0x8E44AD, false, ["mp3", "m4a", "aac", "wav", "ogg", "oga", "opus", "flac", "amr", "wma", "mid", "midi"]),
        (0x5B3CC4, false, ["mp4", "mov", "avi", "mkv", "webm", "3gp", "m4v", "wmv", "flv"]),
        (0x8D6E63, false, ["zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst"]),
        (0x1098AD, false, ["html", "htm", "css", "js", "ts", "json", "xml", "kt", "kts", "java", "py", "c", "h",
                           "cpp", "hpp", "cs", "rs", "go", "sh", "sql", "yml", "yaml", "tex", "m", "ipynb"]),
        (0x607D8B, false, ["txt", "md", "log", "ini", "cfg", "srt", "vtt"]),
        (0x3B5BDB, false, ["epub", "mobi", "azw", "azw3", "fb2", "djvu", "cbz", "cbr"]),
        (0x6CBF5B, false, ["apk", "aab", "xapk", "apks", "exe", "msi", "dmg", "deb", "appimage"]),
        (0x7048E8, false, ["pixpin", "excalidraw"]),
    ]
    private static let resto: [UInt32] = [0x5C7CFA, 0x20C997, 0xFF6B6B, 0xFAB005, 0x845EF7, 0x15AABF, 0xE64980, 0x82C91E]

    /// El color y si el texto va oscuro.
    static func familia(_ ext: String) -> (color: UInt32, oscuro: Bool) {
        let e = ext.trimmingCharacters(in: CharacterSet(charactersIn: ".")).lowercased()
        guard !e.isEmpty, e.utf16.count <= 10 else { return (0x868E96, false) }
        if let f = tabla.first(where: { $0.2.contains(e) }) { return (f.0, f.1) }
        let c = resto[e.utf16.reduce(0) { $0 + Int($1) } % resto.count]
        return (c, c == 0xFAB005)
    }

    var body: some View {
        let f = Self.familia(ext)
        RoundedRectangle(cornerRadius: lado * 0.26)
            .fill(Color(hex: f.color))
            .frame(width: lado, height: lado)
            .overlay {
                Text(String(ext.lowercased().prefix(4)))
                    .font(.system(size: lado * 0.34, weight: .semibold))
                    .minimumScaleFactor(0.5)
                    .lineLimit(1)
                    .padding(.horizontal, 2)
                    .foregroundStyle(f.oscuro ? Color(hex: 0x3A2B00) : .white)
            }
    }
}

// MARK: - Dibujo

struct BurbujaDibujo: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            MiniaturaLienzo(almacen: almacen, chat: chat, mensaje: mensaje)
                .clipShape(RoundedRectangle(cornerRadius: 8))
            if !mensaje.nombre.isEmpty {
                Label(mensaje.nombre, systemImage: "scribble.variable")
                    .font(.system(size: 13, weight: .medium))
            }
            if !mensaje.texto.isEmpty {
                Text(mensaje.texto)
            }
        }
    }
}

// MARK: - Tabla y tareas

/// Una tabla (`TABLA` del movil o la mini-app `tabla` del PC): una ficha
/// que abre la hoja de calculo.
struct BurbujaTablaChat: View {
    let mensaje: MensajeChat
    @Environment(\.escalaChat) private var escala

    /// Si un mensaje es una tabla: `TABLA` del movil o la mini-app `tabla`
    /// del PC (su texto es el JSON de `TablaDeCalculo`, con `celdas`).
    static func esTabla(_ m: MensajeChat) -> Bool {
        if m.clase == "TABLA" { return true }
        guard m.clase == "MINIAPP" else { return false }
        if m.miniapp == "tabla" { return true }
        let t = m.texto.trimmingCharacters(in: .whitespacesAndNewlines)
        return t.hasPrefix("{") && t.contains("\"celdas\"")
    }

    var body: some View {
        HStack(spacing: 10 * escala) {
            Image(systemName: "tablecells.fill")
                .font(.system(size: 22 * escala))
                .foregroundStyle(.white)
                .frame(width: 40 * escala, height: 40 * escala)
                .background(Color(hex: 0x4B49C8), in: RoundedRectangle(cornerRadius: 10))
            VStack(alignment: .leading, spacing: 1) {
                Text(mensaje.nombre.isEmpty ? "Tabla" : mensaje.nombre)
                    .font(.system(size: 14 * escala, weight: .semibold)).lineLimit(1)
                Text("Hoja de cálculo").font(.caption).opacity(0.7)
            }
        }
        .frame(maxWidth: 320, alignment: .leading)
    }
}

/// Una lista de tareas del movil o de Windows: una ficha con su nombre y lo
/// hecho («3 de 7»), como `fila_de_fichero` de Windows.
struct BurbujaTareas: View {
    let mensaje: MensajeChat
    @Environment(\.escalaChat) private var escala

    static func esTareas(_ m: MensajeChat) -> Bool { m.miniapp == "tareas" }

    /// Las tareas del texto (`- [ ]` / `- [x]`): hechas y total.
    static func cuenta(_ texto: String) -> (hechas: Int, total: Int) {
        var h = 0, t = 0
        for l in texto.split(separator: "\n") {
            let s = l.trimmingCharacters(in: .whitespaces).lowercased()
            if s.hasPrefix("- [ ]") || s.hasPrefix("* [ ]") {
                t += 1
            } else if s.hasPrefix("- [x]") || s.hasPrefix("* [x]") {
                t += 1
                h += 1
            }
        }
        return (h, t)
    }

    static func nombre(_ m: MensajeChat) -> String {
        if !m.nombre.isEmpty { return m.nombre }
        let primera = m.texto.split(separator: "\n").first.map(String.init) ?? ""
        let limpia = primera.trimmingCharacters(in: CharacterSet(charactersIn: "# ").union(.whitespaces))
        return limpia.isEmpty ? "Tareas" : limpia
    }

    var body: some View {
        let c = Self.cuenta(mensaje.texto)
        HStack(spacing: 10 * escala) {
            Image(systemName: "checklist")
                .font(.system(size: 22 * escala))
                .foregroundStyle(.white)
                .frame(width: 40 * escala, height: 40 * escala)
                .background(Color(hex: 0x1E7A4A), in: RoundedRectangle(cornerRadius: 10))
            VStack(alignment: .leading, spacing: 2) {
                Text(Self.nombre(mensaje)).font(.system(size: 14 * escala, weight: .semibold))
                Text(c.total == 0 ? "Lista de tareas · vacía" : "Lista de tareas · \(c.hechas) de \(c.total)")
                    .font(.system(size: 12 * escala)).opacity(0.7)
                if c.total > 0 {
                    ProgressView(value: Double(c.hechas), total: Double(c.total))
                        .tint(Paleta.chat.enviar)
                        .frame(width: 140 * escala)
                }
            }
        }
        .padding(.vertical, 2)
    }
}

// MARK: - Hoja nota

/// Una hoja nota del proyecto (una transcripcion de voz, una nota de la
/// galeria): una ficha con su titulo y el principio del texto, sin la
/// sintaxis cruda; pulsarla la abre en el editor de notas, como Windows.
struct BurbujaHojaNota: View {
    let mensaje: MensajeChat
    @Environment(\.escalaChat) private var escala

    static func esHoja(_ m: MensajeChat) -> Bool {
        guard let r = m.referencia, !r.isEmpty else { return false }
        return !MenuMensaje.esDelAsistente(m)
    }

    /// Titulo y cuerpo, sin `# `, sin imagenes ni audios incrustados.
    static func partes(_ m: MensajeChat) -> (titulo: String, cuerpo: String, audio: Bool) {
        var titulo = m.nombre, cuerpo: [String] = [], audio = false
        for l in m.texto.components(separatedBy: "\n") {
            let t = l.trimmingCharacters(in: .whitespaces)
            if t.hasPrefix("![") {
                if t.lowercased().contains("audio") || t.hasSuffix(".m4a)") { audio = true }
                continue
            }
            if t.hasPrefix("#") {
                let h = t.drop { $0 == "#" }.trimmingCharacters(in: .whitespaces)
                if titulo.isEmpty { titulo = h }
                if h == titulo { continue }
            }
            cuerpo.append(l)
        }
        let c = cuerpo.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        return (titulo.isEmpty ? "Nota" : titulo, c, audio)
    }

    var body: some View {
        let p = Self.partes(mensaje)
        HStack(alignment: .top, spacing: 10 * escala) {
            Image(systemName: p.audio ? "waveform.and.mic" : "doc.text")
                .font(.system(size: 22 * escala))
                .foregroundStyle(Paleta.chat.enviar)
                .frame(width: 28 * escala)
            VStack(alignment: .leading, spacing: 3) {
                Text(p.titulo).font(.system(size: 14 * escala, weight: .semibold))
                if !p.cuerpo.isEmpty {
                    Text(textoConFormato(p.cuerpo))
                        .font(.system(size: 13 * escala))
                        .opacity(0.75)
                        .lineLimit(4)
                }
            }
        }
        .frame(maxWidth: 360, alignment: .leading)
    }
}

// MARK: - Hoja-pagina

/// **Una pagina del PDF del proyecto** en el chat, como `HojaDelProyecto`
/// del movil: la hoja (la pagina del PDF o su imagen) y su numero.
struct BurbujaPagina: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @State private var hoja: UIImage?
    @State private var imagen: URL?
    @State private var falta = false
    @Environment(\.escalaChat) private var escala

    private var titulo: String {
        if !mensaje.nombre.isEmpty { return mensaje.nombre }
        return "Página \((mensaje.pagina ?? 0) + 1)"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Group {
                if let hoja {
                    Image(uiImage: hoja).resizable().scaledToFit()
                        .frame(width: ladoDeBurbuja * escala)
                        .background(Color.white)
                } else if let imagen {
                    MiniaturaVista(url: imagen, lado: ladoDeBurbuja * escala)
                } else if falta {
                    Label(titulo, systemImage: "doc.richtext")
                } else {
                    ProgressView().frame(width: ladoDeBurbuja * escala, height: ladoDeBurbuja * 0.7 * escala)
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 8))
            Text(titulo).font(.caption).opacity(0.7)
            if !mensaje.texto.isEmpty, mensaje.texto != mensaje.nombre {
                Text(mensaje.texto)
            }
        }
        .task(id: mensaje.id) { await leer() }
    }

    private func leer() async {
        let nucleo = almacen.nucleo
        let (chat, id) = (chat, mensaje.id)
        let leida = await Task.detached(priority: .utility) { try? nucleo.hojaDePagina(chat: chat, id: id) }.value
        guard let leida = leida ?? nil else {
            falta = true
            return
        }
        if let i = leida.imagen {
            imagen = URL(fileURLWithPath: i)
            return
        }
        guard let pdf = leida.pdf else {
            falta = true
            return
        }
        let pagina = Int(leida.pagina)
        let lado = ladoDeBurbuja * 2
        let pintada = await Task.detached(priority: .utility) { () -> UIImage? in
            guard let doc = PDFDocument(url: URL(fileURLWithPath: pdf)), let p = doc.page(at: pagina) else { return nil }
            return p.thumbnail(of: CGSize(width: lado, height: lado * 1.5), for: .cropBox)
        }.value
        if let pintada { hoja = pintada } else { falta = true }
    }
}

// MARK: - Proyecto entero

/// **Un proyecto entero como acceso directo** (`PROYECTO`): no copia nada,
/// es la puerta a ese proyecto. Pulsarlo lo abre, como en Windows.
struct BurbujaProyecto: View {
    let mensaje: MensajeChat
    let almacen: Almacen
    @Environment(\.escalaChat) private var escala

    static func ficha(_ m: MensajeChat, en almacen: Almacen) -> FichaChat? {
        guard let r = m.referencia, !r.isEmpty else { return nil }
        return almacen.chats.first { $0.id == r }
    }

    var body: some View {
        let ficha = Self.ficha(mensaje, en: almacen)
        HStack(spacing: 10 * escala) {
            Image(systemName: "folder.fill")
                .font(.system(size: 18 * escala))
                .foregroundStyle(Paleta.chat.carpetaIcono)
                .frame(width: 40 * escala, height: 40 * escala)
                .background(ficha == nil ? Color.gray : Paleta.chat.carpeta, in: Circle())
            VStack(alignment: .leading, spacing: 2) {
                Text(ficha?.nombre ?? (mensaje.nombre.isEmpty ? "Proyecto" : mensaje.nombre))
                    .font(.system(size: 14 * escala, weight: .semibold))
                Group {
                    if let ficha {
                        Text(ficha.hojas == 1 ? "Proyecto · 1 hoja" : "Proyecto · \(ficha.hojas) hojas")
                    } else {
                        Text("Este proyecto no está en este iPad")
                    }
                }
                .font(.system(size: 12 * escala)).opacity(0.7)
                if !mensaje.texto.isEmpty {
                    Text(mensaje.texto).font(.system(size: 13 * escala))
                }
            }
        }
        .padding(.vertical, 2)
    }
}

// MARK: - Croquis 3D

/// **Un croquis 3D del movil** (`CROQUIS`): lo dibujado en el espacio. Se
/// abre en el visor 3D.
struct BurbujaCroquis: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @State private var esta: Bool?
    @Environment(\.escalaChat) private var escala

    /// El texto del mensaje (su primera linea) o el codigo, como Windows.
    static func titulo(_ m: MensajeChat) -> String {
        let fuente = m.texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? m.nombre : m.texto
        let linea = fuente.split(separator: "\n", omittingEmptySubsequences: true).first
            .map { $0.trimmingCharacters(in: .whitespaces) } ?? ""
        return linea.isEmpty ? "Croquis \(m.referencia ?? "")" : linea
    }

    var body: some View {
        HStack(spacing: 10 * escala) {
            Image(systemName: "cube.transparent")
                .font(.system(size: 26 * escala))
                .foregroundStyle(esta == false ? Color.secondary : Paleta.chat.enviar)
            VStack(alignment: .leading, spacing: 2) {
                Text(BurbujaCroquis.titulo(mensaje)).font(.system(size: 14 * escala, weight: .semibold))
                Text(esta == false ? "Croquis 3D · aún no ha llegado a este iPad" : "Croquis 3D · toca para verlo")
                    .font(.system(size: 12 * escala)).opacity(0.7)
            }
        }
        .padding(.vertical, 2)
        .task(id: mensaje.id) {
            guard let r = mensaje.referencia else {
                esta = false
                return
            }
            let (nucleo, chat) = (almacen.nucleo, chat)
            esta = await Task.detached(priority: .utility) { nucleo.croquisDelChat(chat: chat, referencia: r) != nil }.value
        }
    }
}

// MARK: - Lo de debajo de la burbuja

/// **El punto de proyecto**: verde si vive tambien en las hojas de un
/// proyecto, rojo si solo en el chat (con un halo blanco).
struct PuntoDeProyecto: View {
    let verde: Bool
    @Environment(\.escalaChat) private var escala

    var body: some View {
        Circle()
            .fill(verde ? Color(hex: 0x2E9E4F) : Color(hex: 0xD24B3E))
            .padding(1.5 * escala)
            .background(Circle().fill(.white))
            .frame(width: 9 * escala, height: 9 * escala)
            .accessibilityLabel(verde ? "En un proyecto" : "Solo en el chat")
    }
}

/// Lo que el movil o Windows dicen del mensaje y va debajo de su burbuja:
/// de donde viene (la zona de un lienzo), el lienzo donde dibujo el
/// asistente y de quien llego.
struct PieDelMovil: View {
    let mensaje: MensajeChat
    let chat: String
    @Environment(\.escalaChat) private var escala

    var body: some View {
        if let v = mensaje.vieneDe {
            Button {
                abrirLienzo(mensaje.vieneDeDibujo)
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "link")
                    VStack(alignment: .leading, spacing: 0) {
                        Text("Viene de").font(.system(size: 9 * escala))
                        Text(v).font(.system(size: 12 * escala))
                    }
                    if mensaje.vieneDeDibujo != nil { Image(systemName: "arrow.right") }
                }
                .foregroundStyle(Paleta.chat.circuloIcono)
                .padding(.horizontal, 10).padding(.vertical, 6)
                .background(Paleta.chat.circulo, in: RoundedRectangle(cornerRadius: 10))
            }
            .buttonStyle(.plain)
            .disabled(mensaje.vieneDeDibujo == nil)
        }
        if let l = mensaje.lienzoIa {
            Button {
                abrirLienzo(l)
            } label: {
                Label("Abrir lienzo", systemImage: "scribble.variable").font(.system(size: 12 * escala))
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
        }
        if let r = mensaje.recibidoDe {
            Label("De \(r)", systemImage: "wifi")
                .font(.system(size: 10 * escala)).opacity(0.7)
        }
    }

    /// Un lienzo por su `referencia`: se abre como el `DIBUJO` que seria.
    private func abrirLienzo(_ id: String?) {
        guard let id, !id.isEmpty else { return }
        Navegacion.compartida.abrir(.lienzo(chat: chat, mensaje: mensajeDeLienzo(id, como: mensaje), foto: nil))
    }
}

/// Un `DIBUJO` que apunta al lienzo `referencia` (para abrir en el editor
/// un lienzo que no es el del mensaje: una hoja-pagina, «Viene de», el del
/// asistente). Lleva el id del mensaje de donde sale.
func mensajeDeLienzo(_ referencia: String, como m: MensajeChat) -> MensajeChat {
    var d = m
    d.clase = "DIBUJO"
    d.referencia = referencia
    return d
}

/// El id del lienzo de una foto, como el movil (`dibujoDeLaFoto`): su
/// `referencia` si la tiene y si no `foto-<id del mensaje>`.
func lienzoDeLaFoto(_ m: MensajeChat) -> String {
    if let r = m.referencia, !r.isEmpty { return r }
    return "foto-\(m.id)"
}
