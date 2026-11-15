# Plan para Codex: revisar Orfeón y proponer su integración con Tolhuin Chord

Este documento es el encargo para un agente de código (Codex u otro). Pide **revisar y
proponer**, con evidencia; implementar sólo lo que figura como "arreglo chico" en la fase 1.
Todo lo demás vuelve como propuesta para que el autor decida.

## Contexto

| Repositorio | Qué es |
|---|---|
| `estuariolabs/orfeon` | Coro de cantantes por modelado físico (Rust). App independiente (Windows, macOS, Linux) y plugin VST3/CLAP con la misma interfaz (egui). Motor de voz: CHIMERA (MIT), dependencia sin modificar. |
| `estuariolabs/tolhuin-chord` | Sintetizador de acordes diatónicos sobre ESP32-S3 (C++/Arduino). Botones de grado + joystick que "colorea" el acorde. Sale por MIDI USB y BLE. Rama `claude/zealous-albattani-3me45m`: timbre VOCAL propio con texto a sílabas en español (`vocal_text.h/.cpp`). |
| `jamesondh/chimera-vocal-synth` | Motor de voz. Ya tiene consonantes, planificador de gestos, sílabas, canto con letra (inglés, coreano) y entrada IPA. Sólo lectura. |

Lo que reporta el autor (macOS, Orfeón 0.2.0):

1. Anda bien y no se traba en una Mac (sí en una notebook Acer más lenta).
2. **Faltan consonantes** y **texto por cantante** (que cada cantante cante palabras).
3. **Con Tolhuin Chord, mover el joystick deja acordes sonando** y no hay cómo pararlos salvo
   cerrar el programa. El botón de pánico existe pero está escondido en la pestaña Ajustes.
   No está claro si la interfaz se congela o sólo quedan notas colgadas.

## Reglas para el agente

- Leer `AGENTS.md`/`CLAUDE.md` de cada repo y respetarlos (en Tolhuin: reglas duras de
  armonía, `dsp.cpp` sin Arduino, comentarios en español).
- No modificar CHIMERA. Si hace falta algo de su lado, proponer el cambio para el autor de
  CHIMERA.
- No cambiar el sonido de Orfeón sin decirlo: hay renders de referencia (ver "Verificación").
- No flashear la placa ni publicar versiones.
- Cada afirmación sobre el código, con archivo y línea. Separar **hecho**, **hipótesis** y
  **propuesta**.
- Trabajo en ramas, un PR en borrador por fase.

## Fase 1 — Diagnóstico de las notas colgadas (con arreglos chicos)

**Hechos ya verificados:**

- Tolhuin manda sólo **diferencias** entre acordes: `midiOut()` en
  `tolhuin_chord_v2.ino` (~línea 104) calcula notas a apagar y a prender y llama a
  `midiSendChord()` (`midi.cpp` ~287).
- Por USB manda un paquete por nota; por **BLE**, todo el cambio de acorde en **una sola
  notificación** con running status (note-off = note-on con velocidad 0). El valor de retorno de
  `notify()` no se revisa. MTU pedido: 128 (`midi.cpp` ~120).
- Tolhuin no manda nunca "all notes off" (CC 123) ni un estado completo del acorde.
- Orfeón (app): `crates/orfeon-app/src/main.rs`, `midi_connect()` traduce 0x90/0x80, pitch
  bend y CC 120/123. id de nota = canal·128 + nota. Las notas van por un canal al hilo de audio
  (`producer()`).
- Orfeón (núcleo): `crates/orfeon-core/src/choir.rs`, `note_on_choir()` / `note_off()`, tope de
  voces con robo de la más vieja.

**Hipótesis a confirmar o descartar:**

1. Se pierde una notificación BLE (ráfagas al mover el joystick) → Orfeón no recibe los
   note-off → notas colgadas para siempre, porque el siguiente diff ya no las incluye.
2. Notas repetidas o desordenadas en el canal USB/BLE que Orfeón interpreta mal (por ejemplo,
   note-on de una nota que ya suena, en modo Coro).
