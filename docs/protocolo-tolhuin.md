# Fase 3 · Integración Tolhuin ↔ Orfeón

## Punto de partida

Tolhuin calcula diferencias respecto al último conjunto local y emite en canal 1 por USB y BLE. La notificación BLE encadena las notas con running status y el firmware pide un MTU de 128. El código de notas tampoco comprueba el retorno de la escritura USB. Orfeón recibe notas y controles básicos. En la app, el filtro de entrada descarta SysEx y el pánico MIDI usa un borrado global. El editor web de Tolhuin ya usa SysEx no comercial `7D`, comandos `01` y respuestas `02`. [Tolhuin, .ino:104–119](https://github.com/estuariolabs/tolhuin-chord/blob/18b9e14/tolhuin_chord_v2.ino#L104-L119) · [midi.cpp:114–124](https://github.com/estuariolabs/tolhuin-chord/blob/18b9e14/midi.cpp#L114-L124) · [midi.cpp:190–239](https://github.com/estuariolabs/tolhuin-chord/blob/18b9e14/midi.cpp#L190-L239) · [Orfeón, main.rs:268–303](https://github.com/estuariolabs/orfeon/blob/db73e7b/crates/orfeon-app/src/main.rs#L268-L303)

## Opciones evaluadas

| Opción | Beneficio | Costo o límite | Estimación inicial |
|---|---|---|---:|
| MIDI estándar con CC 123 al quedar vacío | Funciona con instrumentos y DAW existentes. Recupera silencios si llega el CC. | Un CC perdido o una nota sostenida sin transición a vacío requieren otra recuperación. | Fase 1 |
| CC 123 seguido del acorde completo durante el toque | No requiere SysEx. | Rearticula tonos comunes y reinicia envolventes. Una pérdida parcial deja un estado incompleto. | 1–2 días |
| Diffs MIDI más estado completo por SysEx | Reconcilia sin reatacar notas comunes. Los receptores MIDI genéricos siguen usando los diffs. | Requiere envío y recepción SysEx y pruebas del transporte. Compatibilidad de DAW por validar. | 4–6 días |
| CC de expresión y aprendizaje | Permite tocar timbre y dinámica usando los controles físicos. | Hay que reservar gestos de interfaz y resolver automatización de hosts. | 3–5 días |
| Texto o disparo de sílaba | El texto en Orfeón habilita líneas distintas por cantante. | Enviar texto por SysEx exige fragmentación UTF-8, preparación y estado de edición. Un disparo independiente duplicaría los avances causados por Note On. | 1–2 días para perfil y cursor local, 4–6 adicionales para texto remoto |
| Perfil Tolhuin | Activa puerto, canal, mapa de expresión y reconciliación en un paso. | Requiere un perfil por proyecto y asignación explícita de puerto si hay varios equipos. | 1–2 días |

## Protocolo propuesto

**Activación.** Perfil opcional en Tolhuin y en Orfeón. En la primera entrega, un puerto dedicado y canal 1 en la app. Activación manual y MIDI USB o BLE seleccionado explícitamente. La ruta de plugin se habilita cuando el DAW preserve las tramas y su orden. El monitoreo captura la secuencia y los conjuntos de notas en ambos extremos.

**Interpretación.** Note On y Note Off estándares conservan el ataque inmediato. Mantener las notas remotas activas por puerto y canal. En modo Coro, el receptor compara el estado completo con las notas de esa fuente que procesa y añade sólo faltantes y libera sólo sobrantes. Mantener también el conjunto de notas recibidas independiente de las voces robadas, para evitar reinsertar una nota que ya se recibió y agotó la polifonía. Aplicar las operaciones como un lote antes del próximo bloque, en orden determinista, liberaciones primero. Las notas de pantalla y de otros puertos quedan fuera de la reconciliación. Si Coro está fuera de capacidad, la selección de voces sigue la política del núcleo. Para Tutti y Solo el perfil requiere especificar la nota prioritaria en una revisión futura, pues el conjunto ordenado por número no codifica el orden de pulsación.

**Formato v1.** Bytes MIDI en hex para constantes. Canal codificado de 0 a 15. Las notas van ordenadas ascendentemente y son únicas.

```text
F0 7D 54 01 CH SEQ N [NOTE × N] F7
CH    00 para Tolhuin en canal 1
SEQ   secuencia de trama, 0–127, avanza módulo 128 en cada envío
N     cantidad 0–9, incluidos posibles bajos
NOTE  nota MIDI 0–127, velocidad de recuperación 100
```

`7D 54` queda reservado para pruebas conjuntas y debe validarse para distribución con los responsables de ambos proyectos. El máximo son 17 bytes MIDI y 19 con encabezado y timestamp BLE básicos. Confirmar los bytes efectivos de framing SysEx en USB y BLE antes de dar por válida una trama en el MTU mínimo. Validar versión, canal, longitud, orden y ausencia de duplicados antes de encolar. Un mensaje vacío válido es una foto del acorde vacío. La secuencia detecta duplicados y huecos. Aceptar diferencia modular de 1 a 63 en un flujo activo, descartar 0 y 64–127, y restablecer la referencia de secuencia tras conectar o expirar.

**Cadencia y pérdida.** Enviar estado tras los diffs del cambio con una espera de coalescencia de hasta 30 ms, y repetir el último estado cada 250 ms incluso vacío. En condición estable, perder un estado retrasa la recuperación al siguiente. Perder ocho repeticiones consecutivas activa un plazo de 2 s desde la última trama válida y libera sólo esa fuente. Tras expirar, aceptar una trama íntegra con cualquier secuencia. Flush de la fuente al desconectarse, cambiar de puerto o salir del perfil. Si la API retorna fallo de cola, contabilizarlo y volver a intentar la imagen actual en la siguiente oportunidad. No reenviar diffs atrasados. Procesar del mismo flujo FIFO, verificar en cada plataforma que el orden entre SysEx y notas se conserve. Sin esa garantía, habilitar únicamente el modo compatible o adoptar en una revisión posterior eventos de nota con secuencia común. Si la entrada de audio se queda parada, la recuperación también se demora hasta que vuelva a consumir.

**CC 123 y pánico.** El firmware de fase 1 usa CC 123 al pasar al conjunto vacío. La implementación futura del perfil necesita circunscribir ese CC a la fuente y canal de Tolhuin. Hoy la app y el plugin lo interpretan como pánico global. El botón de pánico de la app deberá suprimir rearmado por fotos periódicas hasta una nueva acción de toque, definida como cambio de conjunto observado o Note On nuevo. Una secuencia de heartbeat sola no rearma. Un estado vacío debe desarmar esa supresión para el siguiente ataque.

## Expresión y letra

| CC en canal 1 | Acción de fábrica | Entrada |
|---|---|---|
| 16 | Carta de vocales X | Eje horizontal |
| 17 | Carta de vocales Y | Eje vertical |
| 18 | Estilo, selección por tramos proporcionales | Botón en capa de expresión |
| 19 | Movimiento del coro, rango del parámetro | Control en capa de expresión |
| 11 | Expresión de salida, 0–127, con rampa | Control en capa de expresión |

La capa de expresión se activa explícitamente para conservar el toque armónico de Tolhuin. Para los ejes, propuesta inicial de 25 Hz y umbral de 2 valores MIDI, con suavizado en Orfeón. «Aprender MIDI» arma un parámetro, acepta el próximo CC del puerto y canal elegidos, muestra la asignación y permite quitarla. Resolver colisiones al asignar y persistir el mapa por proyecto. Excluir los CC de modo de canal y evitar aplicar eje de vocal a cantantes que ya toman el núcleo de la letra.

La primera versión guarda la letra en Orfeón y avanza por ataques musicales. Las adiciones por restauración de un estado sirven para volver a sonar sin avanzar el cursor. Si se pierde un ataque, se detecta la brecha y un reinicio de frase puede volver a alinear la letra. Sincronizar con el cursor VOCAL del firmware exigiría una posición absoluta de frase, porque una orden incremental perdida produce un desfase permanente. Dejar esa extensión para después de validar la letra local y acordar si ambos equipos deben cantar al unísono.

## Entrega por etapas

1. **1–2 días.** Medir en Mac vía USB y BLE por separado, comparar conjuntos y tasa de pérdida. Probar transición rápida bajo HOLD y sin HOLD. Probar con el reloj MIDI activado y desactivado.
2. **3–4 días.** Frame v1 en USB y en app. Inyectar pérdida de diff, estado, fragmento, tramas inválidas, duplicados, reconexión y retorno de secuencia 127→0. Probar un teclado externo junto a Tolhuin. Verificar recuperación sin retrigger de tonos comunes.
3. **1–2 días.** BLE con distintos MTU y ráfagas, comprobación de colas y desconexión. Medir demora de recuperación con y sin errores. Verificar recepción en los DAW objetivo antes de habilitar el perfil allí.
4. **3–5 días.** Perfil, CC y aprendizaje MIDI. Ajustar rangos y suavizado con el autor. Integrar cursores cuando la fase de letra llegue al plugin.
