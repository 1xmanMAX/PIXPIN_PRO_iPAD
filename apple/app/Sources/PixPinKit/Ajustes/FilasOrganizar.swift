// **Ajustes → Pinear, Tareas y Lienzo** (`ventana_ajustes.rs` de Windows,
// `Seccion::Pinear`, `Tareas` y `Lienzo`): las palabras magicas de cada
// herramienta, las palabras de cada urgencia, el iman y su radio, el
// suavizado del trazo y que herramientas salen y donde. Todo en
// `pixpinmax.toml` por `Nucleo.ajustesOrganizar`/`guardarAjustesOrganizar`
// (`ModeloAjustes.cambiarOrganizar`), con los mismos textos y rangos.

import PixPinCore
import SwiftUI

extension FilasAjustes {
    // MARK: Piezas

    /// Una fila de un valor de `AjustesOrganizar`: sabe si cambio y vuelve.
    static func org<V: Equatable>(_ s: SeccionAjustes, _ grupo: String, _ id: String, _ titulo: String,
                                  _ ayuda: String, _ kp: WritableKeyPath<AjustesOrganizar, V>,
                                  control: ControlAjuste) -> FilaAjuste {
        var f = FilaAjuste(id: id, seccion: s, grupo: grupo, titulo: titulo, ayuda: ayuda, control: control)
        f.cambiada = { $0.organizarActual[keyPath: kp] != $0.fabricaOrganizar[keyPath: kp] }
        f.restablecerOrganizar = { a, fabrica in a[keyPath: kp] = fabrica[keyPath: kp] }
        return f
    }

    static func interruptorOrg(_ s: SeccionAjustes, _ grupo: String, _ id: String, _ titulo: String,
                               _ ayuda: String, _ kp: WritableKeyPath<AjustesOrganizar, Bool>) -> FilaAjuste {
        org(s, grupo, id, titulo, ayuda, kp,
            control: .interruptor(get: { $0.organizarActual[keyPath: kp] },
                                  set: { m, v in m.cambiarOrganizar { $0[keyPath: kp] = v } }))
    }

    /// Lo escrito en una caja de palabras, partido como en Windows: por
    /// comas y renglones, sin blancos ni vacias.
    nonisolated static func palabras(de escrito: String) -> [String] {
        escrito.split(whereSeparator: { $0 == "," || $0 == "\n" })
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }

    // MARK: Pinear

    /// Las herramientas de las palabras magicas, con su nombre (el del menu
    /// «Convertir en…» de Windows, `pin-herramienta-*`).
    static let herramientasMagicas: [(String, String)] = [
        ("TIMER", "Temporizador"), ("STOPWATCH", "Cronómetro"), ("CHECKLIST", "Lista de tareas"),
        ("COUNTER", "Contador"), ("LEDGER", "Gastos"), ("BOARD", "Pizarra"), ("RULETA", "Ruleta"),
        ("DRAW", "Lienzo"), ("SHEET", "Hoja"),
    ]

    static var pinear: [FilaAjuste] {
        let g = "Palabras mágicas"
        var filas: [FilaAjuste] = []
        for (app, nombre) in herramientasMagicas {
            var f = FilaAjuste(
                id: "magia-\(app.lowercased())", seccion: .pinear, grupo: g, titulo: nombre,
                ayuda: "",
                control: .vista { m in
                    AnyView(CajaPalabrasAjuste(
                        modelo: m, ninguna: "Ninguna",
                        leer: { palabrasMagicasDe(guardadas: AjustesOrganizarIPad.palabrasGuardadas($0.organizarActual), app: app) },
                        poner: { m, escrito in
                            m.cambiarOrganizar { a in
                                a.palabrasMagicas = palabrasMagicasCambiadas(
                                    guardadas: AjustesOrganizarIPad.palabrasGuardadas(a), app: app, escrito: escrito)
                                a.palabrasPropias = true
                            }
                        }))
                })
            f.cambiada = { m in
                palabrasMagicasDe(guardadas: AjustesOrganizarIPad.palabrasGuardadas(m.organizarActual), app: app)
                    != palabrasMagicasDe(guardadas: nil, app: app)
            }
            f.restablecerOrganizar = { a, _ in
                let deFabrica = palabrasMagicasDe(guardadas: nil, app: app).joined(separator: ",")
                a.palabrasMagicas = palabrasMagicasCambiadas(
                    guardadas: AjustesOrganizarIPad.palabrasGuardadas(a), app: app, escrito: deFabrica)
                a.palabrasPropias = true
            }
            filas.append(f)
        }
        filas.append(FilaAjuste(
            id: "magia-fabrica", seccion: .pinear, grupo: g, titulo: "Palabras de fábrica",
            ayuda: "Copia una sola y pinéala: se abre la herramienta. Sepáralas con comas.",
            control: .vista { m in
                AnyView(HStack {
                    Spacer()
                    Button("De fábrica") {
                        m.cambiarOrganizar { $0.palabrasPropias = false; $0.palabrasMagicas = "" }
                    }
                    .buttonStyle(.borderless)
                    .disabled(!m.organizarActual.palabrasPropias)
                })
            }))
        return filas
    }

