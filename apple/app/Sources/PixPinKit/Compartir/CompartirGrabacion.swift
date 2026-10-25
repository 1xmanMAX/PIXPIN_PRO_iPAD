// **Compartir una grabacion** (Windows 2921a1d, `compartir/clase.rs`; el
// usuario: «en las grabaciones, que el menu de compartir sea el de toda la
// app, con los mismos formatos; sobre todo el HTML, que se vea claro e
// intuitivo»).
//
// Una nota de voz sola (una clase, una reunion o una nota cualquiera), o la
// carpeta de una grabacion, entra en la hoja de compartir de siempre con lo
// de siempre y con lo suyo. Todo lo hace el nucleo
// (`pixpin-nucleo/src/compartir_grabacion.rs`); aqui solo se pone en la
// hoja:
//
// - **HTML**: la pagina con el reproductor, la transcripcion que salta y se
//   resalta, las capturas en su minuto y los marcadores (entera, sin elegir
//   paginas), con el video al lado;
// - **PDF**, **PNG**, **JPG** y **SVG**: sus hojas A4 (el titulo, los
//   marcadores y la transcripcion con las capturas en su minuto), las
//   ordenes de Windows pintadas con el pintor del lienzo;
// - **Word**, **Markdown** (con la carpeta de las capturas, en un ZIP),
//   **Texto** y, si tiene capturas, **Anki**;
// - **Original(es)**: el audio, el video, las capturas y lo adjunto; y el
//   `.pixpin` del chat.

import Foundation
import PixPinCore

extension PrepararCompartir {
    /// **La grabacion del mensaje** (una nota de voz o la carpeta de una
    /// grabacion), con su fecha en la hora de este iPad; `nil` si no es una.
    static func grabacion(_ m: MensajeChat, chat: String, almacen: Almacen) async -> GrabacionParaCompartir? {
        let nucleo = almacen.nucleo
        let id = m.id
        let leida = await Task.detached(priority: .userInitiated) {
            try? nucleo.grabacionParaCompartir(chat: chat, mensaje: id)
        }.value
        guard var g = leida else { return nil }
        if g.cuando > 0 { g.fecha = fechaDeGrabacion(g.cuando) }
        return g
    }

    /// La fecha como la escribe Windows (`grabacion-fecha`: «25/10/2026
    /// 18:05»), en la hora de este iPad.
    static func fechaDeGrabacion(_ ms: Int64) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = .current
        f.dateFormat = "dd/MM/yyyy HH:mm"
        return f.string(from: Date(timeIntervalSince1970: TimeInterval(ms) / 1000))
    }

    /// **Una grabacion en la hoja** (`clase::anadir` y `de_mensajes` de
    /// Windows): sus hojas, su pagina web, sus formatos propios, los
    /// originales y el `.pixpin` del chat.
    static func deGrabacion(_ g: GrabacionParaCompartir, chat: String, almacen: Almacen) async -> PreparadoCompartir {
        let titulo = g.titulo.trimmingCharacters(in: .whitespacesAndNewlines)
        let p = PreparadoCompartir(titulo: titulo.isEmpty ? "Grabación" : g.titulo)
        let leidas = await Task.detached(priority: .userInitiated) { grabacionHojas(g: g) }.value
        var rutas: [UInt64: URL] = [:]
        for f in leidas.fotos { rutas[f.idObjeto] = URL(fileURLWithPath: f.ruta) }
        let fotos = CacheDeFotos(rutas)
        for (i, h) in leidas.hojas.enumerated() {
            let pagina = i == 0
                ? PaginaCompartir(clave: "grabacion", nombre: p.titulo, detalle: "Grabación")
                : PaginaCompartir(clave: "grabacion|p\(i + 1)", nombre: "Página \(i + 1)", detalle: "", nivel: 1)
            var pieza = PiezasCompartir.dibujo(h, pagina: pagina, fotos: fotos)
            // La hoja de Windows es un A4 a 96 ppp: en el PDF, un A4 en puntos.
            pieza.tamano = PiezasCompartir.a4
            // El SVG lleva las capturas dentro: se hace al pedirlo.
            let indice = UInt32(i)
            pieza.svg = { grabacionSvgDeHoja(g: g, indice: indice) }
            p.anadir(pieza)
        }
        let base = p.base
        p.paginaWeb = { carpeta in
            try await enCarpeta(carpeta) { try grabacionWeb(g: g, base: base, carpeta: $0) }
        }
        p.extras.append(propio(IdFormato.word, "Word", "doc.text") { try grabacionWord(g: g, base: base, carpeta: $0) })
        p.extras.append(propio(IdFormato.md, "Markdown", "square.and.pencil") {
            try grabacionMarkdown(g: g, base: base, carpeta: $0)
        })
        p.extras.append(propio(IdFormato.texto, "Texto", "text.alignleft") {
            try grabacionTexto(g: g, base: base, carpeta: $0)
        })
        if !g.capturas.isEmpty {
            // Las tarjetas de estudio: el mismo icono que «Enviar a Anki».
            p.extras.append(propio(IdFormato.anki, "Anki", "book") {
                try grabacionAnki(g: g, base: base, carpeta: $0, pregunta: "¿Qué se explicó aquí?")
            })
        }
        anadirOriginales(p, g.originales.map { URL(fileURLWithPath: $0) })
        if let ficha = almacen.chats.first(where: { $0.id == chat }) {
            p.extras.append(paquete(almacen, [ficha], id: IdFormato.pixpin, nombre: ".pixpin"))
        }
        // Lo suyo es su pagina web, no mandar el audio tal cual.
        p.originalPrimero = false
        return p
    }

    /// Un formato propio de la grabacion: lo escribe el nucleo en la carpeta.
    private static func propio(_ id: String, _ nombre: String, _ simbolo: String,
                               _ hacer: @escaping @Sendable (String) throws -> [String]) -> ExtraCompartir {
        ExtraCompartir(formato: FormatoCompartir(id: id, nombre: nombre, cuantas: .ninguna, simbolo: simbolo),
                       que: .lote { carpeta in try await enCarpeta(carpeta, hacer) })
    }

    /// Lo que escribe el nucleo en `carpeta`, fuera del hilo principal.
    private static func enCarpeta(_ carpeta: URL,
                                  _ hacer: @escaping @Sendable (String) throws -> [String]) async throws -> [URL] {
        let ruta = carpeta.path
        let hechas = try await Task.detached(priority: .userInitiated) { try hacer(ruta) }.value
        return hechas.map { URL(fileURLWithPath: $0) }
    }
}
