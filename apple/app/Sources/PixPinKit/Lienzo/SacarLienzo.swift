// **Sacar el dibujo** en el iPad: compartir en los formatos de Windows,
// copiar como PNG e imprimir, como el grupo `sacar` de la barra de Windows y
// su hoja de compartir (`compartir.rs`: «HTML», «PDF», «PNG», «JPG», «SVG» y
// «.pixpin», en ese orden).
//
// - Las hojas (todo, lo elegido o cada marco) las decide el motor
//   (`hojasParaExportar`, `editor_exportar.rs`); el SVG lo escribe el motor
//   (`svgParaExportar`), el mismo de Windows y del movil, con las fotos y el
//   papel dentro.
// - **La pagina web** es la de Windows y el movil (`paginaWebCompartida`):
//   un solo archivo con sus mandos (lapiz, medir, presentar, imprimir) que
//   se vuelve a anotar, y con el `.excalidraw` dentro para volver a editar
//   el dibujo.
// - **El `.pixpin`** es el proyecto entero del chat del lienzo
//   (`paqueteDeProyecto`, el `Entero::Proyectos` de Windows): se abre en
//   otro PixPin o en el movil con el lienzo dentro, editable.
// - El PNG, el JPG, el PDF (vectorial) y lo impreso los pinta el mismo
//   `PintorLienzo` de la pantalla (`PintorDeHojas`), con el fondo fijo
//   debajo si lo hay.
//
// Lo que sale de fabrica: lo elegido si hay algo elegido; si no, en un
// formato de una hoja (PNG, JPG, SVG) el lienzo entero, y en los de varias
// (HTML, PDF) el lienzo entero y detras cada marco, como la hoja de Windows.
//
// Compartir va por la hoja del sistema (`UIActivityViewController`: Guardar
// en Archivos, AirDrop, Mensajes, Mail...); imprimir, por la del sistema
// (`UIPrintInteractionController`), con un PDF vectorial.

import ImageIO
import PixPinCore
import UIKit
import UniformTypeIdentifiers

/// Los formatos que salen, en el orden de la hoja de Windows.
public enum FormatoSalidaLienzo: String, CaseIterable {
    case web, pdf, png, jpg, svg, pixpin

    /// El nombre de la hoja de Windows (`compartir-web`, `compartir-pdf`...).
    public var titulo: String {
        switch self {
        case .web: return "HTML"
        case .pdf: return "PDF"
        case .png: return "PNG"
        case .jpg: return "JPG"
        case .svg: return "SVG"
        case .pixpin: return ".pixpin"
        }
    }

    /// Lo que se le explica a quien elige.
    public var detalle: String {
        switch self {
        case .web: return "Página web que se vuelve a anotar"
        case .pdf: return "Documento PDF, una página por hoja"
        case .png: return "Imagen PNG"
        case .jpg: return "Imagen JPG"
        case .svg: return "Dibujo SVG"
        case .pixpin: return "El proyecto entero, editable en otro PixPin o el móvil"
        }
    }

    public var sufijo: String {
        switch self {
        case .web: return "html"
        case .jpg: return "jpg"
        default: return rawValue
        }
    }

    /// Cuantas hojas lleva: los de una sola (una imagen, un SVG) sacan una.
    public var variasHojas: Bool { self == .web || self == .pdf }
}

extension HojaExportada {
    var caja: CGRect { CGRect(x: CGFloat(x0), y: CGFloat(y0), width: CGFloat(x1 - x0), height: CGFloat(y1 - y0)) }
}

extension ColorLienzo {
    /// `#rrggbb` (lo que la pagina web pone detras de cada hoja).
    var hex: String {
        func octeto(_ v: CGFloat) -> Int { Int((min(1, max(0, v)) * 255).rounded()) }
        return String(format: "#%02x%02x%02x", octeto(r), octeto(g), octeto(b))
    }
}

/// Lo que se hace con las hojas, sin pantalla (se prueba sola).
enum SacarLienzo {
    /// **Que hojas** salen en `formato`: lo elegido si hay algo elegido; si
    /// no, el lienzo entero y, en los de varias hojas, cada marco detras
    /// (si los hay).
    static func ques(_ formato: FormatoSalidaLienzo, hayElegidos: Bool, hayMarcos: Bool) -> [QueExportar] {
        if hayElegidos { return [.seleccion] }
        if formato.variasHojas, hayMarcos { return [.todo, .marcos] }
        return [.todo]
    }

