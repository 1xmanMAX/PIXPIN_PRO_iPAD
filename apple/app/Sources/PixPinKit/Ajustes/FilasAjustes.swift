// **Las filas de Ajustes del iPad**, las de `ventana_ajustes.rs` de Windows
// (`filas_de_seccion`) y `FilasAjustes.swift` del Mac: cada seccion con sus
// grupos y cada opcion con su explicacion debajo y su control, con los
// mismos textos y rangos, para que los tres digan lo mismo del mismo
// `pixpinmax.toml`.
//
// Son datos y no vistas: asi el buscador busca en todas, «Restablecer
// <seccion>» recorre las suyas y las pruebas las comprueban sin pintar nada.
// Cada fila sabe si esta como de fabrica (`cambiada`, el punto azul) y
// volver a ella (`restablecer`).
//
// Lo que Windows tiene y aqui no sale:
// - Captura, Atajos y Apps predeterminadas: iOS no deja capturar otras apps,
//   ni atajos globales, ni ser la app de las fotos (⛔). Los dias que
//   aguanta una captura si (la galeria), en General.
//
// Tareas (palabras de urgencia), Pinear (palabras magicas) y Lienzo (iman,
// suavizado y que herramientas salen y donde) van en `FilasOrganizar.swift`,
// con su registro aparte del nucleo (`AjustesOrganizar`).

import AVFoundation
import PixPinCore
import SwiftUI

/// Las secciones, en el orden de Windows (las que el iPad tiene).
public enum SeccionAjustes: String, CaseIterable, Identifiable, Sendable {
    case general, pinear, tareas, lienzo, voz, asistente, sincro, apariencia, avanzado

    public var id: String { rawValue }

    public var titulo: String {
        switch self {
        case .general: "General"
        case .pinear: "Pinear"
        case .tareas: "Tareas"
        case .lienzo: "Lienzo y herramientas"
        case .voz: "Voz y transcripción"
        case .asistente: "Asistente"
        case .sincro: "Sincronizar"
        case .apariencia: "Apariencia"
        case .avanzado: "Avanzado"
        }
    }

    public var subtitulo: String {
        switch self {
        case .general: "El idioma, qué hace PixPin con los PDF del chat y cuánto aguantan las capturas."
        case .pinear: "Las palabras que se convierten en herramienta al pinearlas."
        case .tareas: "Lo que se entiende al escribir una tarea."
        case .lienzo: "El imán, el trazo a mano y qué herramientas salen en la barra."
        case .voz: "Con qué voz se leen los documentos en voz alta."
        case .asistente: "Quién contesta en el chat «Asistente» y qué puede hacer."
        case .sincro: "Cómo se encuentra este iPad con el PC, el Mac y el móvil."
        case .apariencia: "El aspecto de PixPin."
        case .avanzado: "El archivo de ajustes y la carpeta de datos."
        }
    }

    public var icono: String {
        switch self {
        case .general: "gearshape"
        case .pinear: "pin"
        case .tareas: "checkmark.square"
        case .lienzo: "scribble.variable"
        case .voz: "mic"
        case .asistente: "sparkles"
        case .sincro: "arrow.triangle.2.circlepath"
        case .apariencia: "paintpalette"
        case .avanzado: "slider.horizontal.3"
        }
    }
}

/// El control de una fila.
public enum ControlAjuste {
    case interruptor(get: @MainActor (ModeloAjustes) -> Bool, set: @MainActor (ModeloAjustes, Bool) -> Void)
    /// Botones de opcion (el segmentado de Windows) o, si son largas, un menu.
    case opcion([String], get: @MainActor (ModeloAjustes) -> Int, set: @MainActor (ModeloAjustes, Int) -> Void)
    /// − valor +, con su texto.
    case numero(ClosedRange<Int>, paso: Int, texto: (Int) -> String,
                get: @MainActor (ModeloAjustes) -> Int, set: @MainActor (ModeloAjustes, Int) -> Void)
    /// Algo propio (las conexiones, la voz de cada idioma, un texto).
    case vista(@MainActor (ModeloAjustes) -> AnyView)
    case nada
}

