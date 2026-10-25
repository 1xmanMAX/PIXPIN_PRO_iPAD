// **La burbuja de una nota rica** (Windows 974d5ba, `ventana_chat/nota_rica.rs`;
// del Mac, `Chat/NotaRica.swift`): el Markdown de la nota ensenado entero
// dentro de su burbuja, con su texto con formato y sus fotos en su sitio;
// las seguidas, en album (filas de dos o tres), como un mensaje de Telegram
// con varias fotos.
//
// Los bloques y el reparto del album salen del nucleo (`pixpin_ui::nota_rica`,
// el mismo crate que Windows): asi el iPad ensena exactamente lo mismo. Una
// nota llana (`notaRicaSinFotos` da `nil`) sigue con su `Text` de siempre.
//
// Los bloques se piden una vez por texto (`NotasRicas`, una `NSCache`); las
// fotos se buscan fuera del hilo principal en una llamada por nota
// (`Nucleo.notaRica`) y se pintan con `Miniaturas.compartidas`. Mientras
// llega, o si su fichero no esta en este iPad, un recuadro con el icono de
// imagen.
//
// Y **con texto escrito, una foto pegada va DENTRO de lo escrito**
// (`fotos_al_borrador` de Windows): `EntradaDeFicheros.pegarEnLaNota`.

import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Las notas ricas ya entendidas: la de cada texto (sin mirar el disco) y,
/// cuando ya se buscaron, con sus fotos.
final class NotasRicas: @unchecked Sendable {
    static let compartidas = NotasRicas()

    private final class Caja {
        let nota: NotaRica?
        init(_ nota: NotaRica?) { self.nota = nota }
    }

    private let puras = NSCache<NSString, Caja>()
    private let resueltas = NSCache<NSString, Caja>()

    init() {
        puras.countLimit = 2000
        resueltas.countLimit = 500
    }

    /// La nota rica de `texto` sin sus ficheros, o `nil` si es una nota
    /// llana. Una llamada al nucleo por texto distinto.
    func pura(_ texto: String) -> NotaRica? {
        let clave = texto as NSString
        if let hecha = puras.object(forKey: clave) { return hecha.nota }
        let nota = notaRicaSinFotos(texto: texto)
        puras.setObject(Caja(nota), forKey: clave)
        return nota
    }

    private static func clave(_ chat: String, _ texto: String) -> NSString {
        (chat + "\u{0}" + texto) as NSString
    }

    /// La nota con sus fotos si ya se busco. No cruza el puente.
    func yaResuelta(chat: String, texto: String) -> NotaRica? {
        resueltas.object(forKey: NotasRicas.clave(chat, texto))?.nota
    }

    /// Busca las fotos de la nota fuera del hilo principal. Se guarda solo
    /// si estan todas: una que aun no ha llegado se vuelve a buscar la
    /// proxima vez que se ve la burbuja.
    func resolver(chat: String, texto: String, nucleo: Nucleo) async -> NotaRica? {
        if let hecha = yaResuelta(chat: chat, texto: texto) { return hecha }
        let nota = await Task.detached(priority: .userInitiated) {
            (try? nucleo.notaRica(chat: chat, texto: texto)) ?? nil
        }.value
        guard let nota else { return nil }
        if NotasRicas.fotos(de: nota).allSatisfy({ $0.ruta != nil }) {
            resueltas.setObject(Caja(nota), forKey: NotasRicas.clave(chat, texto))
        }
        return nota
    }

    /// Las fotos de la nota, en orden (las de sus albumes).
    static func fotos(de nota: NotaRica) -> [FotoDeNota] {
        nota.bloques.flatMap { b -> [FotoDeNota] in
            if case let .album(fotos) = b { return fotos }
            return []
        }
    }
}

/// Las medidas de la nota (las de Windows, en puntos a escala 1).
private enum MedidasNota {
    static let entreBloques: CGFloat = 6
    static let nivel: CGFloat = 14
    static let vineta: CGFloat = 16
    static let casilla: CGFloat = 15
    static let citaBarra: CGFloat = 3
    static let citaHueco: CGFloat = 8
    static let codigoTam: CGFloat = 13
    static let codigoRelleno: CGFloat = 6
    static let huecoAlbum: CGFloat = 2
    static let radioFoto: CGFloat = 4
    /// La letra de las notas en el iPad (la del `Text` de una nota llana).
    static let texto: CGFloat = 16
    /// El alto de una fila del album y el tope de una foto sola (vez y
    /// media), los de las fotos del chat de Windows (`VISTA_ALTO`).
    static let altoFila: CGFloat = 180
}