    /// El nombre del fichero `i` de `n` (con su numero detras si hay
    /// varios, `ruta_numerada` de Windows), con la extension del formato.
    static func nombre(_ base: String, _ i: Int, de n: Int, extension ext: String) -> String {
        var limpio = nombreDeFicheroCompartido(titulo: base).trimmingCharacters(in: .whitespaces)
        if limpio.isEmpty { limpio = "Lienzo" }
        return (n > 1 ? "\(limpio)-\(i + 1)" : limpio) + "." + ext
    }

    /// Una imagen como JPG (sin transparencia: lleva el papel pintado).
    static func jpg(_ img: CGImage, calidad: CGFloat = 0.92) -> Data? {
        let datos = NSMutableData()
        guard let d = CGImageDestinationCreateWithData(datos, UTType.jpeg.identifier as CFString, 1, nil) else {
            return nil
        }
        CGImageDestinationAddImage(d, img, [kCGImageDestinationLossyCompressionQuality: calidad] as CFDictionary)
        return CGImageDestinationFinalize(d) ? datos as Data : nil
    }
}

extension ModeloEditorLienzo {
    /// Hay marcos con nombre (o mas de uno): «cada marco» es otra cosa que
    /// el lienzo entero.
    var hayMarcos: Bool {
        let m = editor.hojasParaExportar(que: .marcos)
        return m.count > 1 || (m.count == 1 && !m[0].nombre.isEmpty)
    }

    /// Las hojas de `ques`, una detras de otra.
    func hojas(_ ques: [QueExportar]) -> [HojaExportada] {
        ques.flatMap { editor.hojasParaExportar(que: $0) }
    }

    /// El papel del fondo fijo, para que el SVG lo lleve dentro.
    private func ponerPapelConImagen() {
        if let fondo {
            editor.ponerPapel(ancho: Float(fondo.tamano.width), alto: Float(fondo.tamano.height), png: fondo.png())
        }
    }

    /// Una hoja pintada (a 2 pixeles por unidad, como la exportacion de
    /// Windows).
    func imagen(de h: HojaExportada, transparente: Bool = false, escala: CGFloat = 2) -> CGImage? {
        PintorDeHojas.imagen(h.plano, caja: h.caja, escala: escala, transparente: transparente,
                             tapados: tapados, version: editor.version(), fotos: { [weak self] in self?.foto($0) })
    }

    /// **Un PDF vectorial** con una pagina por hoja, del tamano de la hoja.
    func pdf(_ hojas: [HojaExportada], transparente: Bool = false) -> Data? {
        guard !hojas.isEmpty else { return nil }
        let datos = NSMutableData()
        guard let consumidor = CGDataConsumer(data: datos as CFMutableData),
              let ctx = CGContext(consumer: consumidor, mediaBox: nil, nil) else { return nil }
        let version = editor.version()
        for h in hojas where h.caja.width > 0 && h.caja.height > 0 {
            var pagina = CGRect(origin: .zero, size: h.caja.size)
            ctx.beginPage(mediaBox: &pagina)
            ctx.translateBy(x: 0, y: pagina.height)
            ctx.scaleBy(x: 1, y: -1)
            PintorDeHojas.pintar(h.plano, caja: h.caja, en: ctx, destino: pagina, transparente: transparente,
                                 tapados: tapados, version: version, fotos: { [weak self] in self?.foto($0) })
            ctx.endPage()
        }
        ctx.closePDF()
        return datos as Data
    }

    /// El dibujo como `.excalidraw`, para que viaje dentro de la pagina web:
    /// el limpio del nucleo si es del chat (ya guardado) o el fichero tal
    /// cual.
    private func excalidrawParaLaWeb() -> String? {
        ModeloEditorLienzo.guardarYa(editor, referencia: referencia)
        if let chat, let referencia, let nucleo, let json = try? nucleo.excalidrawDeDibujo(chat: chat, dibujo: referencia) {
            return json
        }
        return try? String(contentsOfFile: editor.ruta(), encoding: .utf8)
    }

