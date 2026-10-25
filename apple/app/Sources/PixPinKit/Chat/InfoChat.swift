// **La ficha del chat** (`ventana_chat/ficha_del_chat.rs` de Windows,
// 25-oct-2026, el usuario: «Mira Telegram: una isla flotante que muestra el
// chat, fotos, videos, archivos, enlaces… y hacer click dentro no debe salir
// de esa zona»; del Mac, `Chat/FichaDelChat.swift`, la segunda pasada de
// Windows 99f918a, 2439c5c, 8c92ba4 y bd8ac36): la hoja que sale al pulsar
// el nombre del chat en su cabecera (o «Información» en su menu), hecha como
// el panel de medios compartidos de Telegram.
//
// - **Arriba**, el avatar redondo del chat, su nombre en seminegrita y
//   cuantas cosas tiene (con su codigo unico), con la lupa y el aspa en
//   botones redondos.
// - **Las pestanas**, texto con la rayita de color bajo la activa y su
//   cuenta apagada detras. Solo salen las que tienen algo; si sobran, el
//   sitio se reparte; si no caben, la tira se desplaza y la activa se ve.
// - **Los proyectos viven aqui** (2439c5c): la primera pestana, con la que
//   se abre, lista los de la lista de chats con su avatar, su nombre,
//   cuantas hojas tiene, su ultima linea y cuando se toco. Tocar otro
//   proyecto cierra la hoja y el chat pasa a el; tocar el de este chat
//   lleva a su «Galeria».
// - **El contenido, por meses** («Octubre de 2026»), lo mas nuevo arriba:
//   lo que se mira en una rejilla de borde a borde de tres a cinco columnas
//   (en los videos, el triangulo y lo que duran); lo que se lee en filas
//   (archivos con la chapa de su extension, enlaces con la letra de su
//   sitio, notas de voz con su boton: suenan sin salir de la hoja). En
//   «Fotos», tambien las fotos de las notas ricas (8c92ba4).
// - **Lo que se toca se ve DENTRO**: la previa en grande, la foto sobre
//   fondo oscuro, con las flechas a los lados (o deslizando el dedo) y, en
//   la cabecera, volver, «Ver en el chat» y «Abrir».
// - **Con el teclado del iPad** (`tecla`): ← → cambian de pestana (o
//   recorren la previa); ↓ entra en el contenido y las flechas mueven el
//   foco; Intro abre; Esc vuelve un paso y, al final, cierra.
//
// Que pestanas salen, que cuenta cada una, que hay en ella, el orden y a
// donde va el foco son de Rust (`ficha_del_chat.rs`: `Nucleo.fichaDelChat`,
// `cosasDeLaFicha`, `proyectosDeLaFicha`, `vecinaEnLaFicha`), el mismo
// reparto que Windows, el Mac y el movil.

import PixPinCore
import SwiftUI
import UIKit

/// Las medidas de la ficha (`pixpin_ui::info`, las `ISLA_*`), al 100 %.
enum MedidasFicha {
    static let relleno: CGFloat = 16
    static let cabecera: CGFloat = 64
    static let avatar: CGFloat = 40
    static let boton: CGFloat = 36
    static let botonHueco: CGFloat = 4
    static let buscarAlto: CGFloat = 36
    static let tira: CGFloat = 44
    static let pestanaRelleno: CGFloat = 14
    static let subrayado: CGFloat = 3
    static let hueco: CGFloat = 2
    static let mes: CGFloat = 36
    static let fila: CGFloat = 60
    static let filaEnlace: CGFloat = 72
    static let miniaturaFila: CGFloat = 44
    static let pieDeLista: CGFloat = 8
    static let flecha: CGFloat = 44
    static let margenPrevia: CGFloat = 12
}

/// Los textos de la ficha (`info-*` de `main.ftl`), los del Mac.
enum TextosFicha {
    static func rotulo(_ s: SeccionFicha) -> String {
        switch s {
        case .proyectos: "Proyectos"
        case .galeria: "Galería"
        case .todo: "Todo"
        case .fotos: "Fotos"
        case .videos: "Vídeos"
        case .archivos: "Archivos"
        case .enlaces: "Enlaces"
        case .voz: "Voz"
        case .dibujos: "Dibujos"
        case .fijados: "Fijados"
        case .buzon: "Buzón"
        }
    }

    /// El icono de una seccion (para el vacio).
    static func icono(_ s: SeccionFicha) -> String {
        switch s {
        case .fotos, .galeria: "photo.on.rectangle"
        case .videos: "video"
        case .enlaces: "link"
        case .voz: "mic"
        case .dibujos: "scribble.variable"
        case .fijados: "pin"
        case .proyectos: "folder"
        default: "doc.text"
        }
    }

    /// La frase de una seccion vacia (`info-vacia-*`).
    static func vacia(_ s: SeccionFicha) -> String {
        switch s {
        case .galeria: "Las hojas del proyecto saldrán aquí"
        case .fotos: "Las fotos del chat saldrán aquí"
        case .videos: "Los vídeos del chat saldrán aquí"
        case .archivos: "Los archivos del chat saldrán aquí"
        case .enlaces: "Los enlaces del chat saldrán aquí"
        case .voz: "Las notas de voz saldrán aquí"
        case .dibujos: "Los dibujos del chat saldrán aquí"
        case .fijados: "Lo que fijes saldrá aquí"
        case .buzon: "El buzón está vacío"
        case .todo: "Aquí no hay nada todavía"
        case .proyectos: "Aún no hay proyectos"
        }
    }

    /// «24 cosas  ·  K7Q2» (`info-resumen`).
    static func resumen(cosas: UInt32, codigo: String) -> String {
        let n = cosas == 1 ? "1 cosa" : "\(cosas) cosas"
        return codigo.isEmpty ? n : "\(n)  ·  \(codigo)"
    }

    /// «12 hojas» (`chat-hojas`).
    static func hojas(_ n: UInt32) -> String { n == 1 ? "1 hoja" : "\(n) hojas" }

    static let nada = "Nada con eso"
    static let buscar = "Buscar"
    static let abrir = "Abrir"
    static let abrirEnlace = "Abrir enlace"
    static let reproducir = "Reproducir"
    static let verEnElChat = "Ver en el chat"
    static let notaDeVoz = "Nota de voz"
    static let esteChat = "Este chat"
    static let noDisponible = "Ubicación no disponible"

    /// «3 de 24» (`info-previa-posicion`).
    static func posicion(_ k: Int, de n: Int) -> String { "\(k) de \(n)" }