3. La app de Orfeón se congela (interfaz o hilo de audio) en vez de dejar notas colgadas.

**Cómo diagnosticar:**

- Agregar a Orfeón un **monitor MIDI** opcional (registro de lo recibido, con tiempo) y un
  contador de "notas sonando según Orfeón" vs. "notas que el emisor cree que suenan".
- Reproducir con la Tolhuin por USB y por BLE por separado, moviendo el joystick rápido.
- Escribir un test de `orfeon-core` que reciba secuencias de diffs con un paquete perdido y
  muestre el estado final.

**Arreglos chicos que sí puede hacer en esta fase:**

- Orfeón: **botón de pánico visible siempre** (barra de arriba) y **tecla Esc** = pánico.
- Orfeón: tratar CC 120/123 y además **"note-on de una nota ya sostenida" como re-ataque de
  la misma voz**, no como voz nueva.
- Tolhuin: al soltar todo (o cada N segundos sin cambios) mandar **CC 123** en el canal 1.

**Entregable:** informe con la causa (o causas) demostrada, los arreglos chicos en un PR, y la
propuesta de protocolo robusto de la fase 3.

## Fase 2 — Consonantes y texto por cantante (propuesta + prototipo)

**Situación:**

- Orfeón hoy sólo manda **posturas de vocal** al motor (`crates/orfeon-core/src/vowel.rs`,
  `Slot::render_chunk` en `choir.rs`): no usa el planificador de CHIMERA.
- CHIMERA ya tiene `phonetics::sing::plan_note_on` / `plan_note_off` (sílabas → gestos con
  consonantes, `crates/chimera/src/phonetics/sing.rs`), entrada **IPA**
  (`phonetics/ipa.rs`) y un diseño para sumar idiomas (`docs/language-front-ends.md`).
- Tolhuin ya tiene un silabeador del español por reglas, probado (`vocal_text.cpp`, tests en
  `test/test_vocal_text.cpp`): texto → fonemas → sílabas, seseo y yeísmo, diptongos e hiatos.

**Qué proponer:**

1. Un **frente de idioma español** para Orfeón: portar las reglas de `vocal_text.cpp` a Rust
   y producir `Syllable` de CHIMERA en IPA. Comparar con escribir un frente dentro de CHIMERA
   (para proponérselo a su autor).
2. **Una línea de texto por cantante** (o por cuerda: sopranos, contraltos, tenores, bajos,
   que es lo habitual en un coro), con su propio cursor de sílaba: cada note-on de ese cantante
   avanza a su sílaba siguiente. Definir qué pasa en Coro / Tutti / Solo.
3. Cómo convive con la carta de vocales: sin letra, como hoy; con letra, la vocal sale de la
   sílaba.
4. Costo: `plan_note_on` reserva memoria (lo dice `docs/rust-engine.md` de CHIMERA). Proponer
   cómo planificar fuera del hilo de audio o con buffers fijos.
5. Interfaz: dónde se escribe la letra (por cuerda y por cantante), cómo se ve la sílaba
   actual, cómo se reinicia. Guardado en el proyecto del plugin.

**Entregable:** documento de diseño + prototipo mínimo (un cantante, una frase en español con
consonantes) con un WAV de prueba y medición de CPU contra la versión sin letra.

## Fase 3 — Integración Tolhuin ↔ Orfeón (propuesta)

Opciones a evaluar, con pros, contras y costo:

1. **MIDI estándar, más robusto.** Tolhuin manda acordes como hoy; agregar:
   estado completo del acorde periódico o al cambiar (por ejemplo, CC 123 + notas, o un SysEx
   "acorde = {notas}") para que el receptor se resincronice si perdió algo.
2. **Controles de expresión.** Mapear el joystick (zonas de color) y botones de Tolhuin a
   parámetros de Orfeón por CC: vocal (X/Y de la carta), estilo, movimiento, volumen.
   Proponer una tabla de CC y el modo "aprender MIDI" en Orfeón.