    /// **La pagina web** de las hojas de `ques`, con el dibujo dentro (solo
    /// si va el lienzo entero: lo elegido no se vuelve a editar entero).
    func paginaWeb(_ ques: [QueExportar]) -> String? {
        ponerPapelConImagen()
        var hojas: [HojaParaWeb] = []
        for q in ques {
            let svgs = editor.svgParaExportar(que: q, transparente: false)
            let datos = editor.hojasParaExportar(que: q)
            for (i, svg) in svgs.enumerated() {
                let h = i < datos.count ? datos[i] : nil
                let fondo = h.flatMap { try? leerLienzo($0.plano) }?.fondo.hex ?? "#ffffff"
                var nombre = h?.nombre ?? ""
                if nombre.isEmpty { nombre = q == .todo ? "Lienzo completo" : titulo }
                hojas.append(HojaParaWeb(nombre: nombre, svg: svg, fondo: fondo))
            }
        }
        let editable = ques.contains(.todo) ? excalidrawParaLaWeb() : nil
        return paginaWebCompartida(titulo: titulo, hojas: hojas, excalidraw: editable)
    }

    /// **Los ficheros de `formato`** en `carpeta`: uno por hoja en los de una
    /// hoja (solo hay una), uno con todas en los demas. Vacio si no hay nada.
    /// El `.pixpin` va aparte (`pixpin(en:)`): pesa y se hace fuera del hilo
    /// principal.
    func ficheros(_ formato: FormatoSalidaLienzo, en carpeta: URL) -> [URL] {
        _ = editor.terminarTexto()
        let ques = SacarLienzo.ques(formato, hayElegidos: editor.cuantosElegidos() > 0, hayMarcos: hayMarcos)
        var salida: [(String, Data)] = []
        switch formato {
        case .web:
            if let html = paginaWeb(ques) {
                salida.append((SacarLienzo.nombre(titulo, 0, de: 1, extension: "html"), Data(html.utf8)))
            }
        case .pdf:
            if let d = pdf(hojas(ques)) { salida.append((SacarLienzo.nombre(titulo, 0, de: 1, extension: "pdf"), d)) }
        case .png, .jpg:
            let todas = hojas(ques)
            for (i, h) in todas.enumerated() {
                guard let img = imagen(de: h),
                      let d = formato == .png ? PintorDeHojas.png(img) : SacarLienzo.jpg(img) else { continue }
                salida.append((SacarLienzo.nombre(h.nombre.isEmpty ? titulo : h.nombre, i, de: todas.count,
                                                  extension: formato.sufijo), d))
            }
        case .svg:
            ponerPapelConImagen()
            let svgs = ques.flatMap { editor.svgParaExportar(que: $0, transparente: false) }
            for (i, s) in svgs.enumerated() {
                salida.append((SacarLienzo.nombre(titulo, i, de: svgs.count, extension: "svg"), Data(s.utf8)))
            }
        case .pixpin:
            return []
        }
        return salida.compactMap { nombre, datos in
            let url = carpeta.appendingPathComponent(nombre)
            return (try? datos.write(to: url, options: .atomic)) != nil ? url : nil
        }
    }

    /// Una carpeta temporal nueva para lo que se comparte.
    private func carpetaTemporal() -> URL {
        let carpeta = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-compartir-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
        return carpeta
    }

    /// Donde cuelgan la hoja y el menu: el boton de compartir de la barra
    /// del visor (arriba a la derecha).
    private func ancla(_ vista: UIView) -> CGRect {
        CGRect(x: max(0, vista.bounds.maxX - 150), y: 30, width: 1, height: 1)
    }

    // MARK: Compartir