    /// Los meses (`chat-mes-*`).
    static let meses = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio",
                        "agosto", "septiembre", "octubre", "noviembre", "diciembre"]

    /// La clave del mes de `ms`, en la hora de este iPad: cambia de un mes
    /// al siguiente (`clave_del_mes` de Windows).
    static func claveDelMes(_ ms: Int64) -> Int64 {
        let d = Date(timeIntervalSince1970: TimeInterval(ms) / 1000)
        let c = Calendar.current.dateComponents([.year, .month], from: d)
        return Int64((c.year ?? 0) * 12 + (c.month ?? 1))
    }

    /// «Octubre de 2026» (`info-mes`), con su primera letra en mayuscula.
    static func rotuloDelMes(_ ms: Int64) -> String {
        let d = Date(timeIntervalSince1970: TimeInterval(ms) / 1000)
        let c = Calendar.current.dateComponents([.year, .month], from: d)
        let mes = meses[safe: (c.month ?? 1) - 1] ?? ""
        let texto = "\(mes) de \(c.year ?? 0)"
        return texto.prefix(1).uppercased() + texto.dropFirst()
    }

    /// De que es una cosa (`chat-clase-*`). Una nota es solo su texto; una
    /// clase que no se conoce se dice con su palabra.
    static func clase(_ c: String) -> String? {
        switch c {
        case "", "NOTA": nil
        case "IMAGEN": "Imagen"
        case "ARCHIVO": "Archivo"
        case "VOZ": "Nota de voz"
        case "DIBUJO": "Dibujo"
        case "PAGINA": "Página"
        case "PROYECTO": "Proyecto"
        case "MINIAPP": "Mini-aplicación"
        default: c
        }
    }

    /// Cuando: la hora si es de hoy; si no, el dia (`etiqueta_hora`).
    static func hora(_ ms: Int64) -> String {
        let d = Date(timeIntervalSince1970: TimeInterval(ms) / 1000)
        if Calendar.current.isDateInToday(d) { return d.formatted(date: .omitted, time: .shortened) }
        return d.formatted(.dateTime.day().month(.abbreviated).year())
    }

    static func tamano(_ bytes: Int64) -> String {
        ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file)
    }

    /// El nombre con el que se ensena una cosa: su texto (la primera linea)
    /// o, sin el, el del fichero.
    static func nombre(_ m: MensajeChat) -> String {
        let r = m.resumen.trimmingCharacters(in: .whitespacesAndNewlines)
        if r.isEmpty { return m.nombre }
        return r.split(separator: "\n", maxSplits: 1).first.map(String.init) ?? r
    }

    /// El nombre de una cosa de la ficha: el de su foto, si es la de una
    /// nota y su alt dice algo; si no, el del mensaje.
    static func nombre(_ c: CosaDeLaFicha) -> String {
        c.foto != nil && !c.alt.isEmpty ? c.alt : nombre(c.mensaje)
    }

    /// `texto` con lo buscado en negrita (`negritas` de Windows); sin nada
    /// buscado, entero en `peso`.
    static func conNegritas(_ texto: String, buscado: String, tam: CGFloat,
                            peso: Font.Weight = .regular) -> AttributedString {
        let llano = texto.replacingOccurrences(of: "\n", with: " ")
        var a = AttributedString(llano)
        let tramos = negritasDeLaFicha(texto: llano, buscado: buscado)
        a.font = Font.system(size: tam, weight: tramos.isEmpty ? peso : .regular)
        for t in tramos {
            guard let r = Range(NSRange(location: Int(t.inicio), length: Int(t.largo)), in: llano),
                  let desde = AttributedString.Index(r.lowerBound, within: a),
                  let hasta = AttributedString.Index(r.upperBound, within: a) else { continue }
            a[desde ..< hasta].font = Font.system(size: tam, weight: .bold)
        }
        return a
    }
}

// MARK: - El estado

/// Lo que se ve de la seccion de un mes: su rotulo y sus cosas (sus sitios
/// en `cosas`).
struct MesDeLaFicha: Identifiable {
    let clave: Int64
    let desde: Int
    let hasta: Int
    var id: Int { desde }
}

/// **El estado de la ficha** de un chat: la pestana, lo que hay en ella, lo
/// buscado, el foco del teclado y lo que se ve en grande.
@MainActor
final class ModeloFichaDelChat: ObservableObject {
    let chat: String
    let almacen: Almacen
    @Published private(set) var leida: FichaDelChatLeida?
    @Published private(set) var seccion: SeccionFicha = .todo
    @Published private(set) var cosas: [CosaDeLaFicha] = []
    /// Los de la pestana «Proyectos», en el orden de la lista de chats.
    @Published private(set) var proyectos: [ProyectoDeLaFicha] = []
    /// Las cosas por meses, lo mas nuevo arriba.
    @Published private(set) var porMeses: [MesDeLaFicha] = []
    @Published private(set) var cargando = false
    /// La cosa que se ve en grande (su sitio en `cosas`); `nil`, la rejilla.
    @Published var enPrevia: Int?
    /// La cosa (o el proyecto) con el foco del teclado, por su sitio.
    @Published private(set) var foco: Int?
    @Published var buscando = false
    @Published var buscado = "" { didSet { if buscado != oldValue { leerCosas() } } }
    /// El ancho del contenido, que dice cuantas columnas tiene la rejilla.
    var anchoContenido: CGFloat = 400
    /// Lo que se ve de la lista ahora mismo: ↓ entra por la primera.
    var visibles: Set<Int> = []
    /// Cierra la hoja (lo pone la vista).
    var alCerrar: (() -> Void)?
    /// Sube con cada lectura: lo que llegue de una vieja no pisa lo nuevo.
    private var vuelta = 0

    init(chat: String, almacen: Almacen) {
        self.chat = chat
        self.almacen = almacen
    }

    var pestanas: [PestanaFicha] { leida?.pestanas ?? [] }

    /// La pestana puesta va en rejilla.
    var cuadricula: Bool { pestanas.first { $0.seccion == seccion }?.cuadricula ?? false }

    var previa: CosaDeLaFicha? { enPrevia.flatMap { cosas[safe: $0] } }

    /// Cuantas cosas (o proyectos) ensena la pestana puesta.
    var cuantas: Int { seccion == .proyectos ? proyectos.count : cosas.count }

    /// Si lo que se teclea es para la lupa: con su caja y fuera de la previa.
    var tecladoEnLaLupa: Bool { buscando && enPrevia == nil }

    /// La clave con que cada cosa se identifica en la lista: su mensaje y,
    /// en una nota, que foto suya es.
    nonisolated static func clave(_ c: CosaDeLaFicha) -> String {
        "\(c.mensaje.id)#\(c.foto.map { String($0) } ?? "")"
    }

    /// **Abre la ficha** en la primera pestana que tenga algo (los proyectos,
    /// casi siempre), sin la busqueda de la vez anterior.
    func abrir() {
        enPrevia = nil
        foco = nil
        buscando = false
        buscado = ""
        releer(primera: true)
    }

    func cerrar() {
        enPrevia = nil
        foco = nil
        alCerrar?()
    }

    func elegir(_ s: SeccionFicha) {
        enPrevia = nil
        foco = nil
        guard s != seccion else { return }
        seccion = s
        cosas = []
        proyectos = []
        porMeses = []
        visibles = []
        leerCosas()
    }

    /// Vuelve a leer las pestanas y lo de la puesta.
    func releer(primera: Bool = false) {
        let nucleo = almacen.nucleo
        let chat = self.chat
        let actual: SeccionFicha? = primera ? nil : seccion
        _ = Task<Void, Never> {
            let r = await Task.detached(priority: .userInitiated) { () -> FichaDelChatLeida? in
                try? nucleo.fichaDelChat(chat: chat, actual: actual)
            }.value
            guard let r else { return }
            leida = r
            if primera, seccion != r.primera {
                seccion = r.primera
                cosas = []
                proyectos = []
                porMeses = []
            }
            leerCosas()
        }
    }

