// **Preparar lo que se comparte** (D1): de cada cosa, sus paginas y lo que
// va entero, como `preparar` de Windows (`de_mensajes`, `de_proyectos`,
// `anadir_mensaje`, `anadir_lienzo`):
//
// - un **dibujo** (o una pagina de PDF anotada): el lienzo entero y cada
//   marco debajo, sangrado; al abrir van marcados los marcos y no el
//   entero, que los repetiria. lo editable es el `.pixpin`;
// - una **foto**: la foto;
// - un **fichero**: solo, un PDF en paginas; entre otros mensajes va entero,
//   en «Originales» (un PDF de trescientas paginas no puede colarse en el
//   PDF de un chat);
// - una **nota de voz**: su texto, si ya se paso;
// - una **nota** o lo demas: su texto en paginas A4 y, sola, el `.txt`;
// - los ficheros de los mensajes, en «Original» u «Originales», y el
//   proyecto del chat entero como `.pixpin`.
//
// Traido del Mac. Lo del iPad: una **foto anotada** (lo dibujado encima en
// el movil, en Windows o aqui, `Nucleo.fotoAnotada`) va fusionada, la foto
// con su tinta, como `anotado_del_adjunto.rs` de Windows; y entonces lo
// primero que se ofrece es el PNG, no el original sin lo dibujado.

import ImageIO
import PDFKit
import PixPinCore
import UIKit

@MainActor
enum PrepararCompartir {
    /// Lo que se piden al nucleo de una vez como mucho.
    private static let tope: UInt32 = 200

    static func preparar(_ cosa: CosaCompartida, almacen: Almacen?) async -> PreparadoCompartir {
        switch cosa {
        case let .mensajes(chat, titulo, mensajes):
            return await deMensajes(chat: chat, titulo: titulo, mensajes, almacen: almacen)
        case let .proyectos(fichas):
            return await deProyectos(fichas, almacen: almacen)
        case let .documento(url):
            return deFicheros(titulo: TextosCompartir.sinExtension(url.lastPathComponent), [url])
        case let .ficheros(titulo, urls):
            return deFicheros(titulo: titulo, urls)
        case let .imagen(titulo, img):
            let p = PreparadoCompartir(titulo: titulo)
            p.anadir(PiezasCompartir.foto(img, clave: "imagen", nombre: titulo))
            return p
        case let .preparado(p):
            return p
        }
    }

    // MARK: Ficheros sueltos

    /// Ficheros sueltos: en paginas lo que se pueda (fotos; un PDF solo) y
    /// todos tal cual.
    static func deFicheros(titulo: String, _ urls: [URL]) -> PreparadoCompartir {
        let p = PreparadoCompartir(titulo: titulo)
        let solo = urls.count == 1
        for (i, url) in urls.enumerated() {
            let nombre = TextosCompartir.sinExtension(url.lastPathComponent)
            if PiezasCompartir.esImagen(url),
               let f = PiezasCompartir.foto(url, clave: "f\(i)", nombre: nombre, detalle: "Foto") {
                p.anadir(f)
            } else if solo, PiezasCompartir.esPdf(url) {
                anadirPdf(p, url, clave: "f\(i)")
            }
        }
        anadirOriginales(p, urls)
        p.originalPrimero = true
        return p
    }

    /// **Lo que se ve en un visor** (Word, web, notas…), como `de_visor` de
    /// Windows: sus paginas (en `pdf`, ya paginado) para HTML, PDF, PNG, JPG
    /// y SVG, y el original entero.
    static func deVisor(titulo: String, original: URL, pdf: Data?) -> PreparadoCompartir {
        let p = PreparadoCompartir(titulo: titulo)
        if let pdf {
            let tmp = FileManager.default.temporaryDirectory
                .appendingPathComponent("pixpin-visor-\(UUID().uuidString).pdf")
            if (try? pdf.write(to: tmp)) != nil { anadirPdf(p, tmp, clave: "v") }
        }
        anadirOriginales(p, [original])
        p.originalPrimero = p.piezas.isEmpty
        return p
    }