    // MARK: Tareas

    static var tareas: [FilaAjuste] {
        let g = "Palabras de urgencia (sin ninguna, la tarea es normal)"
        let urgencias: [(String, String, WritableKeyPath<AjustesOrganizar, [String]>)] = [
            ("tareas-urgente", "Urgente", \.tareasUrgente),
            ("tareas-muy-urgente", "Muy urgente", \.tareasMuyUrgente),
        ]
        return urgencias.map { id, titulo, kp in
            org(.tareas, g, id, titulo, "", kp, control: .vista { m in
                AnyView(CajaPalabrasAjuste(
                    modelo: m, ninguna: "Ninguna: nada se marca con esta urgencia",
                    leer: { $0.organizarActual[keyPath: kp] },
                    poner: { m, escrito in
                        let nuevas = FilasAjustes.palabras(de: escrito)
                        m.cambiarOrganizar { $0[keyPath: kp] = nuevas }
                    }))
            })
        }
    }

    // MARK: Lienzo

    /// Los sitios con barra propia (`herramientas::SITIOS`), con su nombre.
    static var sitiosHerramientas: [(String, String)] { FilasAjustesSitios.todos }

    /// El sitio elegido arriba de las herramientas (`nil`: en todos).
    static func sitioElegido(_ m: ModeloAjustes) -> String? {
        let i = m.sitioHerramientas - 1
        return sitiosHerramientas.indices.contains(i) ? sitiosHerramientas[i].0 : nil
    }

    /// Los nombres de Windows de las herramientas (`herramientas-<nombre>`).
    static let titulosHerramientas: [String: String] = [
        "lazo": "Lazo", "bolita": "Bolita: pasa por encima para elegir", "zona": "Zona (copia recortada)",
        "lapiz": "Lápiz", "grafito": "Grafito", "resaltador": "Resaltador", "relleno": "Bote de relleno",
        "rectangulo": "Rectángulo", "elipse": "Elipse", "rombo": "Rombo", "arco": "Arco",
        "flecha": "Flecha", "flecha-libre": "Flecha a mano alzada", "linea": "Línea",
        "flecha-codos": "Flecha de codos", "recortar": "Recortar", "extender": "Extender",
        "nudo": "Soldar vértices", "copiar-estilo": "Copiar estilo", "texto": "Texto",
        "serie": "Pasos numerados", "punto": "Punto con letra", "angulo": "Marca de ángulo",
        "mosaico": "Mosaico", "lupa": "Lupa", "foco": "Foco", "laser": "Puntero láser", "cota": "Cota",
        "escalar": "Calibrar la escala", "escala-grafica": "Escala gráfica", "marco": "Marco",
        "imagen": "Imagen", "borrador": "Goma", "figuras": "Figuras", "compartir": "Compartir",
        "imprimir": "Imprimir",
    ]