    private func leerCosas() {
        vuelta += 1
        let mia = vuelta
        let nucleo = almacen.nucleo
        let (chat, seccion, buscado) = (self.chat, self.seccion, self.buscado)
        cargando = true
        if seccion == .proyectos {
            _ = Task<Void, Never> {
                let r = await Task.detached(priority: .userInitiated) { () -> [ProyectoDeLaFicha] in
                    (try? nucleo.proyectosDeLaFicha(chat: chat, buscado: buscado)) ?? []
                }.value
                guard mia == vuelta else { return }
                cargando = false
                if r.map(\.ficha.id) != proyectos.map(\.ficha.id) { foco = nil }
                proyectos = r
                cosas = []
                porMeses = []
            }
            return
        }
        _ = Task<Void, Never> {
            let r = await Task.detached(priority: .userInitiated) { () -> [CosaDeLaFicha] in
                (try? nucleo.cosasDeLaFicha(chat: chat, seccion: seccion, buscado: buscado)) ?? []
            }.value
            guard mia == vuelta else { return }
            cargando = false
            // La que se veia en grande se vuelve a buscar por su clave: si ya
            // no esta, se vuelve a la rejilla en vez de ensenar otra cosa.
            let vista = previa.map(Self.clave)
            if r.map(Self.clave) != cosas.map(Self.clave) { foco = nil }
            // Lo mas nuevo ya viene primero (el nucleo, como Telegram).
            cosas = r
            proyectos = []
            porMeses = Self.agrupar(r)
            enPrevia = vista.flatMap { v in r.firstIndex { Self.clave($0) == v } }
        }
    }

    /// Las cosas por meses: un rotulo cada vez que cambia el mes.
    static func agrupar(_ cosas: [CosaDeLaFicha]) -> [MesDeLaFicha] {
        agrupar(cuando: cosas.map(\.mensaje.cuando))
    }

    /// Lo mismo por sus fechas (en ms), para probarlo sin mensajes.
    nonisolated static func agrupar(cuando: [Int64]) -> [MesDeLaFicha] {
        var v: [MesDeLaFicha] = []
        var desde = 0
        var clave: Int64?
        for (i, ms) in cuando.enumerated() {
            let k = TextosFicha.claveDelMes(ms)
            if let actual = clave, actual != k {
                v.append(MesDeLaFicha(clave: actual, desde: desde, hasta: i))
                desde = i
            }
            clave = k
        }
        if let clave { v.append(MesDeLaFicha(clave: clave, desde: desde, hasta: cuando.count)) }
        return v
    }

    /// La anterior (`-1`) o la siguiente (`+1`) en la previa.
    func mover(_ paso: Int) {
        guard let i = enPrevia, cosas.indices.contains(i + paso) else { return }
        enPrevia = i + paso
    }

    func hayVecina(_ paso: Int) -> Bool {
        guard let i = enPrevia else { return false }
        return cosas.indices.contains(i + paso)
    }

    /// Apaga la lupa y vacia lo buscado.
    func apagarLupa() {
        buscando = false
        buscado = ""
    }

    /// Un toque (o Intro) en una cosa: un enlace se abre en Safari y una
    /// nota de voz suena, y la hoja se queda; lo demas, en grande dentro.
    func pulsar(_ i: Int) {
        guard let c = cosas[safe: i] else { return }
        if c.foto == nil, c.mensaje.ruta == nil, let e = c.enlace, let url = URL(string: e.url) {
            UIApplication.shared.open(url)
            return
        }
        if c.foto == nil, c.mensaje.clase == "VOZ" {
            AbrirDeLaFicha.sonar(c.mensaje, chat: chat, almacen: almacen)
            return
        }
        enPrevia = i
    }

    /// **Tocar un proyecto** (`pulsar_proyecto`): el de este chat no sale de
    /// la hoja, lleva a su «Galeria»; otro es salir a proposito: la hoja se
    /// cierra y el chat pasa a el. La lupa se apaga en los dos casos.
    func pulsarProyecto(_ i: Int) {
        guard let p = proyectos[safe: i] else { return }
        apagarLupa()
        foco = nil
        if p.este {
            elegir(.galeria)
            releer()
            return
        }
        let id = p.ficha.id
        cerrar()
        Navegacion.compartida.irAlChat(id)
    }

    /// «Ver en el chat»: cierra la hoja y salta al mensaje.
    func verEnElChat(_ c: CosaDeLaFicha) {
        let (chat, id) = (self.chat, c.mensaje.id)
        cerrar()
        Navegacion.compartida.irAlChat(chat, mensaje: id)
    }

    /// «Abrir» de la previa; lo que no tiene con que abrirse aparte se ve en
    /// el chat.
    func abrirLaPrevia() {
        guard let c = previa else { return }
        if !AbrirDeLaFicha.abrir(c, modelo: self) { verEnElChat(c) }
    }

    /// Las teclas que la ficha mira.
    enum Tecla { case escape, intro, izquierda, derecha, arriba, abajo }

    /// **El teclado en la ficha** (`tecla` de Windows): ← y → cambian de
    /// pestana (o recorren la previa); ↓ entra en el contenido y las
    /// flechas mueven el foco; Intro abre; Esc vuelve un paso y, al final,
    /// cierra. Devuelve si era suya. Con la lupa, Esc la apaga y ← → son
    /// de su caja.
    func tecla(_ t: Tecla) -> Bool {
        if enPrevia != nil {
            switch t {
            case .escape: enPrevia = nil
            case .izquierda: mover(-1)
            case .derecha: mover(1)
            case .intro: abrirLaPrevia()
            default: return false
            }
            return true
        }
        if tecladoEnLaLupa {
            switch t {
            case .escape:
                apagarLupa()
                return true
            case .izquierda, .derecha: return false
            default: break
            }
        }
        switch (t, foco) {
        case (.escape, .some):
            foco = nil
        case (.escape, nil):
            cerrar()
        case let (.intro, .some(n)):
            if seccion == .proyectos { pulsarProyecto(n) } else { pulsar(n) }
        case (.izquierda, nil), (.derecha, nil):
            let lista = pestanas.map(\.seccion)
            let k = (lista.firstIndex(of: seccion) ?? 0) + (t == .izquierda ? -1 : 1)
            if let s = lista[safe: k] { elegir(s) }
        case (.abajo, nil):
            // Entra en lo que se ve: la primera cosa visible.
            if cuantas > 0 { foco = min(visibles.min() ?? 0, cuantas - 1) }
        case let (_, .some(n)) where t != .intro:
            let (dx, dy): (Int32, Int32) = switch t {
            case .izquierda: (-1, 0)
            case .derecha: (1, 0)
            case .arriba: (0, -1)
            default: (0, 1)
            }
            let meses: [Int64] = seccion == .proyectos
                ? Array(repeating: 0, count: proyectos.count)
                : cosas.map { TextosFicha.claveDelMes($0.mensaje.cuando) }
            if let k = vecinaEnLaFicha(meses: meses, cuadricula: cuadricula,
                                       ancho: UInt32(max(anchoContenido, 1)),
                                       actual: UInt32(n), dx: dx, dy: dy) {
                foco = Int(k)
            } else if t == .arriba {
                // Arriba del todo, el foco vuelve a las pestanas.
                foco = nil
            }
        default:
            return false
        }
        return true
    }
}

/// **«Abrir»** de la previa: cada cosa con lo suyo, como su burbuja.
@MainActor
enum AbrirDeLaFicha {
    /// El icono del boton.
    static func icono(_ c: CosaDeLaFicha) -> String {
        if c.foto == nil, c.mensaje.ruta == nil, c.enlace != nil { return "link" }
        if c.foto == nil, c.mensaje.clase == "VOZ" || c.esVideo { return "play.fill" }
        return "arrow.up.forward.square"
    }

    static func rotulo(_ c: CosaDeLaFicha) -> String {
        if c.foto == nil, c.mensaje.ruta == nil, c.enlace != nil { return TextosFicha.abrirEnlace }
        if c.foto == nil, c.mensaje.clase == "VOZ" || c.esVideo { return TextosFicha.reproducir }
        return TextosFicha.abrir
    }

