# Biblioteca, sincronización con Google Drive, barra estilo StarNote y búsqueda manuscrita

Fecha: 2026-11-10. Fork: YX14ng/rnote, rama `android/engine-build`.

## Intención

El usuario toma apuntes de la facultad a mano en una Galaxy Tab S10 Ultra (S Pen) y en la
PC (Linux, Wacom). Hoy usa StarNote en la tablet, cuyo modelo de trabajo quiere en Rnote:
una pantalla de inicio con todos los cuadernos organizados, un editor con barra superior
de dos filas y un botón para volver al inicio, los mismos cuadernos en todos sus
dispositivos y poder encontrar lo que escribió a mano.

El por qué: dejar StarNote (cerrado, con avisos de backup en chino) por Rnote en todos
lados, con los apuntes como archivos `.rnote` abiertos que también abre el Rnote de
escritorio.

Referencias visuales (capturas de StarNote en la tablet, copiadas a
`/tmp/claude-1000/-home-Fujimaru/653dcda7-2e03-4ea2-a4f6-ca0903fb070e/scratchpad/starnote/`,
`editor.jpg` y `home.jpg`; se pierden al reiniciar la PC, por eso se describen abajo).

## Objetivos

1. **Biblioteca** (Android y escritorio): pantalla de inicio con columna izquierda (Todas
   las notas, Sin categoría, Carpetas con `+`, Etiquetas con `+`, Papelera, cada una con su
   cantidad) y una grilla de cuadernos (miniatura de la primera página, nombre, fecha,
   botón "Nuevo"), con orden (fecha de modificación, nombre), vista grilla/lista y
   selección múltiple.
2. **Editor Android con barra estilo StarNote**, dos filas: arriba `<` (vuelve a la
   biblioteca), título y acciones; abajo deshacer/rehacer a la izquierda y las
   herramientas centradas. La barra lateral de herramientas actual desaparece.
3. **Sincronización con Google Drive** (Android y escritorio) de toda la biblioteca, con
   permiso solo sobre lo que crea la app, entre cualquier cantidad de dispositivos.
4. **Búsqueda dentro del cuaderno abierto** (solo Android) que encuentra texto manuscrito
   y tipeado, marca las coincidencias y salta entre ellas.

## No objetivos

- Instaladores de escritorio para Windows y macOS (el código queda portable).
- CI para los releases.
- Búsqueda en la PC (decisión del usuario). Tampoco búsqueda global desde la biblioteca:
  el buscador es dentro del cuaderno abierto (decisión del usuario).
- Sincronizar el índice de búsqueda entre dispositivos: cada Android arma el suyo.
- Reconocer fórmulas matemáticas como tales; se indexa lo que el reconocedor devuelva.
- Cambiar la barra superior del editor de escritorio más allá de agregar "Inicio".
- Sincronización en tiempo real o colaborativa; sincronizar ajustes de la app.
- Otros proveedores de nube.

## Anti-objetivos (sería un fracaso aunque "ande")

- Perder trabajo: ninguna sincronización puede pisar una versión sin dejar copia.
  Ante la duda, duplicar.
- Pedir acceso a todo el Drive del usuario (scope `drive` o `drive.readonly`).
- Credenciales (client secret, tokens, keystores) en el repositorio.
- Romper la apertura de archivos sueltos que ya existe ("Abrir", "Guardar como" por
  el selector del sistema).
- Reformatear código upstream de Rnote; el código nuevo sigue la Google Rust Style Guide.

## Decisiones del usuario

- Multiplataforma incluyendo la PC; la sincronización va **dentro del Rnote de
  escritorio**, con su botón, no como programa aparte.
- La pantalla de inicio también existe en la PC.
- De la columna de StarNote usa todo: todas, sin categoría, carpetas, etiquetas, papelera.
- Búsqueda: tiene que encontrar lo manuscrito. En Android con el reconocedor de escritura
  de Google (ML Kit Digital Ink); en la PC sin búsqueda.
- Las herramientas del editor Android pasan arriba, como en StarNote.
- Conflictos: no se pisa nada, se guardan las dos versiones (propuesto por el asistente,
  el usuario no lo objetó).
- **La sincronización es para cualquiera**, con su propia cuenta de Google, en los
  releases públicos de GitHub (no solo para los dispositivos del usuario).

## Arquitectura

Dos crates nuevos en el workspace, sin dependencias de GTK ni de Android, usados por
`rnote-ui` (escritorio) y `rnote-android` (JNI):