    /// **Las herramientas por grupos, como en la barra**
    /// (`permitidas::secciones_de_ajustes`): primero las que van siempre a
    /// la vista y luego cada grupo con las suyas, con su nombre del TOML.
    static var gruposDeHerramientas: [(String, [String])] {
        var vistos = Set<String>()
        func nombres(_ ids: [String]) -> [String] {
            ids.compactMap { AjustesOrganizarIPad.nombreEnToml($0) }.filter { vistos.insert($0).inserted }
        }
        var v: [(String, [String])] = [("Siempre a la vista", nombres(["lapiz", "borrador", "figuras"]))]
        for g in GrupoHerramientaLienzo.allCases {
            let n = nombres(g.miembros.map(\.id))
            if !n.isEmpty { v.append((g.titulo, n)) }
        }
        let resto = nombresDeHerramientas().filter { !vistos.contains($0) }
        if !resto.isEmpty { v.append(("Otras", resto)) }
        return v
    }

    static var lienzo: [FilaAjuste] {
        let gi = "Imán", gt = "Trazo a mano"
        var filas: [FilaAjuste] = [
            interruptorOrg(.lienzo, gi, "iman", "Pegar a los puntos notables",
                           "Al dibujar, el cursor se pega a lo que ya hay.", \.imanActivo),
            interruptorOrg(.lienzo, gi, "iman-esquinas", "Esquinas y extremos",
                           "Las esquinas de las formas y los extremos de las líneas.", \.imanEsquinas),
            interruptorOrg(.lienzo, gi, "iman-medios", "Puntos medios", "El punto medio de cada lado.", \.imanMedios),
            interruptorOrg(.lienzo, gi, "iman-centros", "Centros", "El centro de cada forma.", \.imanCentros),
            // Por debajo de 6 px el iman no llega a nada; por encima de 40
            // agarra cosas que no se estaban mirando.
            org(.lienzo, gi, "iman-radio", "Distancia a la que agarra", "En píxeles de pantalla.", \.imanRadioPx,
                control: .numero(6 ... 40, paso: 2, texto: { "\($0) px" },
                                 get: { Int($0.organizarActual.imanRadioPx) },
                                 set: { m, n in m.cambiarOrganizar { $0.imanRadioPx = UInt32(min(max(n, 6), 40)) } })),
            org(.lienzo, gt, "suavizado", "Suavizado del trazo",
                "Natural quita el temblor de la mano parada sin retrasar la punta.", \.suavizado,
                control: .opcion(["Excalidraw", "Natural"],
                                 get: { $0.organizarActual.suavizado == "natural" ? 1 : 0 },
                                 set: { m, i in m.cambiarOrganizar { $0.suavizado = i == 1 ? "natural" : "excalidraw" } })),
            FilaAjuste(id: "herramientas-donde", seccion: .lienzo, grupo: "Herramientas", titulo: "Herramientas en",
                       ayuda: "Elige un sitio para apagar ahí las que no usas sin quitarlas de los demás.",
                       control: .opcion(["Todos"] + sitiosHerramientas.map(\.1),
                                        get: { $0.sitioHerramientas },
                                        set: { m, i in m.sitioHerramientas = i })),
        ]
        for (grupo, nombres) in gruposDeHerramientas {
            for n in nombres {
                var f = FilaAjuste(
                    id: "herramienta-\(n)", seccion: .lienzo, grupo: grupo,
                    titulo: titulosHerramientas[n] ?? n, ayuda: "",
                    control: .interruptor(
                        get: { m in HerramientasAjustes.activa(n, en: sitioElegido(m), m.organizarActual) },
                        set: { m, _ in
                            let sitio = sitioElegido(m)
                            m.cambiarOrganizar { HerramientasAjustes.alternar(n, en: sitio, &$0) }
                        }))
                f.cambiada = { m in !HerramientasAjustes.activaEnTodos(n, m.organizarActual) }
                f.restablecerOrganizar = { a, _ in HerramientasAjustes.encenderEnTodos(n, &a) }
                filas.append(f)
            }
        }
        return filas
    }
}

/// **Encender y apagar herramientas** (`alternar_herramienta` de Windows),
/// puro: en todos los sitios o en uno solo.
public enum HerramientasAjustes {
    static func esta(_ l: [String], _ n: String) -> Bool {
        l.contains { $0.trimmingCharacters(in: .whitespaces).caseInsensitiveCompare(n) == .orderedSame }
    }