    /// Una nota de voz suena sin salir de la hoja.
    static func sonar(_ m: MensajeChat, chat: String, almacen: Almacen) {
        _ = Task<Void, Never> {
            guard let url = await resolverFichero(m, chat, almacen),
                  FileManager.default.fileExists(atPath: url.path) else { return }
            ReproductorVoz.compartido.alternar(m.id, url: url, titulo: BibliotecaAudio.titulo(de: m))
        }
    }

    /// Abre `c`; `false` si no tiene con que abrirse aparte (una mini-app, un
    /// acceso a otro proyecto): entonces se ve en el chat. Los visores van a
    /// pantalla completa por encima de todo: antes se cierra la hoja.
    static func abrir(_ c: CosaDeLaFicha, modelo: ModeloFichaDelChat) -> Bool {
        let m = c.mensaje
        let (chat, almacen) = (modelo.chat, modelo.almacen)
        // La foto de una nota sale en un pin, como al tocarla en su burbuja.
        if c.foto != nil {
            guard let ruta = c.rutaFoto, FileManager.default.fileExists(atPath: ruta) else { return true }
            modelo.cerrar()
            _ = GestorPinesIPad.compartido.pinear(url: URL(fileURLWithPath: ruta))
            return true
        }
        if m.ruta == nil, let e = c.enlace, let url = URL(string: e.url) {
            UIApplication.shared.open(url)
            return true
        }
        if m.clase == "VOZ" {
            sonar(m, chat: chat, almacen: almacen)
            return true
        }
        let nav = Navegacion.compartida
        switch m.clase {
        case "DIBUJO":
            guard let r = m.referencia, !r.isEmpty else { return false }
            despues(modelo) { nav.abrir(.lienzo(chat: chat, mensaje: m, foto: nil)) }
            return true
        case "NOTA":
            despues(modelo) { nav.abrir(.nota(chat: chat, mensaje: m)) }
            return true
        case "TABLA":
            despues(modelo) { nav.abrir(.hoja(chat: chat, mensaje: m)) }
            return true
        default:
            break
        }
        guard let ruta = m.ruta, !ruta.isEmpty else { return false }
        _ = Task<Void, Never> {
            guard let url = await almacen.url(deRuta: ruta, en: chat),
                  FileManager.default.fileExists(atPath: url.path) else { return }
            despues(modelo) {
                if m.clase == "IMAGEN", !c.esVideo {
                    // En su lienzo, como la burbuja.
                    nav.abrir(.lienzo(chat: chat, mensaje: m, foto: url))
                } else {
                    nav.abrirFichero(url, titulo: m.nombre.isEmpty ? nil : m.nombre, chat: chat, mensaje: m)
                }
            }
        }
        return true
    }

    /// Cierra la hoja y, cuando se fue, abre (dos presentaciones a la vez no
    /// caben).
    private static func despues(_ modelo: ModeloFichaDelChat, _ hacer: @escaping @MainActor () -> Void) {
        modelo.cerrar()
        _ = Task<Void, Never> { @MainActor in
            try? await Task.sleep(for: .milliseconds(400))
            hacer()
        }
    }
}

// MARK: - La hoja

/// El avatar de un chat: su foto (imagen, emoji o color) o el circulo de su
/// color con sus iniciales (`Chat/FotoDelChat.swift`).
struct AvatarDelChat: View {
    let id: String
    let nombre: String
    let lado: CGFloat
    var foto: FotoDelChat?

    var body: some View {
        CirculoDelChat(id: id, nombre: nombre, lado: lado, foto: foto)
    }
}

/// **El avatar de la isla**: tocarlo abre la hoja de la foto del chat
/// (Windows 22bf4dc). En el iPad la ficha ya es una hoja, asi que la de la
/// foto sale encima de ella desde aqui (no por `PeticionDeFoto`, que la
/// pediria a la seccion de debajo, ocupada con la ficha).
struct AvatarDeLaIsla: View {
    /// Mirado aqui: al guardar una foto nueva, la isla la ensena ya.
    @ObservedObject var almacen: Almacen
    let id: String
    @State private var eligiendo = false

    var body: some View {
        let datos = almacen.chats.first(where: { $0.id == id })
        Button { eligiendo = true } label: {
            AvatarDelChat(id: id, nombre: datos?.nombre ?? "", lado: MedidasFicha.avatar, foto: datos?.foto)
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .hoverEffect(.lift)
        .accessibilityLabel(TextosFoto.actuales.menu)
        .sheet(isPresented: $eligiendo) {
            HojaFotoDelChat(chat: id, almacen: almacen) { eligiendo = false }
        }
    }
}

/// **La ficha**, en una hoja encima del chat. Tocar dentro no sale de ella.
struct VistaInfoChat: View {
    let almacen: Almacen
    let chat: String
    let titulo: String
    @Environment(\.dismiss) private var cerrarHoja
    @StateObject private var ficha: ModeloFichaDelChat
    @FocusState private var enBuscar: Bool
    @FocusState private var conTeclado: Bool

    init(almacen: Almacen, chat: String, titulo: String) {
        self.almacen = almacen
        self.chat = chat
        self.titulo = titulo
        _ficha = StateObject(wrappedValue: ModeloFichaDelChat(chat: chat, almacen: almacen))
    }

    private var nombreDelChat: String {
        almacen.chats.first { $0.id == chat }?.nombre ?? titulo
    }

