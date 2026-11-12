# Opciones al crear un cuaderno (estilo StarNote)

Fecha: 2026-11-12. Fork: YX14ng/rnote, rama `android/engine-build`.
Diseño visual: canvas de Claude Design https://claude.ai/artifact/PFWV4m2m1e2qjVVaq3qwcK
(artboards "Biblioteca: menú del +", "Cuaderno nuevo" e "Importar PDF"; privado del usuario).

## Intención

En StarNote el usuario arma cada cuaderno de una materia desde una pantalla de creación
(título, etiquetas, portada, formato, color del papel, interlineado, plantilla) y desde un
menú del "+" con nota nueva, nota infinita, importar archivo o imagen, foto y nota rápida.
Usa todas esas opciones. En Rnote para Android hoy "Nuevo" crea un "Sin título" con la hoja
por defecto y todo se cambia después desde el editor. Para dejar StarNote, crear un
cuaderno en Rnote tiene que ser igual de directo.

La portada sirve para **distinguir las materias de un vistazo** en la grilla.

## Objetivos

1. **Menú del "+"** en la biblioteca de Android (tarjeta "Nuevo"): Cuaderno nuevo…, Nota
   rápida, Nota infinita, Importar PDF, Importar imagen, Sacar foto.
2. **Pantalla "Cuaderno nuevo"**: vista previa de portada y hoja; título, carpeta,
   etiquetas, portada (sí/no + color), formato (tamaño y orientación), páginas (hojas,
   continua, infinita), color del papel, interlineado y plantilla.
3. **Portadas de color** en la biblioteca: la tarjeta de un cuaderno con portada muestra
   una tapa del color elegido con su nombre; sin portada, la primera hoja como hoy. Se ven
   igual en la biblioteca de escritorio y viajan con la sincronización.
4. **La última configuración queda guardada por carpeta**: el próximo cuaderno de esa
   carpeta arranca igual; "Nota rápida" la usa sin mostrar la pantalla.
5. **Importar PDF** como cuaderno nuevo con cada página de fondo para escribir encima;
   **importar imagen** y **sacar foto** como cuaderno nuevo con la imagen en la primera
   hoja.

## No objetivos

- Plantillas de estudio o planificación (Cornell, listas, planners): el usuario usa solo
  renglones o cuadrícula (decisión del usuario).
- Portadas con imagen o diseños; solo color y nombre.
- Pantalla de creación en el Rnote de escritorio (allí solo se ven las portadas).
- Importar PPT, DOC, EPUB y demás formatos de StarNote; solo PDF e imágenes.
- Idioma del documento: llega con la búsqueda manuscrita, que es la que lo usa (decisión
  del usuario).

## Anti-objetivos

- Que un `.rnote` creado así no abra en el Rnote de escritorio del fork: todo lo de la hoja
  (tamaño, páginas, color, patrón, interlineado) va en lo que el formato ya guarda.
- Perder la portada o las etiquetas al mover, renombrar o sincronizar el cuaderno.
- Abrir ventanas o diálogos en la PC durante el desarrollo (regla del usuario).

## Decisiones del usuario

- Usa todas las opciones de la pantalla de StarNote, salvo las plantillas que no sean
  renglones o cuadrícula.
- El diseño de menús y pantallas pasa por Claude Design (el canvas de arriba).
- Importar PDF tiene que servir para todo tamaño: guías de unas 20 páginas y apuntes o
  libros enteros de cientos.

## Diseño (lo decidido en el canvas; mis decisiones marcadas)

**Plantillas** (mi decisión): cuatro, En blanco, Renglones, Cuadrícula, Puntos; el grosor
va aparte en **Interlineado** (4, 5, 6, 8, 10 mm). Mapean a `PatternStyle` None, Lines,
Grid, Dots y a `pattern_size` en unidades del documento (mm a 96 ppp).