    static func poner(_ n: String, _ activa: Bool, en l: inout [String]) {
        l.removeAll { $0.trimmingCharacters(in: .whitespaces).caseInsensitiveCompare(n) == .orderedSame }
        if !activa { l.append(n) }
    }

    /// Si sale en `sitio` (`nil`: en la lista de todos).
    public static func activa(_ n: String, en sitio: String?, _ a: AjustesOrganizar) -> Bool {
        if esta(a.herramientasApagadas, n) { return false }
        guard let sitio else { return true }
        return !esta(a.herramientasPorSitio[sitio] ?? [], n)
    }

    /// Si no esta apagada en ningun sitio.
    public static func activaEnTodos(_ n: String, _ a: AjustesOrganizar) -> Bool {
        !esta(a.herramientasApagadas, n) && !a.herramientasPorSitio.values.contains { esta($0, n) }
    }

    /// **Lo que hace tocar su interruptor**. Encender en un sitio una que
    /// estaba apagada en TODOS la deja encendida solo ahi: sale de la lista
    /// general y se apaga en los demas sitios, que asi siguen como estaban.
    /// Un sitio que se queda sin apagadas se queda con la lista vacia (el
    /// guardado que conserva el fichero no borra claves).
    public static func alternar(_ n: String, en sitio: String?, _ a: inout AjustesOrganizar) {
        guard let sitio else {
            poner(n, !activa(n, en: nil, a), en: &a.herramientasApagadas)
            return
        }
        if activa(n, en: sitio, a) {
            var l = a.herramientasPorSitio[sitio] ?? []
            poner(n, false, en: &l)
            a.herramientasPorSitio[sitio] = l
            return
        }
        if esta(a.herramientasApagadas, n) {
            poner(n, true, en: &a.herramientasApagadas)
            for (otro, _) in FilasAjustesSitios.todos where otro != sitio {
                var l = a.herramientasPorSitio[otro] ?? []
                poner(n, false, en: &l)
                a.herramientasPorSitio[otro] = l
            }
        }
        var l = a.herramientasPorSitio[sitio] ?? []
        poner(n, true, en: &l)
        a.herramientasPorSitio[sitio] = l
    }

    /// La vuelve a encender en todos los sitios.
    public static func encenderEnTodos(_ n: String, _ a: inout AjustesOrganizar) {
        poner(n, true, en: &a.herramientasApagadas)
        for s in Array(a.herramientasPorSitio.keys) {
            var l = a.herramientasPorSitio[s] ?? []
            poner(n, true, en: &l)
            a.herramientasPorSitio[s] = l
        }
    }
}

/// Los sitios con barra propia, sin pasar por el actor principal.
enum FilasAjustesSitios {
    static let todos: [(String, String)] = [
        ("pantalla", "Pantalla"), ("pin", "Pines"), ("lienzo", "Lienzo"), ("lector", "Lector"),
    ]
}

/// **La caja de unas palabras** (las magicas de una herramienta, las de una
/// urgencia): lo de ahora debajo y una caja para escribirlas separadas por
/// comas, con «Cambiar», como el `Control::Entrada` de Windows.
struct CajaPalabrasAjuste: View {
    @ObservedObject var modelo: ModeloAjustes
    let ninguna: String
    let leer: @MainActor (ModeloAjustes) -> [String]
    let poner: @MainActor (ModeloAjustes, String) -> Void
    @State private var escrito = ""
    @State private var editando = false

    var body: some View {
        let ahora = leer(modelo)
        VStack(alignment: .leading, spacing: 6) {
            Text(ahora.isEmpty ? ninguna : ahora.joined(separator: ", "))
                .font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            if editando {
                HStack {
                    TextField("palabra, otra palabra…", text: $escrito)
                        .textFieldStyle(.roundedBorder)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onSubmit(aceptar)
                    Button("Cambiar", action: aceptar).buttonStyle(.borderless)
                    Button("Cancelar") { editando = false }.buttonStyle(.borderless)
                }
            } else {
                HStack {
                    Spacer()
                    Button("Cambiar…") {
                        escrito = ahora.joined(separator: ", ")
                        editando = true
                    }
                    .buttonStyle(.borderless)
                }
            }
        }
    }

    private func aceptar() {
        poner(modelo, escrito)
        editando = false
    }
}