/// Una fila: titulo, explicacion y control.
public struct FilaAjuste: Identifiable {
    public let id: String
    public let seccion: SeccionAjustes
    public let grupo: String
    public let titulo: String
    public let ayuda: String
    public let control: ControlAjuste
    /// Si no esta como de fabrica (el punto azul).
    public var cambiada: @MainActor (ModeloAjustes) -> Bool = { _ in false }
    /// La vuelve a fabrica dentro de un cambio del TOML (`nil`: no se puede).
    public var restablecer: ((inout AjustesMac, AjustesMac) -> Void)?
    /// Lo mismo para lo que no es del TOML de `valores` (tema, PDF, voz).
    public var restablecerOtro: (@MainActor (ModeloAjustes) -> Void)?
    /// La vuelve a fabrica dentro de un cambio de Tareas, Lienzo o Pinear
    /// (`AjustesOrganizar`).
    public var restablecerOrganizar: ((inout AjustesOrganizar, AjustesOrganizar) -> Void)?

    public var restablecible: Bool { restablecer != nil || restablecerOtro != nil || restablecerOrganizar != nil }
}

@MainActor
public enum FilasAjustes {
    /// Una fila de un valor del TOML: lee, escribe, sabe si cambio y vuelve.
    static func toml<V: Equatable>(_ s: SeccionAjustes, _ grupo: String, _ id: String,
                                   _ titulo: String, _ ayuda: String,
                                   _ kp: WritableKeyPath<AjustesMac, V>,
                                   control: (_ get: @escaping @MainActor (ModeloAjustes) -> V,
                                             _ set: @escaping @MainActor (ModeloAjustes, V) -> Void) -> ControlAjuste)
        -> FilaAjuste {
        let get: @MainActor (ModeloAjustes) -> V = { $0.valores[keyPath: kp] }
        let set: @MainActor (ModeloAjustes, V) -> Void = { m, v in m.cambiar { $0[keyPath: kp] = v } }
        var f = FilaAjuste(id: id, seccion: s, grupo: grupo, titulo: titulo, ayuda: ayuda, control: control(get, set))
        f.cambiada = { $0.valores[keyPath: kp] != $0.fabrica[keyPath: kp] }
        f.restablecer = { a, fabrica in a[keyPath: kp] = fabrica[keyPath: kp] }
        return f
    }

    static func interruptor(_ s: SeccionAjustes, _ grupo: String, _ id: String, _ titulo: String,
                            _ ayuda: String, _ kp: WritableKeyPath<AjustesMac, Bool>) -> FilaAjuste {
        toml(s, grupo, id, titulo, ayuda, kp) { get, set in .interruptor(get: get, set: set) }
    }

    /// Opciones con su valor en el TOML; lo que no este en la lista cae en la
    /// primera (como `unwrap_or(0)` de Windows).
    static func opcion<V: Equatable>(_ s: SeccionAjustes, _ grupo: String, _ id: String, _ titulo: String,
                                     _ ayuda: String, _ kp: WritableKeyPath<AjustesMac, V>,
                                     _ opciones: [(String, V)]) -> FilaAjuste {
        toml(s, grupo, id, titulo, ayuda, kp) { get, set in
            .opcion(opciones.map(\.0),
                    get: { m in opciones.firstIndex { $0.1 == get(m) } ?? 0 },
                    set: { m, i in set(m, opciones[i].1) })
        }
    }

    static func numero(_ s: SeccionAjustes, _ grupo: String, _ id: String, _ titulo: String, _ ayuda: String,
                       _ kp: WritableKeyPath<AjustesMac, UInt32>, _ rango: ClosedRange<Int>, paso: Int,
                       texto: @escaping (Int) -> String) -> FilaAjuste {
        toml(s, grupo, id, titulo, ayuda, kp) { get, set in
            .numero(rango, paso: paso, texto: texto,
                    get: { Int(get($0)) },
                    set: { m, n in set(m, UInt32(min(max(n, rango.lowerBound), rango.upperBound))) })
        }
    }