/// La burbuja de una `NOTA` con formato o fotos.
struct BurbujaNotaRica: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    /// Sus bloques sin mirar el disco: con ellos se pinta ya.
    let pura: NotaRica
    @State private var resuelta: NotaRica?
    @Environment(\.agujaDelChat) private var aguja
    @Environment(\.escalaChat) private var escala

    var body: some View {
        let nota = resuelta ?? NotasRicas.compartidas.yaResuelta(chat: chat, texto: mensaje.texto) ?? pura
        VStack(alignment: .leading, spacing: MedidasNota.entreBloques * escala) {
            ForEach(Array(nota.bloques.enumerated()), id: \.offset) { _, b in
                bloque(b)
            }
        }
        .tint(Paleta.chat.enviar)
        .fixedSize(horizontal: false, vertical: true)
        .task(id: mensaje.texto) {
            resuelta = await NotasRicas.compartidas.resolver(chat: chat, texto: mensaje.texto,
                                                            nucleo: almacen.nucleo)
        }
    }

    @ViewBuilder
    private func bloque(_ b: BloqueDeNota) -> some View {
        switch b {
        case let .texto(clase, texto, marcas, enlaces):
            renglon(clase, texto: texto, marcas: marcas, enlaces: enlaces)
        case let .codigo(texto):
            // Una tabla tambien llega asi: en monoespaciada se lee.
            Text(texto)
                .font(.system(size: MedidasNota.codigoTam * escala, design: .monospaced))
                .padding(MedidasNota.codigoRelleno * escala)
                .background(Color.primary.opacity(0.08),
                            in: RoundedRectangle(cornerRadius: 4 * escala))
        case .regla:
            Rectangle()
                .fill(Color.secondary.opacity(0.5))
                .frame(width: ladoDeBurbuja * escala, height: 1)
                .padding(.vertical, 4 * escala)
        case let .album(fotos):
            AlbumDeNotaVista(fotos: fotos, ancho: ladoDeBurbuja * escala, escala: escala,
                             abrir: abrir)
        case let .adjunto(nombre):
            // Un fichero que no es una foto: su nombre con el clip.
            Text("📎 \(nombre)")
        }
    }

    /// Un parrafo, titulo, renglon de lista, casilla o cita.
    @ViewBuilder
    private func renglon(_ clase: ClaseDeBloque, texto: String, marcas: [MarcaDeNota],
                         enlaces: [EnlaceDeNota]) -> some View {
        let base = MedidasNota.texto * escala
        switch clase {
        case let .titulo(nivel):
            let tam: CGFloat = nivel == 1 ? 21 : (nivel == 2 ? 19 : 17.5)
            Text(atribuido(texto, marcas: marcas, enlaces: enlaces, tam: tam * escala, negrita: true))
        case let .vineta(nivel):
            HStack(alignment: .firstTextBaseline, spacing: 0) {
                Text("•").font(.system(size: base))
                    .frame(width: MedidasNota.vineta * escala, alignment: .center)
                Text(atribuido(texto, marcas: marcas, enlaces: enlaces, tam: base))
            }
            .padding(.leading, CGFloat(nivel) * MedidasNota.nivel * escala)
        case let .numero(nivel, numero):
            HStack(alignment: .firstTextBaseline, spacing: 5 * escala) {
                Text(numero).font(.system(size: base))
                    .frame(minWidth: (MedidasNota.vineta - 5) * escala, alignment: .leading)
                Text(atribuido(texto, marcas: marcas, enlaces: enlaces, tam: base))
            }
            .padding(.leading, CGFloat(nivel) * MedidasNota.nivel * escala)
        case let .casilla(nivel, hecha):
            HStack(alignment: .firstTextBaseline, spacing: 5 * escala) {
                Image(systemName: hecha ? "checkmark.square.fill" : "square")
                    .font(.system(size: MedidasNota.casilla * escala * 0.85))
                    .foregroundStyle(Paleta.chat.enviar)
                Text(atribuido(texto, marcas: marcas, enlaces: enlaces, tam: base))
                    .foregroundStyle(hecha ? Color.secondary : Color.primary)
            }
            .padding(.leading, CGFloat(nivel) * MedidasNota.nivel * escala)
        case .cita:
            HStack(alignment: .top, spacing: MedidasNota.citaHueco * escala) {
                Rectangle().fill(Paleta.chat.enviar)
                    .frame(width: MedidasNota.citaBarra * escala)
                Text(atribuido(texto, marcas: marcas, enlaces: enlaces, tam: base))
            }
            .fixedSize(horizontal: false, vertical: true)
        case .parrafo:
            Text(atribuido(texto, marcas: marcas, enlaces: enlaces, tam: base))
        }
    }

    /// El texto de un bloque con sus marcas (en UTF-16, las de `NSString`),
    /// sus enlaces, las direcciones sueltas y lo buscado resaltado.
    private func atribuido(_ texto: String, marcas: [MarcaDeNota], enlaces: [EnlaceDeNota],
                           tam: CGFloat, negrita: Bool = false) -> AttributedString {
        var a = AttributedString(texto)
        a.font = Font.system(size: tam, weight: negrita ? .bold : .regular)
        func rango(_ inicio: UInt32, _ largo: UInt32) -> Range<AttributedString.Index>? {
            guard let r = Range(NSRange(location: Int(inicio), length: Int(largo)), in: texto),
                  let desde = AttributedString.Index(r.lowerBound, within: a),
                  let hasta = AttributedString.Index(r.upperBound, within: a) else { return nil }
            return desde ..< hasta
        }
        for m in marcas {
            guard let r = rango(m.inicio, m.largo) else { continue }
            var letra = Font.system(size: tam, weight: (m.negrita || negrita) ? .bold : .regular,
                                    design: m.codigo ? .monospaced : .default)
            if m.cursiva { letra = letra.italic() }
            a[r].font = letra
            if m.tachado { a[r].strikethroughStyle = .single }
            if m.codigo { a[r].backgroundColor = Color.primary.opacity(0.08) }
        }
        // Las direcciones sueltas, como en cualquier nota; las del Markdown
        // despues, que mandan.
        conEnlaces(&a)
        for e in enlaces {
            guard let r = rango(e.inicio, e.largo), let url = URL(string: e.url) else { continue }
            a[r].link = url
            a[r].underlineStyle = .single
        }
        let q = aguja.trimmingCharacters(in: .whitespacesAndNewlines)
        if !q.isEmpty {
            var desde = texto.startIndex
            while desde < texto.endIndex,
                  let r = texto.range(of: q, options: [.caseInsensitive, .diacriticInsensitive],
                                      range: desde ..< texto.endIndex) {
                if let ar = Range(r, in: a) {
                    a[ar].backgroundColor = Color.yellow.opacity(0.55)
                    a[ar].foregroundColor = Color.black
                }
                desde = r.upperBound
            }
        }
        return a
    }

    /// Tocar una foto la saca en un pin, como en Windows (donde se ve
    /// grande, se amplia y se copia).
    private func abrir(_ foto: FotoDeNota) {
        guard let ruta = foto.ruta, FileManager.default.fileExists(atPath: ruta) else { return }
        _ = GestorPinesIPad.compartido.pinear(url: URL(fileURLWithPath: ruta))
    }
}