- `crates/rnote-library`: modelo de la biblioteca en disco, metadatos, papelera,
  miniaturas.
- `crates/rnote-sync`: cliente de Drive v3 por REST, algoritmo de sincronización y su
  estado local. El token de acceso lo provee cada plataforma.

Las interfaces (Kotlin en Android, GTK4/libadwaita en escritorio) solo dibujan y llaman a
estos crates. HTTP con `ureq` (bloqueante, rustls) en un hilo de trabajo; JSON con
`serde_json`. Toda la lógica se testea en Rust con un Drive falso en memoria.

## Biblioteca

### En disco

Una carpeta raíz por dispositivo:

- Android: almacenamiento interno de la app (`filesDir/library`). "Abrir" y "Guardar como"
  por el selector del sistema siguen funcionando para archivos fuera de la biblioteca;
  agregar "Importar a la biblioteca" en el menú.
- Escritorio: configurable en los ajustes; `~/Documents/Rnote` por defecto. En la PC del
  usuario se apunta a su carpeta existente `/mnt/Storage/HandNotes` (disco exFAT; el
  Flatpak necesita permiso de lectura y escritura sobre ella). exFAT no distingue
  mayúsculas: dos nombres que solo difieren en mayúsculas se tratan como conflicto de
  nombre (sufijo), y un renombre que solo cambia mayúsculas se hace en dos pasos.
- Archivos que no son `.rnote` en la raíz: se ignoran para la sincronización, igual que
  los ocultos (p. ej. autoguardados `.*.autosave.xopp` de Xournal++). Los `.xopp`
  (Xournal++) aparecen en la biblioteca como tarjetas "Xournal++ · importar": importar
  usa el importador de `.xopp` que ya tiene `rnote-engine`, crea el `.rnote` al lado y no
  toca el original.

Estructura:

```
<raíz>/
  Cuaderno suelto.rnote            ← "Sin categoría"
  Álgebra/                         ← carpeta (un nivel; ver "lo que queda al constructor")
    Parcial 1.rnote
  .rnote-library/
    library.json                   ← metadatos (se sincroniza)
    trash/                         ← papelera (se sincroniza)
```

- **Carpetas = carpetas reales.** Un cuaderno está en una sola carpeta o en la raíz.
- **`library.json`**: definiciones de etiquetas (nombre, color), etiquetas de cada
  cuaderno, entradas de la papelera (ruta original, fecha de borrado). Cada entrada lleva
  `updated_at` y las bajas son lápidas (`deleted: true`), para mezclar sin conflictos.
- **Identidad del cuaderno**: ruta relativa. Las operaciones hechas desde la app (renombrar,
  mover, papelera) actualizan `library.json`. Si el usuario renombra un archivo por fuera
  (gestor de archivos), pierde sus etiquetas: limitación aceptada y documentada.
- **Papelera**: "a la papelera" mueve el archivo a `.rnote-library/trash/` y registra la
  ruta original. Restaurar lo devuelve (con sufijo si el nombre está ocupado); "Borrar
  definitivamente" y "Vaciar papelera" lo eliminan. Sin vaciado automático.
- **Miniaturas**: primera página renderizada con `rnote-engine`, en la caché local (no se
  sincronizan), clave = ruta + mtime + tamaño. Se generan en segundo plano; mientras tanto,
  una tapa lisa.

### Pantalla de inicio

Común a ambas plataformas, siguiendo la de StarNote:

- Columna izquierda: Todas las notas, Sin categoría, Carpetas (lista + `+` para crear),
  Etiquetas (lista + `+`), Papelera, cada una con su cantidad.
- Arriba: título de la sección, botones de vista (grilla/lista), orden, seleccionar y
  sincronizar (con estado: sincronizado, sincronizando, pendiente, error; tocar el error
  muestra el detalle).
- Grilla: tarjeta "Nuevo" (crea "Sin título N.rnote" en la carpeta actual y lo abre), y
  una tarjeta por cuaderno con miniatura, nombre, fecha y un menú (renombrar, mover a
  carpeta, etiquetas, duplicar, a la papelera). En la Papelera: restaurar, borrar
  definitivamente, vaciar.
- Selección múltiple: mover, etiquetar, a la papelera.
- Carpetas y etiquetas: renombrar y borrar desde su menú (borrar una carpeta no vacía pide
  confirmación y manda su contenido a la papelera).