    nonisolated static func dias(_ n: Int) -> String { n == 0 ? "Nunca" : n == 1 ? "1 día" : "\(n) días" }

    /// Todas las filas de una seccion.
    public static func de(_ s: SeccionAjustes) -> [FilaAjuste] {
        switch s {
        case .general: return general
        case .pinear: return pinear
        case .tareas: return tareas
        case .lienzo: return lienzo
        case .voz: return voz
        case .asistente: return asistente
        case .sincro: return sincro
        case .apariencia: return apariencia
        case .avanzado: return avanzado
        }
    }

    public static var todas: [FilaAjuste] { SeccionAjustes.allCases.flatMap(de) }

    // MARK: General

    static var general: [FilaAjuste] {
        [
            opcion(.general, "Idioma", "idioma", "Idioma", "Vale desde el próximo arranque. Lo comparte con PixPin para Windows.",
                   \.idioma, [("El del iPad", "sistema"), ("Español", "es"), ("English", "en")]),
            filaPdfAligerar(), filaPdfNivel(), filaEscala(),
            numero(.general, "Galería", "caducidad", "Días hasta borrar",
                   "Pasado el plazo, las capturas de la galería se van a la Papelera, salvo las conservadas.",
                   \.diasCaducidad, 0 ... 90, paso: 1, texto: dias),
        ]
    }

    /// **La escala de la interfaz** (Windows c687285: sale del menu ⋮ del
    /// chat a Ajustes → General → Ventanas). La del chat de ⌘+ / ⌘− / ⌘0
    /// (`EscalaChat`): es de esta pantalla, no va en el archivo de ajustes,
    /// y se pone al momento en el chat abierto. Los grupos de ventanas, que
    /// van al lado en Windows, aun no estan en el Mac.
    static func filaEscala() -> FilaAjuste {
        var f = FilaAjuste(
            id: "escala", seccion: .general, grupo: "Ventanas", titulo: "Escala de la interfaz",
            ayuda: "El tamaño de todo en el chat. Se aplica al momento; en el chat también con ⌘ + «+», «−» y «0».",
            control: .vista { _ in AnyView(ControlEscalaInterfaz()) })
        f.cambiada = { _ in
            EscalaChat.valida(UserDefaults.standard.object(forKey: EscalaChat.clave) as? Int ?? EscalaChat.porDefecto)
                != EscalaChat.porDefecto
        }
        f.restablecerOtro = { _ in UserDefaults.standard.set(EscalaChat.porDefecto, forKey: EscalaChat.clave) }
        return f
    }

    static func filaPdfAligerar() -> FilaAjuste {
        var f = FilaAjuste(
            id: "pdf-aligerar", seccion: .general, grupo: "Documentos PDF",
            titulo: "Aligerar los PDF al meterlos en un chat",
            ayuda: "Después de meterlo al chat, en segundo plano.",
            control: .interruptor(get: { ($0.pdf ?? ModeloAjustes.pdfDeFabrica).aligerarAlEntrar },
                                  set: { m, v in m.cambiarPdf { $0.aligerarAlEntrar = v } }))
        f.cambiada = { ($0.pdf ?? ModeloAjustes.pdfDeFabrica).aligerarAlEntrar != ModeloAjustes.pdfDeFabrica.aligerarAlEntrar }
        f.restablecerOtro = { m in m.cambiarPdf { $0.aligerarAlEntrar = ModeloAjustes.pdfDeFabrica.aligerarAlEntrar } }
        return f
    }

