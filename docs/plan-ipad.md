# PixPin para iPad: plan y reglas (25-oct-2026)

La app de iPad es **la misma PixPin** que Windows, Mac y Android: el mismo formato de
proyectos, la misma sincronización por la Wi‑Fi y la misma cara. Se construye sobre el
**mismo núcleo de Rust** que el Mac (`apple/nucleo`, copiado de PIXPIN_PRO_MAC
`apple/nucleo` en e96241c y puesto al día con `apple/herramientas/traer-del-mac.py`, ver «Traer el Mac») y reutiliza tal cual las piezas de Swift del Mac que no tocan
AppKit. Lo de AppKit se reescribe con **SwiftUI + UIKit** pensado para el tacto y el
**Apple Pencil**.

## Referencias

| Qué | Dónde |
|---|---|
| La versión de referencia (funciones, aspecto, textos) | PIXPIN_PRO_WINDOWS (`apps/pixpin`, `crates/`) y sus capturas en `docs/img/` |
| El port de Apple más cercano (Swift, mismo núcleo) | PIXPIN_PRO_MAC `apple/app/Sources/PixPinKit` |
| La API del núcleo para Swift (UniFFI) | `apple/PixPinCore/Sources/PixPinCore/pixpin_nucleo.swift` (generado; buscar con grep, son 42 000 líneas) |
| El plan de Apple (decisiones) | PIXPIN_PRO_MAC `docs/investigacion/2026-10-15-mac-ipad-iphone.md` §4.3 y Fase 4 |

## Estructura

```
apple/
  nucleo/           Rust: el núcleo (igual que el Mac; no se toca sin llevarlo al Mac y a Windows)
  PixPinCore/       Paquete Swift con los enlaces de UniFFI y el xcframework (no versionado)
  app/              Paquete Swift `PixPinIPad` → biblioteca `PixPinKit` (toda la app)
    Sources/PixPin/   @main (lo compila el proyecto de Xcode)
    Sources/PixPinKit/
      Raiz/           Navegación, entorno, barra común de visores, enrutador (comunes a todo)
      Almacen.swift   El estado: chats y mensajes por páginas (del Mac, con UIKit)
      Paleta.swift    Los colores de Windows
      <Zona>/         Una carpeta por zona (ver abajo)
    Tests/PixPinKitTests/
  xcode/project.yml XcodeGen: la app de iPad (iPadOS 18+) y las pruebas
  herramientas/     xcframework.sh (núcleo → xcframework), app.sh (simulador, pruebas, ipa)
```

## Cómo encaja cada zona (contratos)

`Raiz/RaizIPad.swift` monta una `NavigationSplitView` con las secciones de `Seccion`
(lo que en Windows son las ventanas de la bandeja). Cada zona da **su vista pública con
esta firma exacta** (hoy son huecos que se rellenan):

| Zona (carpeta) | Vista pública |
|---|---|
| `Proyectos/` | `VistaProyectos(almacen:)` — lista de proyectos + el chat |
| `Galeria/`, `Timeline/`, `Tareas/`, `Buscar/`, `Lecciones/` | `VistaGaleria(almacen:)`, `VistaTimeline(almacen:)`, `VistaTareas(almacen:)`, `VistaBuscar(almacen:)`, `VistaLeccionesIPad(almacen:)` |
| `Asistente/` | `VistaAsistenteIPad(almacen:)` |
| `Sincro/` | `VistaSincroIPad(almacen:sincro:)` |
| `Ajustes/` | `VistaAjustesIPad(almacen:sincro:)` |
| `Lienzo/` | `EditorLienzoIPad(almacen:chat:mensaje:foto:)` |
| `Documentos/` | `VisorDocumentoIPad(url:titulo:almacen:chat:mensaje:)`, `EditorNotaIPad(almacen:chat:mensaje:)`, `HojaCalculoIPad(almacen:chat:mensaje:)` |
| `Planos/` | `VisorPlanoIPad(url:titulo:nucleo:)`, `Visor3DIPad(url:titulo:nucleo:)` |
| `Pines/` | `CapaPines(almacen:)` — la capa de pines flotantes encima de todo |