Android: la app abre en la biblioteca. Escritorio: la ventana muestra la biblioteca como
una vista alternativa al área de pestañas (p. ej. un `AdwViewStack` alrededor del área
actual), con un botón "Inicio" en la barra. Abrir un cuaderno desde la biblioteca abre una
pestaña o enfoca la existente; las pestañas abiertas se conservan al ir al inicio.

## Editor Android: barra estilo StarNote

Reemplaza la barra superior y la barra lateral actuales:

- **Fila 1**: `<` (guarda si hace falta y vuelve a la biblioteca), título del cuaderno
  (tocar = renombrar), estado de sincronización, buscar, zoom (%), `⋯` (menú actual por
  secciones) y una flecha que pliega/despliega la fila 2.
- **Fila 2**: deshacer y rehacer a la izquierda; herramientas centradas (lápiz, formas,
  texto, goma, lazo); a la derecha la paleta de 5 colores y el botón de grosor.
- Tocar la herramienta activa abre su panel de opciones debajo (los paneles actuales se
  reutilizan, anclados a la barra en vez de al costado). La rueda del S Pen, el gesto de
  deshacer con dos dedos y los atajos de teclado no cambian.
- Estética: el tema Grafito actual (no copiar colores ni íconos de StarNote).

## Búsqueda (solo Android)

- **Reconocimiento**: ML Kit Digital Ink Recognition (`com.google.mlkit:digital-ink-recognition`),
  modelo de español (`es`), descargado una vez (requiere internet la primera vez; mientras
  no esté, el buscador avisa y solo busca texto tipeado).
- **Agrupado**: el engine (Rust) agrupa los trazos de lápiz en renglones por cercanía
  vertical y en palabras/fragmentos por separación horizontal, y entrega a Kotlin los
  puntos de cada grupo. Los trazos de Rnote no guardan tiempos (solo posición y presión),
  así que los tiempos que pide ML Kit se sintetizan a partir del orden de los trazos y los
  puntos. Kotlin reconoce y devuelve los candidatos.
- **Índice**: por cuaderno, en la caché local: para cada grupo, el hash de sus trazos, su
  rectángulo y los 3 mejores candidatos. Se actualiza en segundo plano, solo los grupos
  que cambiaron, unos segundos después de dejar de escribir y al abrir el cuaderno.
- **Texto tipeado**: se busca directo en los trazos de texto del engine, con posición
  exacta.
- **Búsqueda**: sin distinguir mayúsculas ni acentos, por subcadena, sobre los candidatos
  y el texto tipeado. Resultados marcados sobre la hoja (rectángulo resaltado), navegación
  anterior/siguiente con "2 de 7", y la vista salta a cada coincidencia.
- UI: el ícono de buscar de la fila 1 abre un campo en la barra.

## Sincronización con Google Drive

### Proyecto de Google Cloud (lo crea el usuario; el asistente lo guía)

- Drive API habilitada. Pantalla de consentimiento externa, publicada "En producción"
  (con solo el scope no sensible `drive.file` no requiere verificación y los permisos no
  vencen a los 7 días, como en modo prueba).
- Cliente OAuth Android: paquete `io.github.yx14ng.rnote.viewer` + SHA-1 del certificado
  de firma. Hoy el APK se firma con la clave de debug de esta PC; hace falta un keystore
  propio y estable (fuera del repo, ruta y contraseñas por variables de entorno) para que
  el SHA-1 registrado no cambie entre máquinas. El keystore se crea al **empezar la
  etapa 1** y la app actual (firmada con la clave de debug) se desinstala una vez en ese
  momento, antes de que exista la biblioteca interna: desinstalar más tarde borraría
  `filesDir` con los cuadernos que todavía no se sincronizaron. Los archivos guardados por
  el selector del sistema (p. ej. en Documents) no se pierden al desinstalar.
- Cliente OAuth de escritorio ("Desktop app"): client id y secret entran en tiempo de
  compilación por variables de entorno (`RNOTE_GDRIVE_CLIENT_ID`,
  `RNOTE_GDRIVE_CLIENT_SECRET`, leídas con `option_env!`); sin ellas, el botón de
  sincronizar aparece deshabilitado con un mensaje que lo explica. Nunca en el repo.

Ambos clientes en el mismo proyecto: `drive.file` da acceso a lo que creó cualquier cliente
del proyecto, así escritorio y Android ven los mismos archivos y nada más del Drive.

### Distribución pública

- Un solo proyecto de Google Cloud del usuario sirve a todos los que instalen los
  releases: cada persona autoriza con su cuenta y la app solo ve su carpeta `Rnote`.
