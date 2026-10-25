// **Los verbos** (`orden::VERBOS` de Windows): una tabla, no un `switch`
// por verbo, para que los enlaces, la ayuda y los Servicios del Finder
// salgan de la misma fila. Cada fila arma los pedidos del protocolo; los
// nombres y alias son los de Windows y Android (`Atajos.kt`), asi que
// `pixpinmax://tarea?texto=…` y `pixpin://atajo/tarea?texto=…` dan lo mismo
// en el PC, el movil y el Mac.

import Foundation

/// Lo que hay que hacer.
public enum Orden: Equatable, Sendable {
    /// Pedidos del protocolo, en orden.
    case pedidos([Pedido])
    /// Leer el texto de unas imagenes (`ocr`, solo de la linea de ordenes).
    case ocr(rutas: [String], copiar: Bool)
    /// Preguntar al asistente por unos ficheros: falta la pregunta, que se
    /// pide en una caja antes de mandar nada (los Servicios del Finder).
    case preguntar(rutas: [String], proyecto: String?)
    /// La ayuda.
    case ayuda
}

public enum Verbos {
    /// Las extensiones que se tratan como imagen (`EXTENSIONES_IMAGEN`).
    public static let extensionesImagen: Set<String> = ["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif"]
    /// Las opciones que no llevan valor.
    public static let banderas: Set<String> = ["copiar", "sin-arrancar"]

    /// Lo que se le pasa a cada fila.
    public struct Entrada {
        /// Lo suelto detras del verbo (o los trozos del camino del enlace).
        public var sueltos: [String]
        /// `--clave valor`; `?clave=valor` en un enlace.
        public var campos: [String: String]
        /// Si una ruta es un fichero que existe. En un enlace siempre
        /// `false`: alli una ruta solo entra con su nombre (`ruta=`), y asi
        /// pasa por `Enlaces.seguridad`.
        public var esFichero: (String) -> Bool
        /// Si viene de un enlace.
        public var enlace: Bool

        func campo(_ c: String) -> String? {
            guard let t = campos[c]?.trimmingCharacters(in: .whitespacesAndNewlines), !t.isEmpty else { return nil }
            return t
        }

        /// El proyecto, como lo quiere el protocolo (nulo = Mensajes
        /// guardados). El nombre lo cambia por su id `HacerPedidos`.
        var proyecto: ValorPedido { .t(campo("proyecto")) }

        /// Lo suelto partido en ficheros (los que existen) y texto, mas el
        /// campo `texto`.
        func textoYFicheros() -> (String, [String]) {
            var texto: [String] = []
            var ficheros: [String] = []
            for s in sueltos {
                if !enlace && esFichero(s) { ficheros.append(s) } else { texto.append(s) }
            }
            var t = texto.joined(separator: " ")
            if let c = campo("texto") {
                if !t.isEmpty { t += " " }
                t += c
            }
            return (t.trimmingCharacters(in: .whitespacesAndNewlines), ficheros)
        }

        /// Solo ficheros: lo suelto que no lo sea es un error que se dice.
        func soloFicheros() throws -> [String] {
            var v: [String] = []
            for s in sueltos {
                guard !enlace, esFichero(s) else { throw FalloOrden.fallo("no existe el fichero «\(s)»") }
                v.append(s)
            }
            if let r = campo("ruta") { v.append(r) }
            return v
        }