    var body: some View {
        VStack(spacing: 0) {
            if let c = ficha.previa {
                cabeceraDeLaPrevia(c)
                Paleta.proyectos.separador.frame(height: 1)
                PreviaDeLaFicha(ficha: ficha, cosa: c)
            } else {
                cabecera
                TiraDePestanas(ficha: ficha)
                    .padding(.horizontal, MedidasFicha.relleno / 2)
                    .frame(height: MedidasFicha.tira)
                Paleta.proyectos.separador.frame(height: 1)
                ContenidoDeLaFicha(ficha: ficha)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Paleta.proyectos.lista.ignoresSafeArea())
        .focusable()
        .focused($conTeclado)
        .focusEffectDisabled()
        .onKeyPress(phases: .down) { k in teclear(k) ? .handled : .ignored }
        .onAppear {
            ficha.alCerrar = { cerrarHoja() }
            ficha.abrir()
            conTeclado = true
        }
        .presentationDragIndicator(.hidden)
    }

    private func teclear(_ k: KeyPress) -> Bool {
        let t: ModeloFichaDelChat.Tecla? = switch k.key {
        case .escape: .escape
        case .return: .intro
        case .leftArrow: .izquierda
        case .rightArrow: .derecha
        case .upArrow: .arriba
        case .downArrow: .abajo
        default: nil
        }
        guard let t, k.modifiers.subtracting([.numericPad, .function]).isEmpty else { return false }
        return ficha.tecla(t)
    }

    // La cabecera: avatar, nombre y cuantas cosas, lupa y aspa. Con la lupa
    // encendida, la caja de buscar ocupa el sitio del avatar y del nombre.
    private var cabecera: some View {
        HStack(spacing: 0) {
            if ficha.buscando {
                TextField("", text: $ficha.buscado,
                          prompt: Text(TextosFicha.buscar).foregroundStyle(Paleta.proyectos.apagado))
                    .textFieldStyle(.plain)
                    .font(.system(size: 15))
                    .foregroundStyle(Paleta.proyectos.texto)
                    .focused($enBuscar)
                    .submitLabel(.search)
                    .autocorrectionDisabled()
                    .padding(.horizontal, 14)
                    .frame(height: MedidasFicha.buscarAlto)
                    .background(Paleta.proyectos.buscador, in: Capsule())
                    .onAppear { enBuscar = true }
            } else {
                AvatarDeLaIsla(almacen: almacen, id: chat)
                rotulos(nombreDelChat,
                       subtitulo: ficha.leida.map { TextosFicha.resumen(cosas: $0.cosas, codigo: $0.codigo) } ?? "")
                    .padding(.leading, 12)
            }
            Spacer(minLength: 8)
            BotonRedondoFicha(simbolo: "magnifyingglass", ayuda: TextosFicha.buscar, puesto: ficha.buscando) {
                if ficha.buscando {
                    ficha.apagarLupa()
                    conTeclado = true
                } else {
                    ficha.buscando = true
                }
            }
            BotonRedondoFicha(simbolo: "xmark", ayuda: "Cerrar") { ficha.cerrar() }
                .padding(.leading, MedidasFicha.botonHueco)
        }
        .padding(.leading, MedidasFicha.relleno)
        .padding(.trailing, MedidasFicha.relleno - 4)
        .frame(height: MedidasFicha.cabecera)
    }

    // En la previa: volver en el sitio del avatar, el nombre de la cosa y
    // «3 de 24 · cuando · peso», y sus dos botones junto al aspa.
    private func cabeceraDeLaPrevia(_ c: CosaDeLaFicha) -> some View {
        HStack(spacing: 0) {
            BotonRedondoFicha(simbolo: "arrow.left", ayuda: "Volver", lado: MedidasFicha.avatar) {
                ficha.enPrevia = nil
            }
            rotulos(TextosFicha.nombre(c), subtitulo: subtituloDePrevia(c))
                .padding(.leading, 12)
            Spacer(minLength: 8)
            BotonRedondoFicha(simbolo: "bubble.left.and.bubble.right", ayuda: TextosFicha.verEnElChat) {
                ficha.verEnElChat(c)
            }
            BotonRedondoFicha(simbolo: AbrirDeLaFicha.icono(c), ayuda: AbrirDeLaFicha.rotulo(c)) {
                ficha.abrirLaPrevia()
            }
            .padding(.leading, MedidasFicha.botonHueco)
            BotonRedondoFicha(simbolo: "xmark", ayuda: "Cerrar") { ficha.cerrar() }
                .padding(.leading, MedidasFicha.botonHueco)
        }
        .padding(.leading, MedidasFicha.relleno)
        .padding(.trailing, MedidasFicha.relleno - 4)
        .frame(height: MedidasFicha.cabecera)
    }

    private func subtituloDePrevia(_ c: CosaDeLaFicha) -> String {
        var v = [TextosFicha.posicion((ficha.enPrevia ?? 0) + 1, de: ficha.cosas.count),
                 TextosFicha.hora(c.mensaje.cuando)]
        // Lo que pesa la nota no es lo que pesa su foto.
        if c.mensaje.bytes > 0, c.foto == nil { v.append(TextosFicha.tamano(c.mensaje.bytes)) }
        return v.joined(separator: "  ·  ")
    }

    private func rotulos(_ t: String, subtitulo: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(t)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(Paleta.proyectos.texto)
            Text(subtitulo)
                .font(.system(size: 13))
                .foregroundStyle(Paleta.proyectos.apagado)
        }
        .lineLimit(1)
        .truncationMode(.tail)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Un boton redondo de la ficha, con su fondo mientras esta encendido (como
/// la lupa) o con el puntero encima.
private struct BotonRedondoFicha: View {
    let simbolo: String
    let ayuda: String
    var puesto = false
    var lado: CGFloat = MedidasFicha.boton
    let accion: () -> Void

    var body: some View {
        Button(action: accion) {
            Image(systemName: simbolo)
                .font(.system(size: 16))
                .foregroundStyle(puesto ? Color.white : Paleta.proyectos.texto)
                .frame(width: lado, height: lado)
                .background(puesto ? Paleta.proyectos.filaElegida : .clear, in: Circle())
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
        .accessibilityLabel(ayuda)
    }
}

// MARK: - Las pestanas

/// **La tira de pestanas**: texto con la rayita de color bajo la activa y la
/// cuenta apagada detras, en una sola fila. Si sobran, el sitio se reparte
/// entre todas; si no caben, la tira se desplaza y la activa se ve.
private struct TiraDePestanas: View {
    @ObservedObject var ficha: ModeloFichaDelChat

    var body: some View {
        GeometryReader { g in
            ScrollViewReader { proxy in
                ScrollView(.horizontal, showsIndicators: false) {
                    RepartoDePestanas(ancho: g.size.width) {
                        ForEach(ficha.pestanas, id: \.seccion) { p in
                            PestanaDeLaFicha(pestana: p, activa: p.seccion == ficha.seccion) { ficha.elegir(p.seccion) }
                                .id(p.seccion)
                        }
                    }
                }
                .onChange(of: ficha.seccion, initial: true) { _, s in proxy.scrollTo(s) }
            }
        }
    }
}

/// Coloca las pestanas seguidas, cada una a su ancho; si caben todas, lo que
/// sobra se reparte a partes iguales y llenan la tira (`Isla::pestanas`).
private struct RepartoDePestanas: Layout {
    let ancho: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let total = subviews.reduce(CGFloat(0)) { $0 + $1.sizeThatFits(.unspecified).width }
        return CGSize(width: max(total, ancho), height: MedidasFicha.tira)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let anchos = subviews.map { $0.sizeThatFits(.unspecified).width }
        let total = anchos.reduce(0, +)
        let sobra = total < ancho && !anchos.isEmpty ? (ancho - total) / CGFloat(anchos.count) : 0
        var x = bounds.minX
        for (s, w) in zip(subviews, anchos) {
            s.place(at: CGPoint(x: x, y: bounds.minY), anchor: .topLeading,
                    proposal: ProposedViewSize(width: w + sobra, height: bounds.height))
            x += w + sobra
        }
    }
}

private struct PestanaDeLaFicha: View {
    let pestana: PestanaFicha
    let activa: Bool
    let accion: () -> Void

    var body: some View {
        Button(action: accion) {
            HStack(alignment: .firstTextBaseline, spacing: 5) {
                Text(TextosFicha.rotulo(pestana.seccion))
                    .font(.system(size: 15, weight: activa ? .semibold : .regular))
                    .foregroundStyle(activa ? Paleta.chat.enviar : Paleta.proyectos.texto)
                Text("\(pestana.cuenta)")
                    .font(.system(size: 13))
                    .foregroundStyle(activa ? Paleta.chat.enviar.opacity(0.75) : Paleta.proyectos.apagado)
            }
            .fixedSize()
            .frame(height: MedidasFicha.tira)
            // La rayita, tan ancha como el texto y pegada abajo.
            .overlay(alignment: .bottom) {
                if activa {
                    Capsule().fill(Paleta.chat.enviar).frame(height: MedidasFicha.subrayado)
                }
            }
            .padding(.horizontal, MedidasFicha.pestanaRelleno)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
    }
}

// MARK: - El contenido

/// **El contenido de la pestana**: los proyectos en filas; lo demas por
/// meses, en rejilla o en filas. El foco del teclado lleva su filete y se
/// mantiene a la vista.
private struct ContenidoDeLaFicha: View {
    @ObservedObject var ficha: ModeloFichaDelChat

    var body: some View {
        if ficha.cuantas == 0 {
            vacio
        } else {
            GeometryReader { g in
                ScrollViewReader { proxy in
                    ScrollView {
                        if ficha.seccion == .proyectos {
                            proyectos
                        } else {
                            porMeses(ancho: g.size.width)
                        }
                    }
                    .scrollDismissesKeyboard(.immediately)
                    .onChange(of: ficha.foco) { _, f in
                        guard let f else { return }
                        proxy.scrollTo(clave(f))
                    }
                }
                .onAppear { ficha.anchoContenido = g.size.width }
                .onChange(of: g.size.width) { _, w in ficha.anchoContenido = w }
            }
            .id(ficha.seccion)
        }
    }

    private func clave(_ n: Int) -> String {
        if ficha.seccion == .proyectos { return ficha.proyectos[safe: n]?.ficha.id ?? "" }
        return ficha.cosas[safe: n].map(ModeloFichaDelChat.clave) ?? ""
    }

    // Vacio: un icono grande y una frase de lo que falta, en medio.
    private var vacio: some View {
        VStack(spacing: 14) {
            if ficha.cargando {
                ProgressView()
            } else {
                Image(systemName: TextosFicha.icono(ficha.seccion))
                    .font(.system(size: 22))
                    .foregroundStyle(Paleta.proyectos.apagado.opacity(0.8))
                    .frame(width: 44, height: 44)
                    .background(Paleta.proyectos.apagado.opacity(0.12), in: Circle())
                Text(ficha.buscado.trimmingCharacters(in: .whitespaces).isEmpty
                     ? TextosFicha.vacia(ficha.seccion) : TextosFicha.nada)
                    .font(.system(size: 15))
                    .foregroundStyle(Paleta.proyectos.apagado)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 24)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // Los proyectos, sin rotulos de mes: van en el orden de la lista.
    private var proyectos: some View {
        LazyVStack(spacing: 0) {
            ForEach(Array(ficha.proyectos.enumerated()), id: \.element.ficha.id) { i, p in
                FilaDeProyecto(proyecto: p, buscado: ficha.buscado, nucleo: ficha.almacen.nucleo)
                    .overlay { if ficha.foco == i { MarcaDelFoco(radio: 10) } }
                    .onTapGesture { ficha.pulsarProyecto(i) }
                    .onAppear { ficha.visibles.insert(i) }
                    .onDisappear { ficha.visibles.remove(i) }
                    .id(p.ficha.id)
            }
        }
        .padding(.horizontal, MedidasFicha.relleno / 2)
        .padding(.vertical, MedidasFicha.pieDeLista)
    }

    // Cada mes bajo su rotulo; en la rejilla, de borde a borde.
    private func porMeses(ancho: CGFloat) -> some View {
        let columnas = Int(columnasDeLaFicha(ancho: UInt32(max(ancho, 1))))
        let rejilla = Array(repeating: GridItem(.flexible(), spacing: MedidasFicha.hueco), count: max(columnas, 1))
        return LazyVStack(alignment: .leading, spacing: 0) {
            ForEach(ficha.porMeses) { mes in
                RotuloDelMes(cuando: ficha.cosas[safe: mes.desde]?.mensaje.cuando ?? 0)
                if ficha.cuadricula {
                    LazyVGrid(columns: rejilla, spacing: MedidasFicha.hueco) {
                        ForEach(mes.desde ..< mes.hasta, id: \.self) { i in cosa(i) }
                    }
                    .padding(.bottom, MedidasFicha.hueco)
                } else {
                    ForEach(mes.desde ..< mes.hasta, id: \.self) { i in cosa(i) }
                }
            }
        }
        .padding(.horizontal, ficha.cuadricula ? 0 : MedidasFicha.relleno / 2)
        .padding(.bottom, MedidasFicha.pieDeLista)
    }

    @ViewBuilder
    private func cosa(_ i: Int) -> some View {
        if let c = ficha.cosas[safe: i] {
            Group {
                if ficha.cuadricula {
                    CeldaDeLaFicha(cosa: c, chat: ficha.chat, almacen: ficha.almacen)
                } else {
                    FilaDeLaFicha(cosa: c, seccion: ficha.seccion, buscado: ficha.buscado,
                                  chat: ficha.chat, almacen: ficha.almacen)
                }
            }
            .overlay { if ficha.foco == i { MarcaDelFoco(radio: ficha.cuadricula ? 0 : 10) } }
            .onTapGesture { ficha.pulsar(i) }
            .contextMenu {
                Button { ficha.verEnElChat(c) } label: {
                    Label(TextosFicha.verEnElChat, systemImage: "bubble.left.and.bubble.right")
                }
            }
            .onAppear { ficha.visibles.insert(i) }
            .onDisappear { ficha.visibles.remove(i) }
            .id(ModeloFichaDelChat.clave(c))
        }
    }
}

/// El foco del teclado: el filete del color de la casa.
private struct MarcaDelFoco: View {
    let radio: CGFloat

    var body: some View {
        RoundedRectangle(cornerRadius: radio)
            .fill(Paleta.chat.enviar.opacity(0.10))
            .overlay { RoundedRectangle(cornerRadius: radio).strokeBorder(Paleta.chat.enviar, lineWidth: 2) }
            .padding(1)
            .allowsHitTesting(false)
    }
}

/// «Octubre de 2026»: el rotulo de un mes.
private struct RotuloDelMes: View {
    let cuando: Int64

    var body: some View {
        Text(TextosFicha.rotuloDelMes(cuando))
            .font(.system(size: 14, weight: .semibold))
            .foregroundStyle(Paleta.proyectos.apagado)
            .lineLimit(1)
            .padding(.leading, 16)
            .padding(.top, 4)
            .frame(maxWidth: .infinity, minHeight: MedidasFicha.mes, maxHeight: MedidasFicha.mes, alignment: .leading)
    }
}

// MARK: - Las miniaturas

/// Lo que se ve de una cosa: la foto, el dibujo o su tarjeta. Las fotos se
/// leen fuera del hilo principal, por la cache de siempre.
private struct VistaDeLaCosa: View {
    let cosa: CosaDeLaFicha
    let chat: String
    let almacen: Almacen
    /// Los pixeles del lado mayor que se piden de la foto.
    let pixeles: Int
    /// Recortada al cuadrado (la rejilla) o entera (la previa).
    var llenar = true
    @State private var foto: CGImage?

    private var m: MensajeChat { cosa.mensaje }

    /// Es una foto: la de un mensaje de foto (no un video) o la de una nota.
    private var esFoto: Bool { cosa.foto != nil || (m.clase == "IMAGEN" && !cosa.esVideo) }

    var body: some View {
        Group {
            if let foto {
                if llenar {
                    Color.clear.overlay {
                        Image(decorative: foto, scale: 2).resizable().interpolation(.high).scaledToFill()
                    }
                    .clipped()
                } else {
                    Image(decorative: foto, scale: 2).resizable().interpolation(.high).scaledToFit()
                }
            } else if m.clase == "DIBUJO", m.referencia != nil {
                // El dibujo, escalado a lo que haya (su miniatura es fija).
                GeometryReader { g in
                    let t = MiniaturaLienzo.tamano
                    let k = llenar ? max(g.size.width / t.width, g.size.height / t.height)
                        : min(g.size.width / t.width, g.size.height / t.height)
                    MiniaturaLienzo(almacen: almacen, chat: chat, mensaje: m)
                        .scaleEffect(max(k, 0.01))
                        .frame(width: g.size.width, height: g.size.height)
                }
                .clipped()
                .allowsHitTesting(false)
            } else if cosa.esVideo {
                RecuadroConIcono(simbolo: "video")
            } else if esFoto {
                // La foto que carga, o que aun no esta en el aparato.
                RecuadroConIcono(simbolo: "photo")
            } else {
                TarjetaDeLaCosa(mensaje: m)
            }
        }
        .task(id: ModeloFichaDelChat.clave(cosa)) { await cargar() }
    }

    private func cargar() async {
        if cosa.foto != nil {
            guard let ruta = cosa.rutaFoto else { return }
            foto = await Miniaturas.compartidas.cargar(URL(fileURLWithPath: ruta), lado: pixeles)
            return
        }
        if m.clase == "IMAGEN", !cosa.esVideo, let url = await resolverFichero(m, chat, almacen) {
            foto = await Miniaturas.compartidas.cargar(url, lado: pixeles)
        }
    }
}

/// El recuadro neutro de lo que aun no se puede ensenar, con su icono en
/// medio.
private struct RecuadroConIcono: View {
    let simbolo: String

    var body: some View {
        GeometryReader { g in
            let lado = min(max(min(g.size.width, g.size.height) * 0.3, 1), 40)
            ZStack {
                Paleta.proyectos.buscador
                Image(systemName: simbolo)
                    .font(.system(size: lado * 0.8))
                    .foregroundStyle(Paleta.proyectos.apagado.opacity(0.7))
            }
        }
    }
}

/// La tarjeta de lo que no tiene vista previa: la chapa de color con su
/// extension (`PDF`, `MP4`…) o el icono de su clase.
private struct TarjetaDeLaCosa: View {
    let mensaje: MensajeChat

    var body: some View {
        GeometryReader { g in
            let lado = min(g.size.width, g.size.height)
            let ext = ((mensaje.nombre.isEmpty ? (mensaje.ruta ?? "") : mensaje.nombre) as NSString).pathExtension
            ZStack {
                Paleta.proyectos.filaEncima
                if !ext.isEmpty {
                    CuadroExtensionChat(ext: ext, lado: max(lado * 0.42, 20))
                } else {
                    Image(systemName: BurbujaChat.icono(mensaje.clase))
                        .font(.system(size: max(lado * 0.3, 14)))
                        .foregroundStyle(Paleta.proyectos.apagado)
                }
            }
        }
    }
}

/// La insignia de un video, abajo a la izquierda: el triangulo y lo que
/// dura (si se sabe), en una pildora oscura.
private struct InsigniaDeVideo: View {
    let duracionMs: Int64

    var body: some View {
        let dura = duracionEnLaFicha(ms: duracionMs)
        HStack(spacing: 3) {
            Image(systemName: "play.fill").font(.system(size: 9))
            if !dura.isEmpty { Text(dura).font(.system(size: 11)).monospacedDigit() }
        }
        .foregroundStyle(.white)
        .padding(.leading, 5)
        .padding(.trailing, dura.isEmpty ? 6 : 8)
        .padding(.vertical, 3)
        .background(Color.black.opacity(0.6), in: Capsule())
        .padding(6)
    }
}

/// Una celda de la rejilla, cuadrada.
private struct CeldaDeLaFicha: View {
    let cosa: CosaDeLaFicha
    let chat: String
    let almacen: Almacen

    var body: some View {
        Color.clear
            .aspectRatio(1, contentMode: .fit)
            .overlay { VistaDeLaCosa(cosa: cosa, chat: chat, almacen: almacen, pixeles: 400) }
            .clipped()
            .overlay(alignment: .bottomLeading) {
                if cosa.esVideo { InsigniaDeVideo(duracionMs: cosa.mensaje.duracionMs) }
            }
            .contentShape(Rectangle())
            .hoverEffect(.highlight)
            .accessibilityLabel(TextosFicha.nombre(cosa))
    }
}

/// **Una fila**: archivos, enlaces, voz, notas. A la izquierda su chapa de
/// 44; a la derecha el nombre (cortado con «…» si no cabe) y, apagado, de
/// que es, cuanto pesa y cuando. Un enlace ensena su sitio, su direccion y
/// lo que decia el mensaje; una nota de voz, su boton y lo que dura.
private struct FilaDeLaFicha: View {
    let cosa: CosaDeLaFicha
    let seccion: SeccionFicha
    let buscado: String
    let chat: String
    let almacen: Almacen
    @ObservedObject private var voz = ReproductorVoz.compartido

    private var m: MensajeChat { cosa.mensaje }
    private var enlace: EnlaceFicha? { m.ruta == nil ? cosa.enlace : nil }

    var body: some View {
        HStack(spacing: 12) {
            chapa
                .frame(width: MedidasFicha.miniaturaFila, height: MedidasFicha.miniaturaFila)
                .clipShape(RoundedRectangle(cornerRadius: 10))
            VStack(alignment: .leading, spacing: 2) { lineas }
                .lineLimit(1)
                .truncationMode(.tail)
            Spacer(minLength: 0)
            if m.fijado { Image(systemName: "flag.fill").foregroundStyle(Paleta.proyectos.apagado) }
        }
        .padding(.horizontal, 8)
        .frame(height: enlace != nil ? MedidasFicha.filaEnlace : MedidasFicha.fila)
        .contentShape(Rectangle())
        .hoverEffect(.highlight)
    }

    private var cuando: String { TextosFicha.hora(m.cuando) }

    @ViewBuilder
    private var lineas: some View {
        if let e = enlace {
            // El sitio en negrita, la direccion y, si el mensaje decia algo
            // mas, eso.
            Text(e.donde)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(Paleta.proyectos.texto)
            Text(e.url)
                .font(.system(size: 13))
                .foregroundStyle(Paleta.chat.enviar)
            let resto = m.texto.replacingOccurrences(of: e.url, with: " ")
                .split(whereSeparator: \.isWhitespace).joined(separator: " ")
            Text(resto.isEmpty ? cuando : resto)
                .font(.system(size: 13))
                .foregroundStyle(Paleta.proyectos.apagado)
        } else if m.clase == "VOZ" {
            Text(m.nombre.isEmpty ? TextosFicha.notaDeVoz : TextosFicha.nombre(m))
                .font(.system(size: 15))
                .foregroundStyle(Paleta.proyectos.texto)
            let dura = duracionEnLaFicha(ms: m.duracionMs)
            Text(((dura.isEmpty ? [] : [dura]) + [cuando]).joined(separator: "  ·  "))
                .font(.system(size: 13))
                .foregroundStyle(Paleta.proyectos.apagado)
        } else {
            let nombre = seccion == .archivos && !m.nombre.isEmpty ? m.nombre : TextosFicha.nombre(m)
            Text(TextosFicha.conNegritas(nombre, buscado: buscado, tam: 15))
                .foregroundStyle(Paleta.proyectos.texto)
            Text(segunda)
                .font(.system(size: 13))
                .foregroundStyle(Paleta.proyectos.apagado)
        }
    }

    /// «de que es · cuanto pesa · cuando»; en «Archivos», sin el de que es.
    private var segunda: String {
        var partes: [String] = []
        if seccion != .archivos, let k = TextosFicha.clase(m.clase) { partes.append(k) }
        if m.bytes > 0 { partes.append(TextosFicha.tamano(m.bytes)) }
        partes.append(cuando)
        return partes.joined(separator: "  ·  ")
    }

    @ViewBuilder
    private var chapa: some View {
        if let e = enlace {
            // La letra de su sitio, en el color de su sitio.
            RoundedRectangle(cornerRadius: 10).fill(Paleta.avatar(e.donde))
                .overlay {
                    Text(e.donde.prefix(1).uppercased())
                        .font(.system(size: MedidasFicha.miniaturaFila * 0.45))
                        .foregroundStyle(.white)
                }
        } else if m.clase == "VOZ" {
            // Su boton: pausa si es la que suena.
            let suena = voz.actual == m.id && voz.sonando
            circulo(suena ? "pause.fill" : "play.fill", Paleta.chat.enviar)
        } else if m.clase == "IMAGEN", m.ruta != nil {
            VistaDeLaCosa(cosa: cosa, chat: chat, almacen: almacen, pixeles: 120)
        } else if let ext = extension_, !ext.isEmpty {
            CuadroExtensionChat(ext: ext, lado: MedidasFicha.miniaturaFila)
        } else {
            circulo(m.clase == "DIBUJO" ? "scribble.variable" : m.clase == "PAGINA" ? "book" : "doc.text",
                    Paleta.proyectos.apagado.opacity(0.6))
        }
    }

    private var extension_: String? {
        let n = m.nombre.isEmpty ? (m.ruta ?? "") : m.nombre
        return n.isEmpty ? nil : (n as NSString).pathExtension
    }

    private func circulo(_ simbolo: String, _ color: Color) -> some View {
        Circle().fill(color)
            .overlay {
                Image(systemName: simbolo).font(.system(size: 18)).foregroundStyle(.white)
            }
    }
}

/// **Un proyecto de su pestana**: su avatar (o su logo), su nombre en
/// seminegrita con lo buscado resaltado y, a la derecha, cuando se toco;
/// debajo, cuantas hojas tiene y su ultima linea. El de este chat va
/// marcado como la fila elegida de la lista y lo dice delante.
private struct FilaDeProyecto: View {
    let proyecto: ProyectoDeLaFicha
    let buscado: String
    let nucleo: Nucleo
    @ObservedObject private var logos = LogosProyecto.compartidos

    private var f: FichaChat { proyecto.ficha }

    var body: some View {
        let (texto, apagado) = proyecto.este ? (Color.white, Color.white)
            : (Paleta.proyectos.texto, Paleta.proyectos.apagado)
        HStack(spacing: 12) {
            AvatarProyecto(chat: f, logo: logos.logo(f.id), lado: MedidasFicha.miniaturaFila)
                .onAppear { logos.mirar(f.id, nucleo: nucleo) }
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 8) {
                    Text(TextosFicha.conNegritas(f.nombre, buscado: buscado, tam: 15, peso: .semibold))
                        .foregroundStyle(texto)
                    Spacer(minLength: 0)
                    Text(TextosFicha.hora(f.tocado))
                        .font(.system(size: 13))
                        .foregroundStyle(apagado)
                }
                Text(debajo)
                    .font(.system(size: 13))
                    .foregroundStyle(apagado)
            }
            .lineLimit(1)
            .truncationMode(.tail)
        }
        .padding(.horizontal, 8)
        .frame(height: MedidasFicha.fila)
        .background(proyecto.este ? Paleta.proyectos.filaElegida : .clear, in: RoundedRectangle(cornerRadius: 10))
        .contentShape(Rectangle())
        .hoverEffect(.highlight)
    }

    /// «Este chat · 12 hojas · lo ultimo que se dijo». Guardado en un disco
    /// que ahora no esta, eso manda: es lo que explica por que no abre.
    private var debajo: String {
        var partes: [String] = []
        if proyecto.este { partes.append(TextosFicha.esteChat) }
        if !proyecto.disponible {
            partes.append(TextosFicha.noDisponible)
        } else {
            partes.append(TextosFicha.hojas(f.hojas))
            let resumen = f.resumen.split(separator: "\n", maxSplits: 1).first
                .map { $0.trimmingCharacters(in: .whitespaces) } ?? ""
            if !resumen.isEmpty { partes.append(resumen) }
        }
        return partes.joined(separator: "  ·  ")
    }
}

// MARK: - La previa

/// **La previa de una cosa**, en grande dentro de la hoja: una foto sobre
/// fondo oscuro, entera y sin deformar; lo demas en su tarjeta. Las flechas
/// van encima, a los lados, solo si hay a donde ir; deslizar el dedo hace
/// lo mismo.
private struct PreviaDeLaFicha: View {
    @ObservedObject var ficha: ModeloFichaDelChat
    let cosa: CosaDeLaFicha

    private var m: MensajeChat { cosa.mensaje }

    var body: some View {
        vista
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .overlay(alignment: .leading) { flecha("chevron.left", -1) }
            .overlay(alignment: .trailing) { flecha("chevron.right", 1) }
            .padding(MedidasFicha.margenPrevia)
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 30).onEnded { g in
                    let dx = g.translation.width
                    guard abs(dx) > abs(g.translation.height) else { return }
                    ficha.mover(dx < 0 ? 1 : -1)
                }
            )
    }

    @ViewBuilder
    private var vista: some View {
        if cosa.foto != nil || (m.clase == "IMAGEN" && !cosa.esVideo) {
            // La foto sobre su fondo oscuro, con bandas donde no llena.
            VistaDeLaCosa(cosa: cosa, chat: ficha.chat, almacen: ficha.almacen, pixeles: 1600, llenar: false)
                .id(ModeloFichaDelChat.clave(cosa))
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color.black.opacity(0.88))
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .onTapGesture(count: 2) { ficha.abrirLaPrevia() }
        } else if m.clase == "DIBUJO" {
            VistaDeLaCosa(cosa: cosa, chat: ficha.chat, almacen: ficha.almacen, pixeles: 1600, llenar: false)
                .id(m.id)
                .clipShape(RoundedRectangle(cornerRadius: 12))
        } else if m.clase == "NOTA" || m.clase.isEmpty {
            // Una nota (o un enlace): su texto, en una tarjeta.
            ScrollView {
                Text(m.texto)
                    .font(.system(size: 17))
                    .foregroundStyle(Paleta.proyectos.texto)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(18)
            }
            .background(Paleta.proyectos.filaEncima, in: RoundedRectangle(cornerRadius: 12))
        } else {
            // Lo demas: su tarjeta en un cuadrado y debajo su nombre entero.
            GeometryReader { g in
                let lado = max(min(g.size.width, g.size.height) * 0.45, 1)
                VStack(spacing: 14) {
                    TarjetaDeLaCosa(mensaje: m)
                        .frame(width: lado, height: lado)
                        .clipShape(RoundedRectangle(cornerRadius: 16))
                        .overlay(alignment: .bottomLeading) {
                            if cosa.esVideo { InsigniaDeVideo(duracionMs: m.duracionMs) }
                        }
                    Text(m.nombre.isEmpty ? TextosFicha.nombre(m) : m.nombre)
                        .font(.system(size: 15))
                        .foregroundStyle(Paleta.proyectos.texto)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                .frame(width: g.size.width, height: g.size.height)
            }
        }
    }

    @ViewBuilder
    private func flecha(_ simbolo: String, _ paso: Int) -> some View {
        if ficha.hayVecina(paso) {
            BotonFlecha(simbolo: simbolo, ayuda: paso < 0 ? "Anterior" : "Siguiente") { ficha.mover(paso) }
                .padding(.horizontal, 8)
        }
    }
}

/// Una flecha de la previa: un circulo oscuro con el chevron blanco.
private struct BotonFlecha: View {
    let simbolo: String
    let ayuda: String
    let accion: () -> Void

    var body: some View {
        Button(action: accion) {
            Image(systemName: simbolo)
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: MedidasFicha.flecha, height: MedidasFicha.flecha)
                .background(Color.black.opacity(0.45), in: Circle())
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .hoverEffect(.highlight)
        .accessibilityLabel(ayuda)
    }
}