    /// Las paginas de un PDF, con «Página n».
    static func anadirPdf(_ p: PreparadoCompartir, _ url: URL, clave: String) {
        guard let doc = PDFDocument(url: url), !doc.isLocked else { return }
        for i in 0 ..< doc.pageCount {
            guard let pagina = doc.page(at: i) else { continue }
            p.anadir(PiezasCompartir.paginaPdf(pagina, clave: "\(clave)|p\(i + 1)", nombre: "Página \(i + 1)",
                                               detalle: i == 0 ? "PDF" : "", nivel: 0))
        }
    }

    static func anadirOriginales(_ p: PreparadoCompartir, _ urls: [URL]) {
        let hay = urls.filter { FileManager.default.fileExists(atPath: $0.path) }
        guard !hay.isEmpty else { return }
        let uno = hay.count == 1
        p.extras.append(ExtraCompartir(
            formato: FormatoCompartir(id: uno ? IdFormato.original : IdFormato.originales,
                                      nombre: uno ? "Original" : "Originales", cuantas: .ninguna,
                                      simbolo: "doc.on.doc"),
            que: .ficheros(hay)))
    }

    // MARK: Mensajes

    static func deMensajes(chat: String, titulo: String, _ mensajes: [MensajeChat],
                           almacen: Almacen?) async -> PreparadoCompartir {
        let solo = mensajes.count == 1
        let clasesDeFichero: Set<String> = ["IMAGEN", "ARCHIVO", "VOZ"]
        let nombre: String
        if let m = mensajes.first, solo {
            let n = TextosCompartir.nombreDeFila(nombre: m.nombre, resumen: m.resumen)
            nombre = clasesDeFichero.contains(m.clase) ? TextosCompartir.sinExtension(n) : n
        } else {
            nombre = titulo
        }
        let p = PreparadoCompartir(titulo: nombre)
        var originales: [URL] = []
        for m in mensajes {
            let url = await fichero(m, chat: chat, almacen: almacen)
            if let url { originales.append(url) }
            await anadirMensaje(p, m, chat: chat, fichero: url, solo: solo, almacen: almacen)
        }
        anadirOriginales(p, originales)
        // El proyecto entero del chat, para abrirlo en otro PixPin o el movil.
        if let almacen, let ficha = almacen.chats.first(where: { $0.id == chat }) {
            p.extras.append(paquete(almacen, [ficha], id: IdFormato.pixpin, nombre: ".pixpin"))
        }
        // Una foto o un fichero solos: lo que se suele querer es mandarlos
        // tal cual.
        p.originalPrimero = solo && mensajes.contains { clasesDeFichero.contains($0.clase) } && !originales.isEmpty
        // Una foto con lo dibujado encima: lo natural es mandarla con ello.
        if p.conAnotadas {
            p.originalPrimero = false
            p.formatoInicial = solo ? IdFormato.png : IdFormato.pdf
        }
        return p
    }