Para abrir cosas se usa **`Navegacion.compartida`** (`Raiz/Navegacion.swift`):
`abrir(.lienzo(...))`, `abrirFichero(url, titulo:, chat:, mensaje:)` (elige visor por la
extensión), `irAlChat(chat, mensaje:)`, `cerrarVisor()`. Los visores van a pantalla
completa (el «pin sin bordes» de Windows) y **todos** llevan arriba
`BarraVisorIPad(tipo:icono:titulo:alFijar:) { botones propios }` (`Raiz/BarraVisorIPad.swift`),
con el modo noche común `ModoNoche.compartido`.

Lo que una zona tiene que poner en marcha al arrancar se apunta desde su propio fichero con
`Arranque.apuntar { entorno in ... }`, sin tocar `Raiz/`. El `Almacen` avisa por
`GanchosAlmacen.alEnviar` (mensajes nuevos, para el asistente) y `GanchosAlmacen.alEntrarPdf`.
Un enlace `pixpinmax://` llega a `AbrirDesdeFuera.alEnlace`.

## Reglas

1. **El formato y la sincronización son los de Windows y Android.** Todo lo que se guarda
   pasa por el núcleo (`Nucleo`, `EditorLienzo`, …). Nada de formatos propios del iPad.
2. **Mismo aspecto que Windows**: plano, los colores de `Paleta`, los mismos textos en
   español, las mismas herramientas en el mismo orden.
3. **Solo iPadOS 18** y APIs de iOS: nada de AppKit (`NS*`), `Host`, `NSWorkspace`,
   ScreenCaptureKit, Carbon… Lo de iPadOS 26 solo con `if #available(iOS 26, *)` **y** dentro
   de `#if compiler(>=6.2)` (la CI compila con Xcode 16.4, SDK de iOS 18).
4. Swift 5 (modo de lenguaje `.v5`), `@MainActor` para lo de interfaz, el disco fuera del hilo
   principal (`Task.detached`), como en el Mac.
5. Cada zona escribe **solo en su carpeta**. Lo común (`Raiz/`, `Almacen.swift`,
   `Paleta.swift`, `project.yml`) se cambia a propósito y avisando.
6. Los ficheros copiados del Mac se dejan tal cual siempre que se pueda (para poder traer
   sus arreglos); si hay que tocarlos para iOS, lo mínimo.

## Lo que el iPad hace distinto (y por qué)

| En Windows/Mac | En el iPad |
|---|---|
| Ventanas de la bandeja | Secciones de la barra lateral; con Stage Manager, varias ventanas |
| Pines siempre encima de todo | **Capa de pines dentro de la app** (arrastrar, pellizcar, opacidad), **Picture in Picture** para vídeo y el pin en vivo; iOS no deja dibujar encima de otras apps |
| Capturar una zona de la pantalla | iOS no deja capturar otras apps: **fotos, cámara, escáner de documentos (VisionKit), Fotos y la extensión de Compartir**, y capturar la propia app; el editor de capturas anota encima igual |
| Atajos globales | Atajos de teclado dentro de la app (teclado del iPad) y App Intents (Siri, Atajos) |
| Sincronizar siempre | **Mientras la app está abierta** (iOS cierra la red al pasar a segundo plano); al volver se sincroniza con quien esté. Un PC o Mac encendido hace de punto de encuentro |
| Ratón | Tacto, **Apple Pencil** (presión, inclinación, doble toque/apretar para cambiar herramienta, la palma no pinta) y trackpad |

## Traer el Mac

El iPad lleva **el mismo código del Mac** (el núcleo de Rust entero y el Swift que no toca
AppKit), así que se sincroniza con los mismos protocolos que Mac, Windows y Android. Para
ponerse al día con el Mac hay una herramienta, a imagen de `portar-de-windows.py` del Mac:

```
git -C ../PIXPIN_PRO_MAC fetch
apple/herramientas/traer-del-mac.py                 # la punta más reciente del Mac
apple/herramientas/traer-del-mac.py ../PIXPIN_PRO_MAC origin/main   # o un commit/rama
apple/herramientas/traer-del-mac.py --sin-enlaces   # sin regenerar los enlaces de Swift
```

- **Base** = el Mac en el commit de `apple/docs/mac-traido.txt`; **ipad** = lo de aquí;
  **nuevo** = el Mac en el commit pedido. Sin commit, la punta más reciente (por fecha) entre
  `origin/main`, `origin/mac-desarrollo` y las `origin/claude/*` (menos
  `claude/vibrant-gates-0lx4dc`); dice cuál eligió. El repo del Mac es `$PIXPIN_MAC` o
  `../PIXPIN_PRO_MAC`.
- **`apple/nucleo/**`, `apple/Cargo.toml`, `apple/rust-toolchain.toml`**: lo que el iPad no
  tocó se toma del Mac; lo que el Mac no tocó se queda; si cambiaron los dos, `git merge-file`
  (los `.rs` pasan antes por `rustfmt` los tres, para que el formato no salga como
  conflicto). Los conflictos quedan marcados `<<<<<<< ipad … ======= … >>>>>>> mac`.
- **`apple/por-adaptar/**`** y `docs/protocolo-pedidos.md` (las pruebas de `pixpin-ia` lo leen con `include_str!`): manda el Mac.
- **`apple/Cargo.lock`**: el del Mac y luego `cargo update --workspace`.
- **`apple/PixPinCore/Sources/**`**: no se fusiona; se regenera al final con UniFFI si no
  quedan conflictos en Rust (si quedan, dice el comando).
- **Swift** (`apple/app/Sources/PixPinKit/**`): lo que existe en el iPad con la misma ruta (lo
  copiado sin AppKit) se fusiona a tres bandas igual; lo nuevo o cambiado que el iPad no tiene
  se copia tal cual si no importa nada solo del Mac (AppKit, Carbon, ScreenCaptureKit…), no
  choca con un fichero o un tipo del iPad y no usa tipos del Mac que el iPad no tenga; si no,
  va al informe para portarlo a mano. Un fichero que el Mac creó y el iPad ya tenía suyo
  con la misma ruta también va al informe. Las pruebas y `Sources/PixPin` solo se listan.
- Al acabar: el informe en `apple/target/traer-del-mac.json` (tomado, fusión, conflicto,
  `swift_a_portar` con el porqué…), el commit apuntado en `mac-traido.txt` y la lista de
  conflictos en pantalla.

Después: resolver los conflictos (**donde el Mac y el iPad hicieron lo mismo gana el Mac**, que
es el que sigue a Windows; lo que solo tiene el iPad se conserva sin duplicar módulos ni
funciones exportadas; si una función del iPad desaparece porque el Mac trae la suya, se
cambian las llamadas de Swift), `cd apple && CARGO_INCREMENTAL=0 cargo test -p pixpin-nucleo
-- --test-threads=2`, regenerar los enlaces si hizo falta y portar el Swift del informe
(UIKit/SwiftUI, con los mismos textos).

