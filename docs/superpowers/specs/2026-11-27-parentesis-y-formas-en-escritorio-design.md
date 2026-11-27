# Paréntesis grandes y reconocimiento de formas en el escritorio

## Por qué

En los apuntes de Álgebra las matrices llevan paréntesis altos dibujados a mano, que quedan
torcidos o asimétricos. En la tablet, dejar el lápiz quieto al final de un trazo ya convierte
cuadrados, círculos, flechas, etc. en formas prolijas, pero las curvas abiertas se dejan como
están. Además, el usuario escribe mucho en la PC con una Wacom, y ahí no hay reconocimiento de
formas.

## Objetivos

1. **Paréntesis:** un trazo abierto con una sola panza hacia un lado, como `(` o `)`, se
   convierte en un arco simétrico y suave al dejar el lápiz quieto.
2. **Escritorio:** el mismo gesto (lápiz quieto un momento, sin levantarlo) reconoce en la PC
   todas las formas que reconoce Android, incluidos los paréntesis.
3. **Un solo reconocedor:** Android y escritorio usan el mismo código, así las formas salen
   iguales en los dos.

## No objetivos

- Llaves `{`, integrales, eses, curvas de funciones u otras curvas: quedan para después.
- Reconocer sin el gesto (al levantar el lápiz): nunca se toca la escritura sin pedirlo.
- Cambiar el aspecto del resto de las formas ni los umbrales que ya existen.

## Anti-objetivos (sería un fracaso aunque funcione)

- Que una línea recta apenas torcida se convierta en paréntesis, o un paréntesis en línea.
- Que la letra común (una `c`, una `s`, un `)` chico de una cuenta) cambie sola.
- Que la forma reconocida salga de otro color o grosor que el trazo que la dibujó.
- Que en la PC el gesto deje restos: el trazo a mano y la forma a la vez, o que deshacer
  necesite dos pasos.

## Diseño

### Paréntesis (reconocedor compartido)

- Un trazo abierto es candidato a arco si todos sus puntos quedan del mismo lado de la
  cuerda (la recta entre las puntas) y la panza máxima (distancia a la cuerda) está entre el 6 %
  y el 35 % del largo de la cuerda. Menos panza es una línea; más es una `U` o una `C`, que
  se siguen dejando como están (el test `leaves_open_curves_alone` sigue valiendo).
- Este chequeo va **antes** del de línea recta: hoy un paréntesis poco curvo cumple el criterio
  de línea (cuerda ≥ 96 % del largo) y saldría como línea.
- Un trazo con una esquina marcada no es un arco: un `<`, un `⟨` o una `V` abierta siguen
  saliendo como polilínea o flecha, como hoy. Solo una curva sin esquinas puede ser paréntesis.
- El resultado es una curva de Bézier cuadrática simétrica: va de una punta a la otra con el
  punto de control sobre la perpendicular que pasa por el medio de la cuerda, con la panza del
  lado en que se dibujó. Se acepta si se parece al trazo (mismo error medio máximo que las
  demás formas, 5 % del tamaño).
- Si la cuerda está a menos de 6° de la vertical (o de la horizontal), se endereza, igual que
  hoy con las líneas y los rectángulos.
- Sigue valiendo el tamaño mínimo que ya existe: un paréntesis chico de una cuenta no se toca.
- El paréntesis queda como una forma más: se puede seleccionar, mover y deshacer igual que un
  rectángulo reconocido.

### Reconocedor en el motor

- El módulo `shape_recognition` y el seguimiento del trazo (puntos del trazo en curso, "ya se
  convirtió", reemplazar trazo por forma en un solo paso de deshacer) pasan de `rnote-android` a
  `rnote-engine`, para que lo use también el escritorio. Android pasa a llamar a ese código; su
  comportamiento no cambia.
- La forma reconocida toma el color y el grosor del pincel con que se dibujó el trazo, no los
  de la herramienta de formas. En Android ya coinciden porque la app los sincroniza; en el
  escritorio pueden ser distintos. El grosor es el configurado para el estilo de pincel en
  uso, sin medir la presión del trazo.
- El resto del estilo (trazo liso o a mano alzada, relleno) sigue saliendo de la herramienta
  de formas, como hoy en Android, salvo que las formas abiertas (línea, polilínea, flecha,
  paréntesis) nunca se rellenan: un paréntesis relleno sería una media luna.

### Escritorio

- Con el pincel, si el lápiz (o el mouse con el botón apretado) se queda quieto 1 segundo
  dentro de un radio de 6 píxeles de pantalla durante un trazo, el trazo se reemplaza por la
  forma que parece, si parece alguna. Si no parece ninguna, el trazo sigue como si nada.
  Mismos valores que en Android.
- Lo que el lápiz hace después de convertir, hasta levantarse, no dibuja nada.
- Tamaño mínimo: 24 píxeles de pantalla, convertidos según el zoom, como en Android.
- Se puede apagar en Configuración con un interruptor "Reconocer formas" y una línea que
  explica el gesto. Viene prendido, como en Android.
- Los textos nuevos se traducen a los idiomas del fork (es, zh_Hans, zh_Hant, ja, ko, ru).
- `docs/fork-desktop.md` explica el gesto.

## Restricciones

- Código nuevo con el estilo de Rust de Google; no se reformatean archivos de upstream.
- Archivos de 200 a 400 líneas, 800 como máximo.
- Verificación en verde antes de cada commit (tests de Rust, clippy con `-D warnings` en la PC y
  para aarch64, Gradle lint y tests, `msgfmt -c`).

## Lo que queda a criterio de quien lo implemente

- Cómo se mide "todos los puntos del mismo lado" con ruido de la mano (tolerancia chica).
- Dónde exactamente vive el temporizador del gesto en el lienzo del escritorio.
- El nombre de la clave de configuración del escritorio.

## Decisiones mías (no las pidió el usuario)

- Bézier cuadrática en vez de arco de círculo: el motor ya dibuja Béziers y no tiene arcos.
- Los umbrales de panza (6 % y 35 %) y el enderezado a 6°.
- Interruptor en Configuración, prendido por defecto.
- La forma toma el color y el grosor configurado del pincel; relleno y trazo a mano alzada
  siguen saliendo de la herramienta de formas, sin rellenar nunca las formas abiertas.
- Las esquinas mandan: un trazo con esquina sigue siendo polilínea o flecha, nunca paréntesis.