    static func filaPdfNivel() -> FilaAjuste {
        let niveles: [(String, NivelAligerar)] = [("Exacto", .sinPerdida), ("Medio", .equilibrado),
                                                   ("Chico", .pequeno), ("Máx.", .extremo)]
        var f = FilaAjuste(
            id: "pdf-nivel", seccion: .general, grupo: "Documentos PDF", titulo: "Cuánto se aligera",
            ayuda: "Al entrar solo se usan Exacto y Medio; Chico y Máx. quedan para «Aligerar el PDF» a mano.",
            control: .opcion(niveles.map(\.0),
                             get: { m in niveles.firstIndex { $0.1 == (m.pdf ?? ModeloAjustes.pdfDeFabrica).nivel } ?? 1 },
                             set: { m, i in m.cambiarPdf { $0.nivel = niveles[i].1 } }))
        f.cambiada = { ($0.pdf ?? ModeloAjustes.pdfDeFabrica).nivel != ModeloAjustes.pdfDeFabrica.nivel }
        f.restablecerOtro = { m in m.cambiarPdf { $0.nivel = ModeloAjustes.pdfDeFabrica.nivel } }
        return f
    }

    // MARK: Voz

    /// La clave de las voces de Microsoft (la que lee `EdgeVoz.activas`).
    static let claveVocesMicrosoft = "PixPinVocesMicrosoft"
    /// Los idiomas con voz elegible aqui: los de la app y los del lector.
    static let idiomasDeVoz: [(String, String)] = [("es", "español"), ("en", "inglés"), ("pt", "portugués"),
                                                   ("fr", "francés"), ("de", "alemán"), ("it", "italiano")]

    static var voz: [FilaAjuste] {
        let g = "Leer en voz alta"
        var microsoft = FilaAjuste(
            id: "voces-microsoft", seccion: .voz, grupo: g, titulo: "Voces de Microsoft",
            ayuda: "Muy naturales y gratis; no es un servicio oficial y el texto va a Microsoft. Si no contestan, se lee con la voz del iPad.",
            control: .interruptor(get: { _ in vocesMicrosoft },
                                  set: { m, v in
                                      UserDefaults.standard.set(v, forKey: claveVocesMicrosoft)
                                      m.objectWillChange.send()
                                  }))
        microsoft.cambiada = { _ in !vocesMicrosoft }
        microsoft.restablecerOtro = { m in
            UserDefaults.standard.removeObject(forKey: claveVocesMicrosoft)
            m.objectWillChange.send()
        }
        var filas = [microsoft]
        for (lengua, nombre) in idiomasDeVoz {
            var f = FilaAjuste(
                id: "voz-\(lengua)", seccion: .voz, grupo: "Voz del iPad", titulo: "Voz para leer en \(nombre)",
                ayuda: lengua == "es" ? "La que suena sin las voces de Microsoft. Las mejoradas se bajan en Ajustes del iPad › Accesibilidad › Contenido leído › Voces." : "",
                control: .vista { m in AnyView(EleccionVozIPad(lengua: lengua, modelo: m)) })
            f.cambiada = { _ in VozAlta.preferida(lengua: lengua) != nil }
            f.restablecerOtro = { m in
                VozAlta.ponerPreferida(nil, lengua: lengua)
                m.objectWillChange.send()
            }
            filas.append(f)
        }
        filas.append(opcion(.voz, "Notas de voz", "voz-segundo", "Segundo idioma de las notas de voz",
                            "El dictado del iPad prueba también este idioma y se queda con lo que mejor entiende.",
                            \.vozSegundoIdioma, [("Ninguno", ""), ("Inglés", "en"), ("Español", "es"), ("Portugués", "pt")]))
        filas.append(opcion(.voz, "Notas de voz", "voz-modo", "Con dos idiomas",
                            "Qué hacer con lo dicho en el segundo idioma.",
                            \.vozModoDeIdiomas, [("Cada trozo como se dijo", "cada_uno"), ("Todo en el primero", "todo_en_uno")]))
        return filas
    }

    static var vocesMicrosoft: Bool {
        UserDefaults.standard.object(forKey: claveVocesMicrosoft) as? Bool ?? true
    }

    // MARK: Asistente