**Traído el 25-oct-2026**: el Mac `2a49a4c` (rama `claude/pixpin-mac-windows-sync-rrmx84`,
26 commits sobre `e96241c`). Funciones del núcleo que el iPad tiene y el Mac no (para
llevarlas al Mac): `proyectos.rs` (renombrar, archivar e `importar_proyecto`), `aparatos::olvidar_direccion`,
`ia_conectar_con` (`asistente/conexiones.rs`), el ángulo del lienzo
(`cambiar_letra_de_angulo`, `alternar_angulo_recto`), `ajustes_organizar.rs`,
`zona_al_chat.rs`, `magia.rs`, `editor_tabla.rs`, `fusionar_paginas.rs`, `letras_lienzo.rs`,
`hojas/suelto.rs` y `hojas/graficos.rs`, `notas/comentarios_nativos.rs` y `notas/nativo_md.rs`,
`apuntar_tarea_con_imagenes_en` (`tareas.rs`) y la ficha de clase del horario
(`horario/ventana_mac.rs`: `FichaDeClase`).
Luego, el Mac `27b1bcd` (2 commits sobre `2a49a4c`): el crate `pixpin-motor-audio` (entra
solo por `nucleo/*`; nadie lo usa aun desde Swift), la nota rica revisada, el vuelo a una
marca que aterriza, la figura que reabre el chat y la foto pegada numerada; sin conflictos
en Rust. Esas funciones del iPad van al Mac en `dffbeef` (rama `claude/vibrant-gates-0lx4dc`),
que aun no esta en `27b1bcd`.
Luego, el Mac `383811e` (5 commits sobre `27b1bcd`): la ficha del chat al estilo Telegram
(proyectos en su primera pestana, las fotos de las notas ricas en «Fotos», lo mas nuevo
primero, por meses, la previa dentro y el teclado; en el iPad, `Chat/InfoChat.swift`),
grabar por el motor de audio de Windows (`Voz/MezclaVoz.swift`, tal cual del Mac:
`GrabadoraVoz` y `MicrofonoSuelto`, y por el «Grabar clase») y los crates al dia hasta
Windows `cad5e20` (`mezcla_audio.rs`, `motor_video.rs`, `pixpin-proyecto/clase.rs`,
`pixpin-ui/conversacion.rs`). Conflictos en Rust: `lib.rs` (los modulos de los dos) y la
prueba del orden de la ficha (la del Mac). El motor de video no se usa en el iPad (no deja
grabar la pantalla de otras apps). Ninguna funcion nueva del nucleo hecha aqui.
Luego, el Mac `62fe093` (6 commits sobre `383811e`): al sincronizar se juntan marcadores,
paginas de carpetas y figuras sin perder nada (reglas 7.1-7.3 de Windows, `pixpin-sincro`
`fusion.rs`/`disco.rs`/`protocolo.rs` tal cual del Mac, con su prueba contra el movil
`pixpin-proyecto/tests/sincronizar_con_el_movil.rs`, 40 pruebas que pasan; `pixpin-sincro` y
`pixpin-proyecto` quedan identicos al Mac), los chats compartidos como conversacion
(`conversacion.rs`; en el iPad `Chat/ConversacionCompartida.swift`, el lado de cada burbuja
en `Chat/VistaChat.swift`, la chapa en la lista y la cabecera, y el «+» que pregunta normal o
compartido y abre «Chats compartidos» en Aparatos), grabar clase en dos modos y «Estudiar la
clase» (`estudiar_clase.rs`; en el iPad `VozVentanas/EstudiarClase.swift` a pantalla completa,
y en `GrabarClase.swift` el selector con «Pantalla y sonido» apagado, que el iPad no deja), y
la ventana de Figuras que se relee al sincronizar (en el iPad ya lo hacia por
`.pixpinTrajoCambios`). `VisorClase` («Ver con las capturas») desaparece, como en el Mac.
Conflictos: `lib.rs` (los modulos de los dos) y tres de Swift (`MenuMensaje`, `Compartidos`,
`VistasVoz`). Ninguna funcion nueva del nucleo hecha aqui.
Luego, el Mac `41ee5eb` (1 commit sobre `62fe093`, Windows hasta e78c1a3): el OCR sigue con
Vision (`pixpin-indra` y `pixpin-ocr` solo de referencia en `por-traducir/`), la seccion
`[ocr]` de `ajustes.toml` se conserva (`pixpin-store`, sin fila en Ajustes, como el Mac) y
`pixpin-mcp` la lee solo en Windows. En el iPad, la hoja de «Sacar texto»
(`Captura/TextoSacado.swift`, la que abren el pin y el editor de captura) dice «Leyendo…»
hasta que llega el texto y una lectura que acaba tarde (hoja cerrada) no pisa nada. Sin
conflictos; 575+ pruebas del nucleo pasan.
