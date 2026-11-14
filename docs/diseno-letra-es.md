# Fase 2 · Consonantes y letra por cantante

## Hechos

- `Slot::render_chunk` de Orfeón actualiza fuente, postura vocal y puerta de cada voz por tramo. La postura sale de la carta y de los desplazamientos del cantante. [Orfeón, choir.rs:300–365](https://github.com/estuariolabs/orfeon/blob/db73e7b/crates/orfeon-core/src/choir.rs#L300-L365)
- Tolhuin convierte texto UTF-8 a fonemas y separa ataque, vocal y coda con reglas de español. La ruta se cubre en el test del timbre VOCAL. [Tolhuin, vocal_text.cpp:58–201](https://github.com/estuariolabs/tolhuin-chord/blob/18b9e14/vocal_text.cpp#L58-L201) · [test_vocal_text.cpp](https://github.com/estuariolabs/tolhuin-chord/blob/18b9e14/test/test_vocal_text.cpp)
- En el CHIMERA fijado por Orfeón, `plan_note_on` deja una vocal sostenida con dos etiquetas y `plan_note_off` genera deslizamiento y coda. La planificación reserva memoria. La programación y liberación de gestos ya preparados tienen capacidad acotada. [CHIMERA, sing.rs:208–262](https://github.com/jamesondh/chimera-vocal-synth/blob/edd2676/crates/chimera/src/phonetics/sing.rs#L208-L262) · [rust-engine.md:52–78](https://github.com/jamesondh/chimera-vocal-synth/blob/edd2676/docs/rust-engine.md#L52-L78)

## Propuesta de idioma

Portar a Rust la normalización y las reglas de Tolhuin y convertir la salida a `Syllable`, conservando el texto mostrado, límites de palabra y de frase. Empezar con seseo y yeísmo. Mantener las pruebas existentes como casos comparables, sumar vocales tónicas débiles, grupos consonánticos, puntuación y las variantes rioplatenses que el autor elija. Verificar cada símbolo contra el inventario y los núcleos calibrados de CHIMERA. Registrar aproximaciones, sobre todo para /ʃ~ʒ/, vibrantes, diptongos y oclusivas con poca aspiración. Ofrecer corrección IPA por sílaba.

Hacer el frente en Orfeón permite validar reglas y variantes sin cambiar la dependencia. Un frente dentro de CHIMERA lo haría reutilizable y permitiría compartir léxico, inventario y pruebas. Requeriría coordinar API y licencia con su autor, y agregar selección explícita de español para evitar que el enrutador de escritura latina elija inglés. [CHIMERA, language-front-ends.md:1–32](https://github.com/jamesondh/chimera-vocal-synth/blob/edd2676/docs/language-front-ends.md#L1-L32)

## Reparto e interfaz

- **Coro**. Asignar la voz con la lógica actual y luego consumir la próxima sílaba de ese cantante. Una letra de cuerda se comparte como texto, con un cursor independiente por cantante. Un reataque del mismo ID conserva la sílaba. Si una voz es robada por otra nota, libera los gestos anteriores y avanza el cursor.
- **Tutti**. Cada nota nueva que pasa al tope hace avanzar a los cantantes activos. Al volver a una nota que todavía está pulsada, recuperar la sílaba que se le asignó sin avanzar. **Solo** usa lo mismo con el solista. Guardar asociación de nota a sílaba en la pila acotada. [Orfeón, choir.rs:655–739](https://github.com/estuariolabs/orfeon/blob/db73e7b/crates/orfeon-core/src/choir.rs#L655-L739)
- **Fin de línea**. Sostener la última sílaba hasta soltar su nota y usar la carta para ataques posteriores. Un botón «Reiniciar letra» vuelve los cursores al comienzo para el próximo ataque. Pánico libera voces y gestos. Al cargar programa o proyecto los cursores empiezan desde cero.
- **Edición**. Pestaña Letra con campos SATB. La tarjeta del cantante muestra su sílaba actual y permite reemplazar el texto de su cuerda. Letra vacía usa la carta de vocales por la ruta actual. Con letra, el núcleo determina la vocal y la carta queda disponible para cantantes sin texto. Persistir texto, variante y reemplazos mediante un campo versionado, con valor vacío por defecto para proyectos anteriores. [Orfeón, lib.rs del plugin:68–80](https://github.com/estuariolabs/orfeon/blob/db73e7b/crates/orfeon-plugin/src/lib.rs#L68-L80)

## Costo en tiempo real

Precompilar letra y gestos cuando se edita el texto o cambia el cuerpo del cantante. Conservar plantillas de ataque y cierre con pitch de referencia y ajustar los destinos de frecuencia a la nota plegada al copiar en memoria preasignada. Confirmar contra el planificador en varios registros y con deslizamientos antes de usar esa transposición. Publicar planes inmutables mediante IDs y generaciones. El hilo de audio selecciona y copia gestos a buffers acotados y libera por etiquetas. El hilo que prepara planes también recupera y libera los bancos viejos. Si el presupuesto de un banco se alcanza, rechazar la edición con indicación del límite y conservar el último plan válido. Evitar reconstruir cuerpos, asignar memoria o esperar a un hilo en el callback.

Validar en Alto y Eco, con pitch bend, expresión por nota, botones de pánico, robo de voz y guardado en un DAW. Medir ataques en ráfaga a 1, 8 y 32 voces. Estimación inicial de 6–10 días para frente, integración y proyectos más 2–4 días de escucha y medición.

## Prototipo y medición

`cargo run --locked --release -p orfeon-core --example letra_es -- /ruta/de/salida`

Una voz canta «Canta al sol», transcrita a mano como `kan.ta al sol` para probar ataque y coda. El ejemplo usa `Engine` de CHIMERA fuera de línea y deja la app y el plugin fuera del circuito. Genera dos WAV mono de 2,720 s a 48 kHz, con la misma melodía, cuerpo, vocales y tiempos de nota.

En Apple Silicon, bloques de 128, mediana de siete renders intercalados y una vuelta de calentamiento. Se cronometra programación, liberación y render. La preparación del plan, inicialización y escritura del WAV quedan fuera del tiempo medido.

| Variante | Tiempo | Carga respecto al tiempo real |
|---|---:|---:|
| Solo vocales | 541,2 ms | 19,90 % |
| Con letra | 551,5 ms | 20,28 % |

La diferencia observada fue de 10,3 ms por render, aproximadamente +1,9 % relativo. La referencia usa el mismo Engine y planificador con el ataque y la coda vacíos. La medición no cuantifica el costo de añadir letra a `Choir` ni la latencia de un plugin en vivo.