/// Un album de la nota: sus fotos donde las pone `albumDeNota` (el reparto
/// de Windows), una sola con su proporcion y las de una fila recortadas
/// por el centro.
struct AlbumDeNotaVista: View {
    let fotos: [FotoDeNota]
    let ancho: CGFloat
    let escala: CGFloat
    let abrir: (FotoDeNota) -> Void

    var body: some View {
        let reparto = albumDeNota(proporciones: fotos.map(\.proporcion),
                                  ancho: Float(ancho),
                                  altoSolo: Float(MedidasNota.altoFila * escala * 1.5),
                                  altoFila: Float(MedidasNota.altoFila * escala),
                                  hueco: Float(MedidasNota.huecoAlbum * escala))
        let sola = fotos.count == 1
        let anchoUsado = reparto.cajas.map { CGFloat($0.x + $0.ancho) }.max() ?? ancho
        ZStack(alignment: .topLeading) {
            ForEach(0 ..< min(fotos.count, reparto.cajas.count), id: \.self) { k in
                let caja = reparto.cajas[k]
                FotoDeNotaVista(foto: fotos[k], ancho: CGFloat(caja.ancho), alto: CGFloat(caja.alto),
                                sola: sola, radio: MedidasNota.radioFoto * escala)
                    .onTapGesture { abrir(fotos[k]) }
                    .offset(x: CGFloat(caja.x), y: CGFloat(caja.y))
            }
        }
        .frame(width: anchoUsado, height: CGFloat(reparto.alto), alignment: .topLeading)
    }
}

/// Una foto de la nota por su miniatura, o su recuadro con el icono de
/// imagen mientras llega o si su fichero no esta aqui.
struct FotoDeNotaVista: View {
    let foto: FotoDeNota
    let ancho: CGFloat
    let alto: CGFloat
    let sola: Bool
    let radio: CGFloat
    @State private var imagen: CGImage?