        /// El texto suelto, o el campo con nombre.
        func textoO(_ c: String) -> String? {
            let t = sueltos.joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines)
            return t.isEmpty ? campo(c) : t
        }
    }

    /// Una fila de la tabla.
    public struct Fila {
        /// El nombre y sus alias (el primero sale en la ayuda).
        public let nombres: [String]
        public let uso: String
        public let ayuda: String
        /// Las `--opciones` que admite (sin guiones).
        public let opciones: [String]
        /// Si se puede pedir por enlace.
        public let porEnlace: Bool
        let armar: (Entrada) throws -> Orden
    }

    /// **La tabla.** El orden es el de la ayuda.
    public static let tabla: [Fila] = [
        Fila(nombres: ["capturar", "capture"], uso: "capturar", ayuda: "Captura una zona de la pantalla, como el atajo general.",
             opciones: [], porEnlace: true) { _ in uno("capturar") },
        Fila(nombres: ["pinear", "pin"], uso: "pinear <imagen|fichero>… | --codigo <id> [--proyecto P]",
             ayuda: "Saca a la pantalla como pin cada fichero (o el del mensaje <id> del chat).",
             opciones: ["codigo", "proyecto", "ruta"], porEnlace: true, armar: armarPinear),
        Fila(nombres: ["pinear-ultima", "ultima"], uso: "pinear-ultima", ayuda: "Saca como pin la captura mas reciente.",
             opciones: [], porEnlace: true) { _ in uno("pinear_ultima") },
        Fila(nombres: ["archivo"], uso: "archivo <id del mensaje> [--proyecto P]",
             ayuda: "Saca a la pantalla el fichero de ese mensaje del chat (pixpin://atajo/archivo/<id> de Android).",
             opciones: ["proyecto", "id"], porEnlace: true, armar: armarArchivo),
        Fila(nombres: ["chat", "mensaje"], uso: "chat [texto] [fichero…] [--proyecto P]",
             ayuda: "Escribe en el chat (sin proyecto, en Mensajes guardados) el texto y los ficheros; sin nada, abre ese chat.",
             opciones: ["proyecto", "texto"], porEnlace: true, armar: armarChat),
        Fila(nombres: ["mensajes", "guardados"], uso: "mensajes", ayuda: "Abre Mensajes guardados.",
             opciones: [], porEnlace: true) { _ in uno("abrir", ["que": .objeto(["tipo": .texto("proyecto"), "proyecto": .nulo])]) },
        Fila(nombres: ["proyectos"], uso: "proyectos", ayuda: "Abre la ventana principal (el chat de proyectos).",
             opciones: [], porEnlace: true) { _ in uno("ventana_principal") },
        Fila(nombres: ["proyecto"], uso: "proyecto <id|nombre>", ayuda: "Abre el chat de ese proyecto.",
             opciones: ["id", "proyecto"], porEnlace: true, armar: armarProyecto),
        Fila(nombres: ["tarea", "task", "todo"], uso: "tarea \"<texto>\" [--proyecto P] [--lista <id>]",
             ayuda: "Apunta una tarea (sin proyecto ni lista, en el Inbox); sin texto, abre las tareas.",
             opciones: ["proyecto", "lista", "texto"], porEnlace: true, armar: armarTarea),
        Fila(nombres: ["tareas", "tasks"], uso: "tareas", ayuda: "Abre la ventana de tareas.",
             opciones: [], porEnlace: true) { _ in ventana("tareas") },
        Fila(nombres: ["capturas", "galeria"], uso: "capturas", ayuda: "Abre la galeria de capturas.",
             opciones: [], porEnlace: true) { _ in ventana("galeria") },
        Fila(nombres: ["nota", "note"], uso: "nota [texto] [imagen…] [--proyecto P]",
             ayuda: "Abre una nota nueva en el editor, empezada con el texto.",
             opciones: ["proyecto", "texto"], porEnlace: true) { try textoEImagenes($0, accion: "nota_nueva", que: "una nota") },
        Fila(nombres: ["lienzo", "dibujo"], uso: "lienzo [nombre] [--proyecto P]", ayuda: "Abre un lienzo nuevo.",
             opciones: ["proyecto", "nombre"], porEnlace: true) { e in
            uno("lienzo_nuevo", ["nombre": .t(e.textoO("nombre")), "proyecto": e.proyecto])
        },
        Fila(nombres: ["lista"], uso: "lista [titulo] [--proyecto P]", ayuda: "Crea una lista de tareas nueva.",
             opciones: ["proyecto", "titulo"], porEnlace: true) { e in
            // «Tareas», como `Atajos.LISTA` de Android cuando no dice titulo.
            uno("lista_nueva", ["titulo": .texto(e.textoO("titulo") ?? "Tareas"), "proyecto": e.proyecto])
        },
        Fila(nombres: ["leccion"], uso: "leccion [texto] [imagen…] [--proyecto P]",
             ayuda: "Abre la ficha de una leccion aprendida nueva.",
             opciones: ["proyecto", "texto"], porEnlace: true) { try textoEImagenes($0, accion: "leccion_nueva", que: "una leccion") },
        Fila(nombres: ["lecciones"], uso: "lecciones [buscar] [--proyecto P]",
             ayuda: "Abre la lista de lecciones (ya buscando, con texto).",
             opciones: ["proyecto", "consulta"], porEnlace: true) { e in
            uno("lecciones", ["consulta": .t(e.textoO("consulta")), "proyecto": e.proyecto])
        },
        Fila(nombres: ["grabar", "voz"], uso: "grabar [nombre] [--proyecto P]",
             ayuda: "Saca el microfono flotante y graba una nota de voz.",
             opciones: ["proyecto", "nombre"], porEnlace: true) { e in
            uno("grabar", ["nombre": .texto(e.textoO("nombre") ?? "Nota de voz"), "proyecto": e.proyecto])
        },
        Fila(nombres: ["soltar"], uso: "soltar [--proyecto P]",
             ayuda: "Saca la caja flotante a la que soltar ficheros para ese chat.",
             opciones: ["proyecto"], porEnlace: true) { e in uno("soltar", ["proyecto": e.proyecto]) },
        Fila(nombres: ["sincronizar", "sync"], uso: "sincronizar [todos|<aparato>]",
             ayuda: "Sin nada, abre Sincronizar; con «todos» o un aparato, sincroniza ya.",
             opciones: ["aparato"], porEnlace: true) { e in
            // Sin aparato, la pantalla (el atajo de Android y la Jump List).
            if let a = e.sueltos.first ?? e.campo("aparato") { return uno("sincronizar", ["aparato": .texto(a)]) }
            return ventana("sincronizar")
        },
        Fila(nombres: ["al-movil", "movil"], uso: "al-movil <fichero>… [--aparato <id>]",
             ayuda: "Manda los ficheros al movil (a todos si no se dice): imagenes al lienzo, lo demas al chat.",
             opciones: ["aparato"], porEnlace: false, armar: armarAlMovil),
        Fila(nombres: ["ocr", "texto"], uso: "ocr <imagen>… [--copiar]",
             ayuda: "Escribe en la consola el texto de las imagenes (y lo copia con --copiar).",
             opciones: [], porEnlace: false) { e in
            .ocr(rutas: try soloImagenes(e, "ocr"), copiar: e.campos["copiar"] != nil)
        },
        Fila(nombres: ["copiar-texto"], uso: "copiar-texto <imagen>…",
             ayuda: "PixPin lee el texto de las imagenes, lo copia y lo dice en su aviso (los Servicios del Finder).",
             opciones: [], porEnlace: false) { e in
            let rutas = try soloImagenes(e, "copiar-texto")
            var campos: [String: ValorPedido] = ["ruta": .texto(rutas[0])]
            if rutas.count > 1 { campos["rutas"] = .textos(Array(rutas.dropFirst())) }
            return uno("ocr", campos)
        },
        Fila(nombres: ["leer", "leer-en-voz"], uso: "leer \"<texto>\" [--titulo T]",
             ayuda: "Lee el texto en voz alta con la voz de PixPin, sin abrir nada.",
             opciones: ["texto", "titulo"], porEnlace: true) { e in
            guard let t = e.textoO("texto") else { throw FalloOrden.noSeEntiende("leer: falta el texto") }
            return uno("leer_en_voz", ["texto": .texto(t), "titulo": .t(e.campo("titulo"))])
        },
        Fila(nombres: ["asistente", "ia", "preguntar"], uso: "asistente [\"pedido\"] [fichero…] [--proyecto P]",
             ayuda: "Le pide algo al asistente sin abrir el chat. Con ficheros y sin pedido, pregunta que hacer con ellos.",
             opciones: ["proyecto", "texto"], porEnlace: true) { e in
            let (texto, ficheros) = e.textoYFicheros()
            if texto.isEmpty && !ficheros.isEmpty { return .preguntar(rutas: ficheros, proyecto: e.campo("proyecto")) }
            return .pedidos([pedidoAlAsistente(texto, ficheros: ficheros, proyecto: e.campo("proyecto"))])
        },
        Fila(nombres: ["abrir", "open"], uso: "abrir <fichero>… [--pagina N] [--buscar T]",
             ayuda: "Abre cada fichero como «Abrir con PixPin» (lector, editor o pin).",
             opciones: ["pagina", "buscar", "ruta", "tipo", "proyecto", "codigo", "referencia"], porEnlace: true,
             armar: armarAbrir),
        Fila(nombres: ["ventana"], uso: "ventana tareas|galeria|lecciones|sincronizar",
             ayuda: "Abre esa ventana de la bandeja.", opciones: ["cual"], porEnlace: true) { e in
            let cual = (e.sueltos.first ?? e.campo("cual") ?? "").lowercased()
            guard ["tareas", "galeria", "lecciones", "sincronizar"].contains(cual) else {
                throw FalloOrden.noSeEntiende("ventana: tareas, galeria, lecciones o sincronizar")
            }
            return ventana(cual)
        },
        Fila(nombres: ["pedido"], uso: "pedido '<json>'", ayuda: "Manda un pedido del protocolo tal cual.",
             opciones: ["json"], porEnlace: true) { e in
            guard let j = e.sueltos.first ?? e.campo("json") else { throw FalloOrden.noSeEntiende("pedido: falta el JSON") }
            return .pedidos([try Pedido.de(json: j)])
        },
        Fila(nombres: ["ayuda", "help"], uso: "ayuda", ayuda: "Esta ayuda.", opciones: [], porEnlace: false) { _ in .ayuda },
    ]

    /// La fila de un verbo, sin mirar mayusculas.
    public static func fila(_ nombre: String) -> Fila? {
        let n = nombre.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return tabla.first { $0.nombres.contains(n) }
    }

    // MARK: Como en la linea de ordenes

    /// **Lee unos argumentos** como la linea de ordenes de Windows
    /// (`pixpinmax <verbo> … --opcion valor`, `--pedido '<json>'`,
    /// `--url <enlace>`). Lo usan los Servicios del Finder (verbo y rutas)
    /// y las pruebas. `nil` si no es una orden.
    public static func deArgumentos(_ args: [String], esFichero: @escaping (String) -> Bool,
                                    firmas: FirmaEnlaces? = .delMac) -> Result<LlamadaOrden, FalloOrden>? {
        let args = args.filter { $0 != "--sin-arrancar" }
        guard let primero = args.first else { return nil }
        let resto = Array(args.dropFirst())
        func r(_ f: () throws -> Orden) -> Result<LlamadaOrden, FalloOrden> {
            do { return .success(LlamadaOrden(orden: try f(), origen: .consola)) } catch let f as FalloOrden {
                return .failure(f)
            } catch { return .failure(.noSeEntiende("\(error)")) }
        }
        switch primero {
        case "--pedido":
            return r {
                guard let j = resto.first else { throw FalloOrden.noSeEntiende("falta el JSON detras de --pedido") }
                return .pedidos([try Pedido.de(json: j)])
            }
        case "--url":
            guard let u = resto.first else { return .failure(.noSeEntiende("falta el enlace detras de --url")) }
            return Result { try Enlaces.leer(u, firmas: firmas) }.mapError { ($0 as? FalloOrden) ?? .noSeEntiende("\($0)") }
        case "--ayuda", "--help", "-h", "-?", "/?":
            return .success(LlamadaOrden(orden: .ayuda, origen: .consola))
        case let u where Enlaces.esEnlace(u):
            return Result { try Enlaces.leer(u, firmas: firmas) }.mapError { ($0 as? FalloOrden) ?? .noSeEntiende("\($0)") }
        case let o where o.hasPrefix("--"):
            return nil
        default:
            // Una palabra que no es un verbo es el arranque de siempre.
            guard let fila = fila(primero) else { return nil }
            return r { try armarDeConsola(fila, resto, esFichero: esFichero) }
        }
    }

    static func armarDeConsola(_ fila: Fila, _ resto: [String], esFichero: @escaping (String) -> Bool) throws -> Orden {
        var sueltos: [String] = []
        var campos: [String: String] = [:]
        var i = 0
        var soloSueltos = false
        while i < resto.count {
            let a = resto[i]
            i += 1
            if soloSueltos || !a.hasPrefix("--") || a.count == 2 {
                if a == "--" && !soloSueltos {
                    soloSueltos = true
                    continue
                }
                sueltos.append(a)
                continue
            }
            let cuerpo = a.dropFirst(2)
            let clave: String
            var valor: String?
            if let igual = cuerpo.firstIndex(of: "=") {
                clave = cuerpo[..<igual].lowercased()
                valor = String(cuerpo[cuerpo.index(after: igual)...])
            } else {
                clave = cuerpo.lowercased()
            }
            if banderas.contains(clave) {
                campos[clave] = "1"
                continue
            }
            guard fila.opciones.contains(clave) else {
                throw FalloOrden.noSeEntiende("«\(fila.nombres[0])» no tiene la opcion --\(clave) (pixpinmax ayuda)")
            }
            if valor == nil {
                guard i < resto.count else { throw FalloOrden.noSeEntiende("falta el valor de --\(clave)") }
                valor = resto[i]
                i += 1
            }
            campos[clave] = valor
        }
        return try fila.armar(Entrada(sueltos: sueltos, campos: campos, esFichero: esFichero, enlace: false))
    }

    /// La ayuda, sacada de la tabla.
    public static func ayuda() -> String {
        var s = "PixPin Max: pedirle cosas a PixPin desde fuera.\n\n"
            + "Enlaces: pixpinmax://<verbo>?campo=valor (y pixpin://atajo/<verbo> de Android).\n\nVerbos:\n"
        for f in tabla {
            s += "  " + f.uso.padding(toLength: max(52, f.uso.count), withPad: " ", startingAt: 0) + " " + f.ayuda + "\n"
        }
        s += "\n--proyecto acepta el id o el nombre del proyecto. Sin --proyecto, Mensajes guardados.\n"
            + "Ejemplos: pixpinmax://tarea?texto=comprar%20yeso, pixpinmax://proyecto/<id>, pixpinmax://atajo/capturar.\n\n"
            + "Codigos de salida: 0 aceptado, 1 fallo aqui (fichero), 2 version, 3 no se entiende,\n"
            + "6 rechazado (enlace no permitido o dijo que no).\n"
        return s
    }

    // MARK: Las filas

    static func uno(_ accion: String, _ campos: [String: ValorPedido] = [:]) -> Orden {
        .pedidos([Pedido(accion, campos)])
    }

    static func ventana(_ cual: String) -> Orden {
        uno("ventana", ["cual": .texto(cual)])
    }

    /// Si una ruta es de imagen, por su extension.
    public static func esImagen(_ ruta: String) -> Bool {
        extensionesImagen.contains((ruta as NSString).pathExtension.lowercased())
    }

    /// Lo que va al chat como foto (`compartido::es_foto`): un GIF o un
    /// WebP van tal cual, como fichero.
    public static func esFoto(_ ruta: String) -> Bool {
        ["png", "jpg", "jpeg", "bmp"].contains((ruta as NSString).pathExtension.lowercased())
    }

    static func armarPinear(_ e: Entrada) throws -> Orden {
        if let c = e.campo("codigo") { return uno("pinear", ["codigo": .texto(c), "proyecto": e.proyecto]) }
        let rutas = try e.soloFicheros()
        guard !rutas.isEmpty else { throw FalloOrden.noSeEntiende("pinear: falta el fichero (o --codigo)") }
        return .pedidos(rutas.map { Pedido("pinear", ["ruta": .texto($0)]) })
    }

    static func armarArchivo(_ e: Entrada) throws -> Orden {
        guard let id = e.sueltos.first ?? e.campo("id") else { throw FalloOrden.noSeEntiende("archivo: falta el id del mensaje") }
        return uno("pinear", ["codigo": .texto(id), "proyecto": e.proyecto])
    }

    static func armarChat(_ e: Entrada) throws -> Orden {
        let (texto, ficheros) = e.textoYFicheros()
        if texto.isEmpty && ficheros.isEmpty {
            return uno("abrir", ["que": .objeto(["tipo": .texto("proyecto"), "proyecto": e.proyecto])])
        }
        var campos: [String: ValorPedido] = ["texto": .texto(texto), "proyecto": e.proyecto]
        let fotos = ficheros.filter(esFoto)
        let otros = ficheros.filter { !esFoto($0) }
        if !fotos.isEmpty { campos["imagenes"] = .textos(fotos) }
        if !otros.isEmpty { campos["archivos"] = .textos(otros) }
        return uno("chat", campos)
    }

    static func armarProyecto(_ e: Entrada) throws -> Orden {
        let id = (e.sueltos.first ?? e.campo("id") ?? e.campo("proyecto"))?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard let id, !id.isEmpty else { throw FalloOrden.noSeEntiende("proyecto: falta cual") }
        return uno("abrir", ["que": .objeto(["tipo": .texto("proyecto"), "proyecto": .texto(id)])])
    }

    static func armarTarea(_ e: Entrada) throws -> Orden {
        let (texto, ficheros) = e.textoYFicheros()
        if texto.isEmpty && ficheros.isEmpty { return ventana("tareas") }
        if ficheros.contains(where: { !esImagen($0) }) { throw FalloOrden.noSeEntiende("una tarea solo lleva imagenes") }
        // Sin proyecto, el Inbox: el campo ni va (no es «Mensajes guardados»).
        var p = Pedido("anadir_tarea", ["texto": .texto(texto), "codigo": .t(e.campo("lista"))])
        if e.campo("proyecto") != nil { p["proyecto"] = e.proyecto }
        if !ficheros.isEmpty { p["imagenes"] = .textos(ficheros) }
        return .pedidos([p])
    }

    /// Texto e imagenes para `nota_nueva` y `leccion_nueva`.
    static func textoEImagenes(_ e: Entrada, accion: String, que: String) throws -> Orden {
        let (texto, ficheros) = e.textoYFicheros()
        if let f = ficheros.first(where: { !esImagen($0) }) {
            throw FalloOrden.noSeEntiende("\(que) solo lleva imagenes, y «\(f)» no lo es")
        }
        var campos: [String: ValorPedido] = ["texto": .t(texto.isEmpty ? nil : texto), "proyecto": e.proyecto]
        if !ficheros.isEmpty { campos["imagenes"] = .textos(ficheros) }
        return uno(accion, campos)
    }

    static func armarAlMovil(_ e: Entrada) throws -> Orden {
        let ficheros = try e.soloFicheros()
        guard !ficheros.isEmpty else { throw FalloOrden.noSeEntiende("al-movil: faltan los ficheros") }
        return uno("enviar_al_movil", ["aparato": .texto(e.campo("aparato") ?? "todos"),
                                       "imagenes": .textos(ficheros.filter(esImagen)),
                                       "archivos": .textos(ficheros.filter { !esImagen($0) })])
    }

    static func soloImagenes(_ e: Entrada, _ que: String) throws -> [String] {
        let rutas = try e.soloFicheros()
        guard !rutas.isEmpty else { throw FalloOrden.noSeEntiende("\(que): faltan las imagenes") }
        if let r = rutas.first(where: { !esImagen($0) }) { throw FalloOrden.fallo("«\(r)» no es una imagen") }
        return rutas
    }

    /// El pedido `asistente` de un texto y unos ficheros: las imagenes las
    /// ve el modelo; de lo demas se le dan las rutas.
    public static func pedidoAlAsistente(_ texto: String, ficheros: [String], proyecto: String?) -> Pedido {
        var t = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        let otros = ficheros.filter { !esImagen($0) }
        if !otros.isEmpty {
            t += "\n\nArchivos:"
            for o in otros { t += "\n- \(o)" }
        }
        var campos: [String: ValorPedido] = ["texto": .texto(t), "proyecto": .t(proyecto)]
        let imagenes = ficheros.filter(esImagen)
        if !imagenes.isEmpty { campos["imagenes"] = .textos(imagenes) }
        return Pedido("asistente", campos)
    }

    static func armarAbrir(_ e: Entrada) throws -> Orden {
        let proyecto = e.proyecto
        switch e.campo("tipo")?.lowercased() {
        case let t? where t == "mensaje" || t == "nota":
            guard let codigo = e.campo("codigo") else { throw FalloOrden.noSeEntiende("abrir: falta codigo") }
            return uno("abrir", ["que": .objeto(["tipo": .texto(t), "proyecto": proyecto, "codigo": .texto(codigo)])])
        case "hoja"?:
            guard let r = e.campo("referencia") else { throw FalloOrden.noSeEntiende("abrir: falta referencia") }
            return uno("abrir", ["que": .objeto(["tipo": .texto("hoja"), "proyecto": proyecto, "referencia": .texto(r)])])
        case "proyecto"?:
            return uno("abrir", ["que": .objeto(["tipo": .texto("proyecto"), "proyecto": proyecto])])
        case nil, "fichero"?:
            break
        case let otro?:
            throw FalloOrden.noSeEntiende("abrir: no hay tipo «\(otro)»")
        }
        let rutas = try e.soloFicheros()
        guard !rutas.isEmpty else { throw FalloOrden.noSeEntiende("abrir: falta el fichero") }
        var pagina: Int?
        if let p = e.campo("pagina") {
            guard let n = UInt32(p), n >= 1 else { throw FalloOrden.noSeEntiende("--pagina es un numero desde 1") }
            pagina = Int(n)
        }
        let buscar = e.campo("buscar")
        return .pedidos(rutas.map { r in
            var que: [String: ValorPedido] = ["tipo": .texto("fichero"), "ruta": .texto(r)]
            if let pagina { que["pagina"] = .numero(Double(pagina)) }
            if let buscar { que["buscar"] = .texto(buscar) }
            return Pedido("abrir", ["que": .objeto(que)])
        })
    }
}