**Color del papel** (mi decisión): los mismos de `BackgroundColor` del diálogo del editor
(`DocumentFormat.kt:80`): Blanco, Crema, Gris claro #EEEEEE, Oscuro #282A36 (el "A4
Dracula" que el usuario ya usa) y Negro, así un cuaderno nuevo coincide con lo que después
muestra el editor. El color del patrón es `BackgroundColor.patternColor`. (El canvas dibuja
un gris #E6E7EA y no tiene Negro: manda el código.)

**Páginas**: Hojas = `Layout::FixedSize`, Continua = `ContinuousVertical`, Infinita =
`Infinite`. "Nota infinita" del menú = Infinita con el resto de la última configuración.

**Formato**: los tamaños de `PageSize` que ya ofrece el diálogo del editor, más vertical u
horizontal.

**Portada** (mi decisión: vive en la biblioteca, no en el `.rnote`): ocho colores (Azul
#3D6FB6, Verde #3F8F6B, Naranja #C9773A, Rojo #B5524E, Violeta #7A5BB0, Turquesa #2F8C93,
Mostaza #B8922E, Grafito #5E6470). Se guarda en `library.json` como un mapa nuevo
`notebook_covers: path → { color, updated_at, deleted }`, con las mismas reglas que
`notebook_tags`: sigue al cuaderno al mover, renombrar y mandar a la papelera, y se une con
`merge_meta` por registro (gana el `updated_at` más nuevo). Un `library.json` sin el mapa
se lee como sin portadas, así el formato sigue en `version` 1.

Compatibilidad hacia adelante (mi decisión): `Meta` hoy descarta claves que no conoce
(`meta.rs:48`, y `merge_meta` rearma el struct), así que una versión vieja que guarde o
una borraría las portadas. `Meta` gana un `#[serde(flatten)] extra` que conserva claves
desconocidas, y `merge_meta` las une (claves de ambos lados; ante el mismo nombre, la
local). Las versiones anteriores a este cambio no lo tienen: hoy solo sincroniza la tablet,
que se actualiza junta; el escritorio todavía no sincroniza.

**Escritorio** (mi decisión): las tarjetas de `rnote-ui` (`library/card.rs`) muestran la
portada leyendo `library.json`. Llega a la PC cuando exista la sincronización de escritorio
(etapa 5) o la carpeta llegue por otro lado; no depende de esta etapa. El color también se puede
cambiar después desde el menú "⋮" de la tarjeta.

**Última configuración por carpeta** (mi decisión: en el dispositivo, no sincronizada):
preferencias de Android por carpeta (y una para "Sin carpeta"); sin nada guardado, A4
vertical, Hojas, Oscuro, Cuadrícula de 5 mm, sin portada.

**Crear**: `new_notebook_path` como hoy; un cuaderno nuevo ya se guarda apenas abre
(`MainActivity.kt:348`). Lo nuevo es aplicar la configuración con `document_settings.rs`
(`apply_page_size`, `apply_layout`, `apply_background`) antes de ese primer guardado, y
guardar la portada y las etiquetas en `library.json`.

**Importar PDF**: selector del sistema → paso "Importar PDF" con **Páginas** (todas, o
desde–hasta), **Tipo** (Vector, por defecto, o Imagen) y contraseña si el PDF la pide → cuaderno
nuevo con el nombre del PDF en la carpeta actual. Las páginas se importan con
`generate_pdf_pages_from_bytes` (hayro, Rust puro), con `adjust_document` (mi decisión): la
hoja toma el ancho del formato elegido y el alto de la proporción del PDF, en `FixedSize`; un
PDF que mezcla vertical y horizontal queda con un solo tamaño (lo que hace el motor). Las
páginas son imágenes debajo de la tinta: la goma no las toca (`trash_comp.rs:108` ignora
imágenes), el lazo sí puede moverlas; no se bloquean (mi decisión, bloquearlas es trabajo
nuevo en el motor).

Vector por defecto (mi decisión): cada página queda como SVG (`hayro_svg`), que se comprime
dentro del `.rnote`; como Imagen cada página es un PNG a 1,8×, que no se comprime y hace
pesado un libro entero. Imagen queda para el PDF que en la tablet se dibuje lento.

Para que un PDF de cientos de páginas ande (consecuencia de "todo tamaño"):
- La importación corre fuera del hilo de la interfaz, con progreso por página y Cancelar;
  cancelar no deja un cuaderno a medias.
- **Subida a Drive de archivos grandes** (mi decisión, requisito de esta etapa): hoy
  `rnote-sync` sube con `uploadType=multipart` y `media` (`drive.rs:331`, `:338`), que Drive
  documenta para archivos de hasta 5 MB. Los archivos de más de 5 MB pasan a subida reanudable
  (`uploadType=resumable`), con el mismo comportamiento ante errores y sin conexión que hoy.
- La prueba en la tablet incluye un PDF real de varios cientos de páginas: importarlo, abrirlo,
  desplazarse y escribir sin trabarse, y que suba a Drive.

**Imagen / foto**: imagen insertada en la
primera hoja ajustada al ancho; la foto usa la cámara del sistema (`ACTION_IMAGE_CAPTURE`
con un archivo propio vía un `FileProvider` nuevo en el manifiesto, sin pedir el permiso
de cámara).

## Lo que queda para el que lo construye

- Cómo se dibuja la pantalla en Android (vistas a mano con `Graphite`, como el resto).
- Nombre del cuaderno vacío: "Sin título" con sufijo libre, como hoy.