    /// **Compartir…** (⇧⌘S): que formato, como la hoja de Windows, y despues
    /// la hoja del sistema con el fichero.
    func compartir() {
        desplegado = nil
        guard let vista else { return }
        _ = editor.terminarTexto()
        guard !editor.hojasParaExportar(que: .todo).isEmpty || fondo != nil else {
            CajetinesLienzo.avisar("Compartir", "No hay nada que compartir aquí.", desde: vista)
            return
        }
        let menu = UIAlertController(title: "Compartir",
                                     message: editor.cuantosElegidos() > 0 ? "Lo elegido" : nil,
                                     preferredStyle: .actionSheet)
        for f in FormatoSalidaLienzo.allCases {
            if f == .pixpin, chat == nil || nucleo == nil { continue }
            menu.addAction(UIAlertAction(title: "\(f.titulo) — \(f.detalle)", style: .default) { [weak self] _ in
                self?.compartir(como: f)
            })
        }
        menu.addAction(UIAlertAction(title: "Copiar como PNG", style: .default) { [weak self] _ in
            self?.copiarComoPNG()
        })
        menu.addAction(UIAlertAction(title: "Imprimir…", style: .default) { [weak self] _ in self?.imprimir() })
        menu.addAction(UIAlertAction(title: "Cancelar", style: .cancel))
        if let pop = menu.popoverPresentationController {
            pop.sourceView = vista
            pop.sourceRect = ancla(vista)
        }
        CajetinesLienzo.controlador(desde: vista)?.present(menu, animated: true)
    }

    /// Compartir en `formato`, ya elegido.
    func compartir(como formato: FormatoSalidaLienzo) {
        guard let vista else { return }
        let carpeta = carpetaTemporal()
        if formato == .pixpin {
            guard let chat, let nucleo else { return }
            ModeloEditorLienzo.guardarYa(editor, referencia: referencia)
            let nombre = (try? nucleo.chats())?.first { $0.id == chat }?.nombre ?? titulo
            Task { [weak self] in
                let url = await Task.detached(priority: .userInitiated) { () -> URL? in
                    guard let datos = try? nucleo.paqueteDeProyecto(chat: chat) else { return nil }
                    let url = carpeta.appendingPathComponent(SacarLienzo.nombre(nombre, 0, de: 1, extension: "pixpin"))
                    return (try? datos.write(to: url, options: .atomic)) != nil ? url : nil
                }.value
                guard let self, let vista = self.vista else { return }
                guard let url else {
                    CajetinesLienzo.avisar("Compartir", "No se pudo preparar el .pixpin del proyecto.", desde: vista)
                    return
                }
                self.hojaDelSistema([url], carpeta: carpeta, desde: vista)
            }
            return
        }
        let urls = ficheros(formato, en: carpeta)
        guard !urls.isEmpty else {
            try? FileManager.default.removeItem(at: carpeta)
            CajetinesLienzo.avisar("Compartir", "No se pudo preparar.", desde: vista)
            return
        }
        hojaDelSistema(urls, carpeta: carpeta, desde: vista)
    }

    /// La hoja de compartir del sistema con estos ficheros; la carpeta se
    /// borra al cerrarla.
    private func hojaDelSistema(_ urls: [URL], carpeta: URL, desde vista: UIView) {
        let hoja = UIActivityViewController(activityItems: urls, applicationActivities: nil)
        hoja.completionWithItemsHandler = { _, _, _, _ in
            try? FileManager.default.removeItem(at: carpeta)
        }
        if let pop = hoja.popoverPresentationController {
            pop.sourceView = vista
            pop.sourceRect = ancla(vista)
        }
        CajetinesLienzo.controlador(desde: vista)?.present(hoja, animated: true)
    }

    /// **Copiar como PNG** (⇧⌘C): lo elegido o todo, al portapapeles.
    func copiarComoPNG() {
        desplegado = nil
        _ = editor.terminarTexto()
        let que: QueExportar = editor.cuantosElegidos() > 0 ? .seleccion : .todo
        guard let h = editor.hojasParaExportar(que: que).first, let img = imagen(de: h),
              let png = PintorDeHojas.png(img) else {
            UINotificationFeedbackGenerator().notificationOccurred(.error)
            return
        }
        UIPasteboard.general.setData(png, forPasteboardType: UTType.png.identifier)
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }

    /// El dibujo (o lo elegido) pintado, para fijarlo como pin.
    func imagenParaPin() -> CGImage? {
        _ = editor.terminarTexto()
        let que: QueExportar = editor.cuantosElegidos() > 0 ? .seleccion : .todo
        return editor.hojasParaExportar(que: que).first.flatMap { imagen(de: $0) }
    }

    // MARK: La zona, al chat