    static var asistente: [FilaAjuste] {
        let g = "Motor del asistente", v = "Voz del asistente"
        var filas = [
            opcion(.asistente, g, "asistente-motor", "Motor",
                   "En el iPad: gratis y sin red, con Apple Intelligence. Nube: con tu clave (en «Claves de la nube»).",
                   \.asistenteMotor, [("En el iPad", ""), ("Claude", "claude"), ("DeepSeek", "deepseek")]),
            FilaAjuste(id: "asistente-claves", seccion: .asistente, grupo: g, titulo: "Claves de la nube",
                       ayuda: "Se guardan en el Llavero de este iPad, nunca en claro. Sácalas en console.anthropic.com o platform.deepseek.com.",
                       control: .vista { m in AnyView(BotonClavesAsistente(modelo: m)) }),
            opcion(.asistente, g, "asistente-pensar", "Pensar antes de contestar",
                   "Antes de actuar, el asistente piensa y escribe lo que entendió. Pensar tarda: en automático solo piensa en las tareas complejas o si se lo pides («piénsalo», «paso a paso»).",
                   \.asistentePensar, [("Automático", nil), ("Siempre", true), ("Nunca", false)]),
            interruptor(.asistente, g, "asistente-respaldo", "Si la nube falla, seguir con el del iPad",
                        "Sin red, con la clave mal o sin saldo, contesta el modelo del iPad (si lo hay) y te avisa en la respuesta.",
                        \.asistenteRespaldoLocal),
            opcion(.asistente, g, "asistente-docs", "Tus documentos y la nube",
                   "Con Claude o DeepSeek, lo que el asistente encuentre en tus PDF, notas y chats sale por internet para contestarte. Con el modelo del iPad nunca sale del iPad.",
                   \.asistenteDocumentosALaNube, [("Preguntar", ""), ("Permitir", "si"), ("No permitir", "no")]),
            interruptor(.asistente, v, "asistente-revisar", "Ver lo dicho antes de enviarlo",
                        "Con el micrófono del chat «Asistente»: lo que dices queda escrito en la caja para corregirlo y lo envías tú. Apagado, se envía solo.",
                        \.asistenteRevisarLoDicho),
        ]
        // Las conexiones (Windows, 17-oct): Google Drive y Calendar para el
        // asistente de la nube, con el ID de cliente del usuario y OAuth.
        for c in iaConexiones() {
            filas.append(FilaAjuste(
                id: "conexion-\(c.id)", seccion: .asistente, grupo: "Conexiones", titulo: "Conectar \(c.nombre)",
                ayuda: "El asistente de la nube podrá buscar, leer y bajar tus archivos (y subir, preguntando). Crea el ID de cliente en \(c.consola).",
                control: .vista { _ in AnyView(FilaConexion(id: c.id)) }))
        }
        return filas
    }

    // MARK: Sincro, Apariencia, Avanzado

    static var sincro: [FilaAjuste] {
        let g = "En la red local"
        return [
            interruptor(.sincro, g, "presencia", "Dejarse encontrar",
                        "Tus otros aparatos ven este iPad en cuanto abren Sincronizar. Apagado no se abre ninguna puerta en la red.",
                        \.sincroPresencia),
            interruptor(.sincro, g, "automatica", "Sincronizar sola con los aparatos del grupo",
                        "Al aparecer un aparato del grupo en la Wi‑Fi, 30 s después de un cambio y cada 10 min, mientras PixPin está abierto; también con las personas de los chats compartidos. Se pausa en modo de bajo consumo.",
                        \.sincroAutomatica),
            interruptor(.sincro, g, "lo-mio", "Lo mío manda",
                        "Al sincronizar, lo de este iPad se impone: del otro lado solo llega lo que aquí no existe. Es como empieza el interruptor de «¿Qué sincronizar?».",
                        \.sincroLoMioManda),
        ]
    }

    static var apariencia: [FilaAjuste] {
        var tema = FilaAjuste(
            id: "tema", seccion: .apariencia, grupo: "Tema", titulo: "Aspecto",
            ayuda: "Claro, oscuro o como el iPad. Es de este iPad: no va en el archivo de ajustes.",
            control: .opcion(Tema.allCases.map(\.nombre),
                             get: { Tema.allCases.firstIndex(of: $0.tema) ?? 0 },
                             set: { m, i in m.tema = Tema.allCases[i] }))
        tema.cambiada = { $0.tema != .sistema }
        tema.restablecerOtro = { $0.tema = .sistema }
        return [tema]
    }