    /// A 2x (Retina) y en escalones de 128 px: dos fotos de tamano parecido
    /// comparten miniatura en la cache.
    private var pixeles: Int {
        let lado = Int(max(ancho, alto) * 2)
        return max(128, (lado + 127) / 128 * 128)
    }

    private var url: URL? { foto.ruta.map { URL(fileURLWithPath: $0) } }

    var body: some View {
        let hecha = imagen ?? url.flatMap { Miniaturas.compartidas.enCache($0, lado: pixeles) }
        ZStack {
            if let hecha {
                Image(decorative: hecha, scale: 2)
                    .resizable()
                    .interpolation(.high)
                    .aspectRatio(contentMode: sola ? .fit : .fill)
                    .frame(width: ancho, height: alto)
            } else {
                Rectangle().fill(Color.secondary.opacity(0.18))
                Image(systemName: "photo")
                    .font(.system(size: min(24, ancho, alto) * 0.8))
                    .foregroundStyle(.secondary)
            }
        }
        .frame(width: ancho, height: alto)
        .clipShape(RoundedRectangle(cornerRadius: radio))
        .contentShape(Rectangle())
        .accessibilityLabel(foto.alt)
        .task(id: foto.ruta) {
            guard let url, imagen == nil else { return }
            imagen = await Miniaturas.compartidas.cargar(url, lado: pixeles)
        }
    }
}

// MARK: - Pegar una foto con texto escrito

/// De donde sale una foto pegada.
private enum FuenteDeFoto: Sendable {
    case fichero(URL)
    case datos(Data)
}

extension EntradaDeFicheros {
    /// **Con texto ya escrito, una foto pegada va DENTRO de lo escrito**
    /// (`fotos_al_borrador` de Windows, como el Mac): se guarda en `notas/`
    /// del proyecto y su renglon `![](notas/…)` queda donde esta la rayita;
    /// al enviar sale UNA nota con el texto y sus fotos. Solo fotos: un PDF
    /// o un audio siguen siendo su mensaje, y con la caja vacia la foto
    /// tambien. `false` si no es lo suyo y el ⌘V sigue su camino.
    @MainActor
    static func pegarEnLaNota(en chat: String, almacen: Almacen, campo: UITextView,
                              avisar: @escaping (String) -> Void) -> Bool {
        guard !(campo.text ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return false }
        let pb = UIPasteboard.general
        var fuentes: [(String, FuenteDeFoto)] = []
        let urls = (pb.urls ?? []).filter(\.isFileURL)
        if !urls.isEmpty {
            guard urls.allSatisfy({ esFotoDeNota(nombre: $0.lastPathComponent) }) else { return false }
            fuentes = urls.map { ($0.lastPathComponent, FuenteDeFoto.fichero($0)) }
        } else if !pb.hasStrings {
            // Una imagen suelta (una captura copiada): en PNG, como Windows.
            if let png = pb.data(forPasteboardType: UTType.png.identifier) {
                fuentes = [("pegado.png", FuenteDeFoto.datos(png))]
            } else if let png = pb.image?.pngData() {
                fuentes = [("pegado.png", FuenteDeFoto.datos(png))]
            }
        }
        guard !fuentes.isEmpty else { return false }
        let nucleo = almacen.nucleo
        let pedidas = fuentes
        _ = Task<Void, Never> { @MainActor in
            let renglones = await Task.detached(priority: .userInitiated) { () -> [String] in
                pedidas.compactMap { par -> String? in
                    let datos: Data?
                    switch par.1 {
                    case let .fichero(url): datos = try? Data(contentsOf: url)
                    case let .datos(d): datos = d
                    }
                    guard let datos else { return nil }
                    return try? nucleo.fotoALaNota(chat: chat, nombre: par.0, datos: datos)
                }
            }.value
            guard !renglones.isEmpty else {
                avisar("No se pudo hacer")
                return
            }
            // Donde este la rayita ahora, cada foto en su renglon.
            let texto = (campo.text ?? "") as NSString
            let r = campo.selectedRange
            let antes = texto.substring(to: min(texto.length, r.location))
            let despues = texto.substring(from: min(texto.length, r.location + r.length))
            let s = renglonesEnSuSitio(antes: antes, despues: despues, renglones: renglones)
            if let rango = campo.selectedTextRange {
                campo.replace(rango, withText: s)
            } else {
                campo.insertText(s)
            }
            avisar("Foto añadida a lo escrito: se envía todo en una sola nota")
        }
        return true
    }
}