    /// **La foto de una zona, al chat del proyecto** (`zona_al_chat.rs` de
    /// Windows, `mandarLaZona` del movil): va como **sublienzo** (la foto
    /// clavada en su propio lienzo, hoja del proyecto detras de este, con
    /// «Viene de: Lienzo «…»») y aqui queda la marca azul con su enlace,
    /// en un paso de deshacer. Tocar el icono de la marca lleva a el.
    /// `png` es la foto tal cual (sin redondear: solo se redondea la copia)
    /// y `caja` la zona en el mundo.
    func mandarZonaAlChat(_ png: Data, caja: CGRect) {
        guard let chat, let nucleo, let vista, let dibujo = referencia else { return }
        let tam = ImagenPegadaLienzo.tamano(de: png) ?? CGSize(width: caja.width, height: caja.height)
        let nombre = titulo
        let editor = self.editor
        Task { [weak self] in
            let hecho = await Task.detached(priority: .userInitiated) { () -> ZonaMandada? in
                try? nucleo.zonaAlChat(chat: chat, dibujo: dibujo, pagina: nil, nombre: nombre, png: png,
                                       ancho: UInt32(max(1, tam.width)), alto: UInt32(max(1, tam.height)))
            }.value
            guard let self else { return }
            guard let z = hecho else {
                CajetinesLienzo.avisar("Zona al chat", "No se pudo mandar la zona al chat.", desde: vista)
                return
            }
            if editor.marcarZona(x0: Float(caja.minX), y0: Float(caja.minY), x1: Float(caja.maxX),
                                 y1: Float(caja.maxY), dibujo: z.dibujo) {
                self.vista?.trasCambio()
            }
            UINotificationFeedbackGenerator().notificationOccurred(.success)
            // El chat se vuelve a leer (como cuando llegan cambios) y la
            // sincronizacion cuenta su calma desde aqui.
            NotificationCenter.default.post(name: .pixpinTrajoCambios, object: nil)
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        }
    }

    /// **La foto de una zona, al chat, sin marca** (la usan los planos, que
    /// no son un lienzo del proyecto): entra como una foto, con «Viene de» en
    /// su descripcion.
    func mandarZonaAlChat(_ png: Data) {
        guard let chat, let nucleo, let vista else { return }
        let carpeta = carpetaTemporal()
        let nombre = SacarLienzo.nombre("Zona de \(titulo)", 0, de: 1, extension: "png")
        let url = carpeta.appendingPathComponent(nombre)
        let descripcion = "Viene de: Lienzo «\(titulo)»"
        Task { [weak self] in
            let hecho = await Task.detached(priority: .userInitiated) { () -> Bool in
                defer { try? FileManager.default.removeItem(at: carpeta) }
                guard (try? png.write(to: url, options: .atomic)) != nil,
                      let m = try? nucleo.adjuntar(chat: chat, rutaOrigen: url.path, nombre: nombre) else { return false }
                _ = try? nucleo.cambiarMensaje(chat: chat, id: m.id, cambio: .descripcion(texto: descripcion))
                return true
            }.value
            guard self != nil else { return }
            if hecho {
                UINotificationFeedbackGenerator().notificationOccurred(.success)
                NotificationCenter.default.post(name: .pixpinTrajoCambios, object: nil)
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            } else {
                CajetinesLienzo.avisar("Zona al chat", "No se pudo mandar la zona al chat.", desde: vista)
            }
        }
    }

    // MARK: Imprimir

    /// **Imprimir…** (⌘P): un PDF vectorial con una hoja por pagina: lo
    /// elegido, cada marco si hay marcos o el lienzo entero (`imprimir-que`
    /// de Windows). El dialogo del sistema lo encaja en el papel.
    func imprimir() {
        desplegado = nil
        guard let vista else { return }
        _ = editor.terminarTexto()
        let que: QueExportar = editor.cuantosElegidos() > 0 ? .seleccion : (hayMarcos ? .marcos : .todo)
        guard let datos = pdf(editor.hojasParaExportar(que: que)) else {
            CajetinesLienzo.avisar("Imprimir", "No hay nada dibujado que imprimir.", desde: vista)
            return
        }
        let impresora = UIPrintInteractionController.shared
        let info = UIPrintInfo.printInfo()
        info.jobName = titulo
        info.outputType = .general
        impresora.printInfo = info
        impresora.printingItem = datos
        _ = impresora.present(from: ancla(vista), in: vista, animated: true, completionHandler: nil)
    }
}