    static var avanzado: [FilaAjuste] {
        let g = "El archivo de ajustes"
        return [
            FilaAjuste(id: "fichero", seccion: .avanzado, grupo: g, titulo: "Archivo de ajustes",
                       ayuda: "pixpinmax.toml, el mismo que PixPin para Windows. Lo que no está aquí sigue ahí, con sus comentarios.",
                       control: .vista { m in AnyView(TextoAjuste((m.ficheroAjustes as NSString).lastPathComponent)) }),
            FilaAjuste(id: "datos", seccion: .avanzado, grupo: g, titulo: "Carpeta de datos",
                       ayuda: "Proyectos, capturas, sincronización y el archivo de ajustes viven dentro de PixPin en este iPad (en Archivos › En mi iPad › PixPin), con la misma estructura que en Windows.",
                       control: .vista { m in AnyView(TextoAjuste(m.carpetaDatos.path)) }),
            FilaAjuste(id: "version", seccion: .avanzado, grupo: "PixPin", titulo: "Versión",
                       ayuda: "", control: .vista { _ in AnyView(TextoAjuste(ModeloAjustes.version)) }),
        ]
    }

    // MARK: Buscar y restablecer

    /// Sin tildes ni mayusculas, para buscar.
    nonisolated static func plano(_ s: String) -> String {
        s.folding(options: [.diacriticInsensitive, .caseInsensitive], locale: Locale(identifier: "es"))
    }

    /// Si la fila habla de `busqueda` (todas las palabras, en el titulo, la
    /// explicacion, el grupo o la seccion).
    public static func coincide(_ f: FilaAjuste, _ busqueda: String) -> Bool {
        let texto = plano([f.titulo, f.ayuda, f.grupo, f.seccion.titulo].joined(separator: " "))
        let palabras = plano(busqueda).split(whereSeparator: \.isWhitespace)
        return !palabras.isEmpty && palabras.allSatisfy { texto.contains($0) }
    }

    /// Las que coinciden de TODAS las secciones, en su orden.
    public static func buscar(_ busqueda: String) -> [FilaAjuste] {
        todas.filter { coincide($0, busqueda) }
    }

    /// Si algo de la seccion no esta como de fabrica.
    public static func hayCambios(en s: SeccionAjustes, _ m: ModeloAjustes) -> Bool {
        de(s).contains { $0.restablecible && $0.cambiada(m) }
    }

    /// «Restablecer <seccion>»: lo del TOML en un solo paso de «Deshacer».
    public static func restablecer(_ s: SeccionAjustes, _ m: ModeloAjustes) {
        let filas = de(s)
        let fabrica = m.fabrica
        m.cambiar { a in
            for f in filas { f.restablecer?(&a, fabrica) }
        }
        let deOrganizar = filas.filter { $0.restablecerOrganizar != nil && $0.cambiada(m) }
        if !deOrganizar.isEmpty {
            let fabricaOrganizar = m.fabricaOrganizar
            m.cambiarOrganizar { a in
                for f in deOrganizar { f.restablecerOrganizar?(&a, fabricaOrganizar) }
            }
        }
        for f in filas where f.cambiada(m) { f.restablecerOtro?(m) }
    }

    /// Vuelve una fila a fabrica.
    public static func restablecer(_ f: FilaAjuste, _ m: ModeloAjustes) {
        if let r = f.restablecer {
            let fabrica = m.fabrica
            m.cambiar { r(&$0, fabrica) }
        }
        if let r = f.restablecerOrganizar {
            let fabrica = m.fabricaOrganizar
            m.cambiarOrganizar { r(&$0, fabrica) }
        }
        f.restablecerOtro?(m)
    }

    /// El `Almacen` de la app, para lo que lo necesita (las claves de la
    /// nube). Lo pone la vista de Ajustes.
    static weak var almacen: Almacen?
}

// MARK: - Piezas de las filas