3. **Letra desde la Tolhuin.** La Tolhuin ya avanza sílabas por ataque (timbre VOCAL): ¿manda
   la sílaba (SysEx de texto) para que Orfeón la cante, o Orfeón guarda la letra y la Tolhuin
   sólo marca "siguiente sílaba"?
4. **Perfil "Tolhuin" en Orfeón**: un programa y un mapeo listos para usar al conectarla.

**Entregable:** documento con la especificación del protocolo elegido (mensajes, canales,
qué pasa si se pierde un mensaje) y un plan por etapas con estimación.

## Fase 4 — Vocales desde el micrófono (propuesta + prototipo en la app)

Idea del autor (como "The Mouth" para Reaktor): el coro toma la **forma de la boca** de una voz
que entra por micrófono, mientras las notas vienen de la Tolhuin o del teclado.

**No es un vocoder:** se mide la *postura* (vocal) y cada cantante la aplica a su propio tracto,
así siguen siendo cuerpos distintos.

1. Cada ~10 ms, estimar F1/F2 de la entrada (LPC; CHIMERA trae `analysis/lpc.rs` y
   `analysis/pitch.rs`: verificar que sirvan en tiempo real y no reserven memoria).
2. Convertir F1/F2 en un punto de la carta de vocales (`vowel_x`, `vowel_y`). Propuesta:
   tabla precalculada con el propio modelo (`airway/frequency.rs` da los formantes de cada
   punto de la carta para cada cuerpo) y búsqueda del más cercano, normalizando por el
   hablante.
3. Volumen de la entrada → presión de pulmones; relación ruido/armónicos → aire.
4. Sólo en la app (entrada de micrófono con cpal). El plugin con entrada lateral queda para
   después.

**Riesgos a resolver:** acople (micrófono que escucha los parlantes: pedir auriculares,
compuerta de ruido), latencia del análisis (~20–30 ms), F1 poco confiable en voces agudas
(suavizar y descartar lecturas raras).

**Criterio de aceptación:** con un acorde sostenido, decir "aaa-eee-iii-ooo-uuu" al micrófono y
que las cinco vocales se reconozcan a oído en el coro, sin saltos; la interfaz muestra el
punto medido y la demora.

## Orden de trabajo acordado

1. Robustez MIDI y pánico visible (fase 1).
2. Preset SATB, conducción de voces por costo (registro, distancia a la última nota del
   cantante, sin cruces), tonos comunes por diffs y fraseo escalonado. Detrás de un
   interruptor: con él apagado, los 36 programas actuales suenan idéntico byte a byte.
3. Vocales desde el micrófono (fase 4), en la app.
4. Joystick expresivo (CC de la Tolhuin → vocal, vibrato, brillo), reutilizando el mismo
   punto de la carta.
5. Canal MIDI por voz (S, A, T, B) en Tolhuin y Orfeón.
6. Letra en español (fase 2).
7. Micrófono en el plugin (entrada lateral) y consonantes desde el micrófono.

Compatibilidad en todas las fases: no cambiar IDs de parámetros ni el orden de los enums (el
plugin guarda los enums por índice); presets nuevos sólo al final de `PRESETS` (el plugin
guarda el programa por índice).

## Verificación que el agente tiene que correr

- Orfeón: `cargo test --release` (16 tests) y, si toca el núcleo, comparar renders con la
  versión actual: `cargo run --release -p orfeon-cli -- render a.wav --preset 1` antes y
  después, `cmp` byte a byte (sin letra cargada no debería cambiar nada).
- Orfeón: `cargo build --release -p orfeon-app -p orfeon-plugin`.
- Tolhuin: `test/run_tests.ps1` o los `g++` equivalentes de `test/` (armonía, DSP, VOCAL).
- GitHub Actions de Orfeón en verde (`.github/workflows/compilar.yml`).

## Formato del informe final

1. Resumen de una página: qué causa las notas colgadas, qué se arregló, qué se propone.
2. Por fase: hechos (con archivo:línea), hipótesis descartadas y por qué, propuestas con costo.
3. Lista de PRs abiertos y qué probar en el equipo real (Mac + Tolhuin por USB y por BLE).
4. Preguntas abiertas para el autor.