    /// **Una foto anotada como pagina**: el lienzo propio de la foto pintado
    /// y recortado a la foto (`soloLaFoto`), con su tinta. `nil` si la foto
    /// no tiene nada dibujado encima (o su lienzo aun no ha llegado).
    static func fotoAnotada(_ m: MensajeChat, chat: String, clave: String, nombre: String, fichero: URL?,
                            almacen: Almacen) async -> PiezaCompartir? {
        let nucleo = almacen.nucleo
        let id = lienzoDeLaFoto(m)
        let leido = await Task.detached(priority: .userInitiated) { () -> (HojaDeDibujo, CajaLienzo?, [UInt64: URL])? in
            guard let anotada = try? nucleo.fotoAnotada(chat: chat, lienzo: id),
                  let hoja = (try? nucleo.hojasDeDibujo(chat: chat, dibujo: anotada.lienzo))?.first else { return nil }
            var fotos: [UInt64: URL] = [:]
            if let lienzo = try? nucleo.abrirLienzo(chat: chat, dibujo: anotada.lienzo) {
                for f in (try? nucleo.imagenesDelLienzo(chat: chat, lienzo: lienzo)) ?? [] {
                    fotos[f.idObjeto] = URL(fileURLWithPath: f.ruta)
                }
            }
            return (hoja, anotada.foto, fotos)
        }.value
        guard let (hoja, foto, rutas) = leido else { return nil }
        let todo = CGRect(x: CGFloat(hoja.x0), y: CGFloat(hoja.y0), width: CGFloat(hoja.x1 - hoja.x0),
                          height: CGFloat(hoja.y1 - hoja.y0))
        let soloFoto = foto.map {
            CGRect(x: CGFloat($0.x0), y: CGFloat($0.y0), width: CGFloat($0.x1 - $0.x0), height: CGFloat($0.y1 - $0.y0))
        }
        // La foto sola o con lo dibujado alrededor, como se ve en la burbuja.
        guard let caja = cajaDeFotoAnotada(foto: soloFoto, todo: todo, soloLaFoto: m.soloLaFoto),
              caja.width > 0, caja.height > 0 else { return nil }
        // A los pixeles de la foto de verdad (en el lienzo suele ir reducida),
        // sin pasar de 4 por unidad.
        let pixeles = fichero.flatMap(PiezasCompartir.anchoEnPixeles) ?? caja.width * 2
        let escala = min(max(pixeles / caja.width, 1), 4)
        let fotos = CacheDeFotos(rutas)
        let plano = hoja.plano
        var pieza = PiezaCompartir(
            pagina: PaginaCompartir(clave: clave, nombre: nombre, detalle: "Foto · Con lo anotado"),
            tamano: caja.size, fondo: hoja.fondo, escalaImagen: escala,
            pintar: { ctx, destino in
                PintorDeHojas.pintar(plano, caja: caja, en: ctx, destino: destino, transparente: false,
                                     tapados: nil, version: 0, fotos: { fotos.imagen($0) })
            },
            svg: { nil })
        // El SVG del motor es el lienzo entero: aqui va la foto ya fusionada.
        let copia = pieza
        pieza.svg = {
            guard let img = PreparadoCompartir.imagen(copia, opaca: true),
                  let png = PreparadoCompartir.codificar(img, .png) else { return nil }
            return PiezasCompartir.svgDeImagen(png, mime: "image/png", tamano: caja.size)
        }
        return pieza
    }

    /// El fichero del mensaje en este Mac, si esta.
    static func fichero(_ m: MensajeChat, chat: String, almacen: Almacen?) async -> URL? {
        guard let almacen, let ruta = m.ruta, !ruta.isEmpty,
              let url = await almacen.url(deRuta: ruta, en: chat),
              FileManager.default.fileExists(atPath: url.path) else { return nil }
        return url
    }