/// Un valor que solo se lee (la carpeta, la version), seleccionable.
struct TextoAjuste: View {
    let texto: String
    init(_ texto: String) { self.texto = texto }

    var body: some View {
        Text(texto)
            .font(.caption.monospaced())
            .foregroundStyle(.secondary)
            .textSelection(.enabled)
    }
}

/// **La voz del iPad para un idioma** (`Preferencias::voz_de` de Windows):
/// «Automática» o una de las del sistema, con «Escuchar».
struct EleccionVozIPad: View {
    let lengua: String
    @ObservedObject var modelo: ModeloAjustes

    private static let altavoz = AVSpeechSynthesizer()

    private var voces: [AVSpeechSynthesisVoice] {
        AVSpeechSynthesisVoice.speechVoices()
            .filter { VozAlta.partes($0.language).lengua == lengua }
            .sorted {
                if $0.quality.rawValue != $1.quality.rawValue { return $0.quality.rawValue > $1.quality.rawValue }
                return $0.name.localizedStandardCompare($1.name) == .orderedAscending
            }
    }

    var body: some View {
        let lista = voces
        HStack {
            if lista.isEmpty {
                Text("El iPad no tiene voces de este idioma.").font(.caption).foregroundStyle(.secondary)
            } else {
                Picker("Voz", selection: Binding(
                    get: { VozAlta.preferida(lengua: lengua) ?? "" },
                    set: { nueva in
                        VozAlta.ponerPreferida(nueva.isEmpty ? nil : nueva, lengua: lengua)
                        modelo.objectWillChange.send()
                    })) {
                    Text("Automática (la mejor del iPad)").tag("")
                    ForEach(lista, id: \.identifier) { v in
                        Text("\(VozAlta.nombre(de: v)) · \(v.language)").tag(v.identifier)
                    }
                }
                .pickerStyle(.menu)
                .labelsHidden()
                Spacer()
                Button("Escuchar", action: escuchar)
                    .buttonStyle(.borderless)
            }
        }
    }

    private func escuchar() {
        let muestras = ["es": "Hola, así leo tus documentos.", "en": "Hello, this is how I read your documents.",
                        "pt": "Olá, é assim que leio os teus documentos.", "fr": "Bonjour, voici comment je lis vos documents.",
                        "de": "Hallo, so lese ich deine Dokumente.", "it": "Ciao, così leggo i tuoi documenti."]
        let frase = AVSpeechUtterance(string: muestras[lengua] ?? "PixPin")
        frase.voice = VozAlta.voz(para: lengua, preferida: VozAlta.preferida(lengua: lengua), voces: voces)
        Self.altavoz.stopSpeaking(at: .immediate)
        Self.altavoz.speak(frase)
    }
}

/// «Claves de la nube…»: la hoja de las claves del asistente (la misma que
/// abre el chat «Asistente»). Al cerrarla se relee el TOML (el motor se
/// pudo cambiar alli).
struct BotonClavesAsistente: View {
    @ObservedObject var modelo: ModeloAjustes
    @State private var abierta = false

    var body: some View {
        HStack {
            Spacer()
            Button("Claves de la nube…") { abierta = true }
                .buttonStyle(.borderless)
                .disabled(FilasAjustes.almacen == nil)
        }
        .sheet(isPresented: $abierta, onDismiss: { Task { await modelo.cargar() } }) {
            if let almacen = FilasAjustes.almacen {
                HojaClaves(almacen: almacen)
            }
        }
    }
}

/// La escala de la interfaz en Ajustes → General → Ventanas (la del chat,
/// `EscalaChat`), como el Mac (`VistaAjustes.swift`).
struct ControlEscalaInterfaz: View {
    @AppStorage(EscalaChat.clave) private var escala = EscalaChat.porDefecto

    var body: some View {
        Picker("", selection: Binding(get: { EscalaChat.valida(escala) }, set: { escala = $0 })) {
            ForEach(EscalaChat.escalones, id: \.self) { n in
                Text(verbatim: "\(n) %").tag(n)
            }
        }
        .labelsHidden()
        .pickerStyle(.menu)
        .fixedSize()
    }
}