- Pantalla de consentimiento "En producción" con nombre de la app, página de inicio (el
  repo en GitHub) y una política de privacidad corta en el repo (qué se accede: solo los
  archivos que crea la app, en el Drive de cada usuario; no hay servidor propio ni se
  recolecta nada). Con solo `drive.file` (no sensible) no hace falta la verificación de
  scopes sensibles; la verificación de marca (nombre y logo en la pantalla) es opcional y
  se pide si el usuario lo quiere.
- Android: el cliente OAuth se identifica por paquete + SHA-1, sin secreto. Todos los APK
  publicados se firman con el keystore estable (sin él no hay actualizaciones posibles:
  hacer copia de seguridad fuera de la PC).
- Escritorio: el client secret de un cliente "Desktop app" no es confidencial para Google
  (va dentro de cualquier binario distribuido, como en rclone), pero igual no se commitea:
  entra por variable de entorno al compilar el release. Builds sin credenciales (forks,
  desarrollo) muestran la sincronización deshabilitada con explicación, en Android y en
  escritorio.
- Los releases se arman con un script local (como los anteriores) que lee keystore y
  credenciales de variables de entorno; CI queda fuera de alcance.
- Escritorio público: bundle Flatpak (`.flatpak`) en el release. El código se mantiene
  portable (token con el crate `keyring`: Secret Service en Linux, Credential Manager en
  Windows, Keychain en macOS), pero los instaladores de Windows y macOS quedan fuera de
  alcance por ahora.
- Cuota de la Drive API: compartida por todos los usuarios del proyecto; los reintentos
  usan backoff exponencial ante 403/429.

### Autorización

- Android: `play-services-auth`, `Identity.getAuthorizationClient` con el scope
  `https://www.googleapis.com/auth/drive.file`; obtiene un access token antes de cada
  sincronización y se lo pasa a Rust por JNI.
- Escritorio: OAuth 2.0 para apps instaladas con redirección a loopback
  (`http://127.0.0.1:<puerto>`) y PKCE; abre el navegador del sistema. El refresh token se
  guarda con el crate `keyring` (Secret Service en Linux, incluido Flatpak).
- "Cerrar sesión de Drive" en los ajustes revoca y borra el token.

### En Drive

Carpeta `Rnote` en la raíz de "Mi unidad", creada por la app (su id queda en el estado de
sincronización; si no está, se busca por nombre entre lo que la app puede ver y, si no
existe, se crea). Su contenido es espejo de la raíz de la biblioteca, incluidos
`.rnote-library/library.json` y `.rnote-library/trash/`.

### Algoritmo

Estado local por dispositivo (fuera de la carpeta sincronizada): por ruta relativa, el id
del archivo en Drive, el `md5Checksum` y `modifiedTime` remotos y el hash local (SHA-256)
de la última sincronización. Una sincronización:

1. Lista lo remoto (recursivo bajo `Rnote`, `trashed = false`) y escanea lo local.
2. Por ruta, comparación de tres vías contra el estado:
   - cambió solo lo local → subir; solo lo remoto → bajar; ninguno → nada;
   - cambiaron los dos (y no son iguales) → **conflicto**: la versión remota se baja como
     `Nombre (conflicto, <dispositivo>, <fecha>).rnote` al lado de la local, y la local se
     sube; las dos quedan en todos los dispositivos;
   - nuevo en un lado → copiar al otro; si la ruta ya existe del otro lado con otro
     contenido, conflicto como arriba;
   - borrado en un lado sin cambios en el otro → borrar en el otro (en Drive, a la
     papelera de Drive, nunca borrado permanente); borrado contra modificado → gana el
     modificado (se restaura).
3. Movimientos y renombres: un archivo que desaparece de una ruta y aparece en otra con el
   mismo hash se trata como movimiento (en Drive, `files.update` de nombre/padres, sin
   volver a subir). Las carpetas se crean y borran según los archivos.
4. `library.json` no entra en conflicto: se mezcla entrada por entrada por `updated_at`
   (lápidas incluidas) y se sube la mezcla.
5. El estado se guarda al final de cada archivo procesado, para que un corte a mitad de
   camino no repita ni pierda trabajo.

Una sola sincronización a la vez (candado). El nombre de dispositivo es `Build.MODEL` en
Android y el hostname en escritorio.

### Cuándo sincroniza

- Al abrir la app y al volver a la biblioteca.
- Unos segundos después de guardar un cuaderno.
- Con el botón.
- Sin conexión: queda "pendiente" y se reintenta en el siguiente disparador.