    /// **Las paginas de un mensaje**, segun lo que sea.
    static func anadirMensaje(_ p: PreparadoCompartir, _ m: MensajeChat, chat: String, fichero: URL?,
                              solo: Bool, almacen: Almacen?) async {
        let clave = m.id
        let nombre = TextosCompartir.nombreDeFila(nombre: m.nombre, resumen: m.resumen)
        switch m.clase {
        case "DIBUJO", "PAGINA":
            guard let almacen, let referencia = m.referencia, !referencia.isEmpty else { return }
            let detalle = m.clase == "PAGINA" ? "Página del PDF" : "Lienzo"
            await anadirDibujo(p, chat: chat, referencia: referencia, clave: clave,
                               nombre: m.nombre.isEmpty ? detalle : nombre, detalle: detalle,
                               solo: solo, almacen: almacen)
        case "IMAGEN":
            let nombreFoto = TextosCompartir.sinExtension(nombre)
            if let almacen, let anotada = await fotoAnotada(m, chat: chat, clave: clave, nombre: nombreFoto,
                                                            fichero: fichero, almacen: almacen) {
                p.anadir(anotada)
                p.conAnotadas = true
                return
            }
            guard let fichero, let f = PiezasCompartir.foto(
                fichero, clave: clave, nombre: nombreFoto, detalle: "Foto") else { return }
            p.anadir(f)
        case "ARCHIVO":
            guard solo, let fichero else { return }
            if PiezasCompartir.esPdf(fichero) {
                anadirPdf(p, fichero, clave: clave)
            } else if PiezasCompartir.esImagen(fichero),
                      let f = PiezasCompartir.foto(fichero, clave: clave, nombre: nombre, detalle: "Foto") {
                p.anadir(f)
            }
        case "VOZ":
            let texto = (m.transcripcion ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            guard !texto.isEmpty else { return }
            anadirTexto(p, clave: clave, nombre: nombre, detalle: "Nota de voz", titulo: nil, texto: texto, solo: solo)
        case "PROYECTO":
            return
        case "MINIAPP" where CarpetasDelChat.esCarpeta(m):
            // Una carpeta de paginas: sus paginas, en orden, con la regla
            // del cuaderno (`Compartir/PdfDeCarpeta.swift`).
            guard let almacen, let carpeta = await CarpetasDelChat.leer(m.id, chat: chat, almacen: almacen) else { return }
            for pagina in carpeta.paginas {
                await PdfDeCarpeta.anadirPagina(p, pagina, chat: chat, almacen: almacen)
            }
        default:
            // Una mini-app guarda su documento en el texto: se lee su resumen.
            let texto = m.clase == "MINIAPP" ? m.resumen : (m.texto.isEmpty ? m.resumen : m.texto)
            guard !texto.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
            anadirTexto(p, clave: clave, nombre: nombre, detalle: m.clase == "MINIAPP" ? "Mini-app" : "Nota",
                        titulo: nil, texto: texto, solo: solo)
        }
    }

    /// Un texto como paginas. Solo, va tambien entero como `.txt`.
    static func anadirTexto(_ p: PreparadoCompartir, clave: String, nombre: String, detalle: String,
                            titulo: String?, texto: String, solo: Bool) {
        for pieza in PiezasCompartir.texto(clave: clave, nombre: nombre, detalle: detalle, titulo: titulo, texto: texto) {
            p.anadir(pieza)
        }
        if solo {
            let entero = titulo.map { "\($0)\n\n\(texto)" } ?? texto
            let fichero = "\(p.base).txt"
            p.extras.append(ExtraCompartir(
                formato: FormatoCompartir(id: IdFormato.texto, nombre: "Texto", cuantas: .ninguna, simbolo: "text.alignleft"),
                que: .escrito(nombre: fichero) { Data(entero.utf8) }))
        }
    }

    /// **Un lienzo como paginas**: el entero y, si tiene marcos, cada marco
    /// debajo, sangrado (`anadir_lienzo`).
    static func anadirDibujo(_ p: PreparadoCompartir, chat: String, referencia: String, clave: String,
                             nombre: String, detalle: String, solo: Bool, almacen: Almacen) async {
        let nucleo = almacen.nucleo
        let leido = await Task.detached(priority: .userInitiated) { () -> ([HojaDeDibujo], [UInt64: URL], String?)? in
            guard let hojas = try? nucleo.hojasDeDibujo(chat: chat, dibujo: referencia), !hojas.isEmpty else { return nil }
            var fotos: [UInt64: URL] = [:]
            if let lienzo = try? nucleo.abrirLienzo(chat: chat, dibujo: referencia) {
                for f in (try? nucleo.imagenesDelLienzo(chat: chat, lienzo: lienzo)) ?? [] {
                    fotos[f.idObjeto] = URL(fileURLWithPath: f.ruta)
                }
            }
            let json = solo ? try? nucleo.excalidrawDeDibujo(chat: chat, dibujo: referencia) : nil
            return (hojas, fotos, json)
        }.value
        guard let (hojas, rutas, json) = leido else { return }
        let fotos = CacheDeFotos(rutas)
        let conMarcos = hojas.count > 1
        for (i, h) in hojas.enumerated() {
            let pagina = i == 0
                ? PaginaCompartir(clave: clave, nombre: conMarcos ? "Lienzo completo" : nombre, detalle: detalle)
                : PaginaCompartir(clave: "\(clave)|marco-\(i)",
                                  nombre: h.nombre.trimmingCharacters(in: .whitespaces).isEmpty ? "Marco \(i)" : h.nombre,
                                  detalle: "Marco", nivel: 1)
            // Con marcos, van marcados los marcos y no el entero.
            p.anadir(PiezasCompartir.dibujo(h, pagina: pagina, fotos: fotos), marcada: i > 0 || !conMarcos)
        }
        // El lienzo va dentro de la pagina web; fuera, lo editable es el
        // `.pixpin` del chat (el `.excalidraw` suelto ya no se ofrece,
        // como en Windows desde el 20-oct-2026).
        if let json { p.excalidraw = json }
    }

    // MARK: Proyectos

    /// **Unos proyectos de la lista**: todas sus hojas, en el orden del
    /// chat, con el nombre del proyecto al lado si son varios, y cada uno
    /// entero como `.pixpin` para seguir editandolo en otro PixPin.
    static func deProyectos(_ fichas: [FichaChat], almacen: Almacen?) async -> PreparadoCompartir {
        let p = PreparadoCompartir(titulo: fichas.map(\.nombre).joined(separator: " + "))
        guard let almacen else { return p }
        let varios = fichas.count > 1
        var originales: [URL] = []
        for ficha in fichas {
            let desde = p.piezas.count
            for m in await todos(ficha.id, almacen: almacen) {
                let url = await fichero(m, chat: ficha.id, almacen: almacen)
                if let url { originales.append(url) }
                await anadirMensaje(p, m, chat: ficha.id, fichero: url, solo: false, almacen: almacen)
            }
            if varios {
                p.renombrarDetalles(desde: desde) { $0.isEmpty ? ficha.nombre : "\(ficha.nombre) · \($0)" }
            }
        }
        anadirOriginales(p, originales)
        p.extras.append(paquete(almacen, fichas, id: IdFormato.editable, nombre: "Editable"))
        return p
    }

    /// Todos los mensajes de un chat, del mas viejo al mas nuevo.
    static func todos(_ chat: String, almacen: Almacen) async -> [MensajeChat] {
        let nucleo = almacen.nucleo
        let tope = self.tope
        return await Task.detached(priority: .userInitiated) {
            var v: [MensajeChat] = []
            var desde: UInt64 = 0
            while let pagina = try? nucleo.mensajes(chat: chat, desde: desde, cuantos: tope), !pagina.mensajes.isEmpty {
                v += pagina.mensajes
                desde = pagina.desde + UInt64(pagina.mensajes.count)
                if desde >= pagina.total { break }
            }
            return v
        }.value
    }

    /// El `.pixpin` de unos proyectos: uno, el fichero; varios, uno por
    /// proyecto (se escriben al pedirlos: pueden pesar mucho).
    static func paquete(_ almacen: Almacen, _ fichas: [FichaChat], id: String, nombre: String) -> ExtraCompartir {
        let nucleo = almacen.nucleo
        let formato = FormatoCompartir(id: id, nombre: nombre, cuantas: .ninguna, simbolo: "shippingbox")
        if fichas.count == 1, let f = fichas.first {
            return ExtraCompartir(formato: formato, que: .escrito(nombre: "\(nombreDeFicheroCompartido(titulo: f.nombre)).pixpin") {
                try await Task.detached { try nucleo.paqueteDeProyecto(chat: f.id) }.value
            })
        }
        // Varios: cada uno en su fichero, escritos al pedirlos.
        return ExtraCompartir(formato: formato, que: .lote { carpeta in
            var urls: [URL] = []
            for f in fichas {
                let datos = try await Task.detached { try nucleo.paqueteDeProyecto(chat: f.id) }.value
                let url = carpeta.appendingPathComponent(nombreDeFicheroCompartido(titulo: f.nombre))
                    .appendingPathExtension("pixpin")
                try datos.write(to: url, options: .atomic)
                urls.append(url)
            }
            return urls
        })
    }
}

extension PreparadoCompartir {
    /// Cambia el detalle de las piezas desde `desde` (el nombre del
    /// proyecto delante, si son varios).
    func renombrarDetalles(desde: Int, _ cambio: (String) -> String) {
        for i in desde ..< piezas.count { cambiarDetalle(i, cambio(piezas[i].pagina.detalle)) }
    }
}