Si un cuaderno abierto cambió en Drive: si no tiene cambios locales sin guardar, se recarga
con un aviso; si los tiene, al guardarlo se resuelve como conflicto (dos versiones).

## Escritorio: cómo llega al usuario

Hoy usa el Rnote oficial de Flathub (`com.github.flxzt.rnote`, 0.14.2) con un preset A4
oscuro configurado por gsettings. El manifiesto `build-aux/com.github.flxzt.rnote.Devel.json`
no sirve tal cual (perfil de desarrollo, logging debug, sin `--share=network`, que OAuth y
Drive necesitan). Se agrega un manifiesto propio de release, con app id propio del fork y
`--share=network`, que se instala al lado del oficial sin pisarlo. El preset A4 oscuro se
copia al esquema del fork. El asistente compila e instala; las variables de Drive se pasan
al build. ~~Los archivos del fork siguen abriéndose en el 0.14.2 oficial (sin cambios de
formato desde esa versión).~~ **Corrección (2026-11-10, revisión final de la etapa 2):**
falso. El fork guarda el formato 0.15 (upstream f3f7efd6, posterior a v0.14.2), y el 0.14.2
lo lee sin error pero manda imágenes, PDF, textos y formas al origen. Un cuaderno guardado
por el fork o por Android no se abre en el 0.14.2.

Si el Rnote oficial u otro programa edita archivos de la biblioteca, la sincronización lo
ve igual (escanea el disco, no depende de eventos de guardado).

## Verificación

- Rust: tests de `rnote-library` (estructura, papelera, metadatos y su mezcla), de
  `rnote-sync` contra un Drive falso en memoria (todos los casos del algoritmo, cortes a
  mitad de camino, dos y tres dispositivos simulados) y del agrupado en renglones/palabras.
  clippy limpio en host y aarch64.
- Android: lint sin avisos nuevos; prueba en la tablet (biblioteca, barra, búsqueda) por
  adb, respetando que no se lanza la app si hay otra cosa en primer plano.
- Escritorio: prueba manual de la biblioteca y la sincronización.
- De punta a punta, con el proyecto de Google Cloud real: crear en la tablet, ver en la
  PC, editar en ambos sin sincronizar y comprobar la copia de conflicto.

## Orden propuesto

1. Keystore estable + desinstalación única en la tablet; `rnote-library` + biblioteca en
   Android + barra estilo StarNote.
2. Biblioteca en escritorio + build Devel instalado.
3. `rnote-sync` con el Drive falso.
4. Proyecto de Google Cloud (usuario) + keystore + sincronización en Android.
5. Sincronización en escritorio.
6. Búsqueda manuscrita en Android.

Cada etapa cierra con verificación, commits por tipo y push, y se prueba en la tablet o la
PC antes de pasar a la siguiente.

## Decisiones del asistente (revisables)

- Carpetas reales y metadatos en `.rnote-library/library.json` dentro de la carpeta
  sincronizada; papelera como carpeta dentro de ella.
- Identidad por ruta; renombrar por fuera de la app pierde las etiquetas.
- Biblioteca de la PC del usuario en `/mnt/Storage/HandNotes`; los `.xopp` se muestran para
  importarlos a `.rnote` y no se sincronizan.
- Biblioteca de Android en el almacenamiento interno de la app.
- Conflictos: dos versiones, la remota con sufijo "(conflicto, dispositivo, fecha)".
- Borrados en Drive van a la papelera de Drive, nunca permanentes.
- Índice de búsqueda local, no sincronizado; 3 candidatos por grupo.
- En la barra Android: paleta y grosor en la fila 2, a la derecha.
- Escritorio: biblioteca como vista alternativa al área de pestañas; manifiesto Flatpak
  propio de release con red, al lado del oficial.
- Keystore y desinstalación única al empezar la etapa 1, no en la 4.
- `ureq` + rustls para HTTP; `keyring` para el token en escritorio.
- Releases con script local, Flatpak como bundle de escritorio; Windows/macOS fuera de alcance.

## Lo que queda al constructor

- Anidamiento de carpetas: StarNote muestra un nivel; soportar un nivel es suficiente,
  más niveles si no complica.
- Diseño exacto de tarjetas, íconos y espaciados, dentro del tema Grafito (Android) y de
  libadwaita (escritorio).
- Umbrales del agrupado en renglones/palabras y del retardo de indexado.
- Detalles de la API de Drive (paginación, reintentos con backoff, subida simple vs.
  reanudable según tamaño).
