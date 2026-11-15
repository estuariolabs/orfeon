# Etapa 3 · Consonantes y vocales desde el micrófono

La entrada de micrófono dirige **cuándo y cómo articula** el cuarteto. El MIDI fija las alturas y la conducción conserva los cantantes de cada línea. La vocal del núcleo puede sostenerse en varias notas. Las consonantes indican cuándo empezar una sílaba, cerrar la anterior o cambiar la forma del tracto aunque siga sonando la misma nota.

## Interpretación en tiempo real

El primer prototipo usa un micrófono mono en la app. Un trabajador recibe muestras con su marca de tiempo por una cola acotada, detecta actividad y estima energía por banda, sonoridad, transitorios y formantes. Un clasificador causal estima clases articulatorias y, cuando hay evidencia suficiente, fonemas en IPA. Trabaja sobre ventanas de unos 20 ms, con un salto de 5–10 ms, y conserva un estado por sílaba. Medir cierre, ruido y sonoridad evita tratar un estallido breve como si fuese una vocal.

| Gesto | Qué detectar | Qué hace el cantante |
| --- | --- | --- |
| Oclusivas /p t k b d g/ | Caída de energía o cierre, estallido y tiempo hasta el comienzo de la voz | Cierra, libera y enlaza con la vocal. El punto de articulación y la sonoridad eligen el gesto. Para /b d g/ intervocálicas se acepta una aproximante sin inventar un estallido. |
| Fricativas /s f x ʃ/ y africada /tʃ/ | Ruido sostenido y distribución espectral. En /tʃ/, cierre seguido de ruido | Conserva la duración percibida del ruido y suaviza la entrada de la vocal. |
| Nasales /m n ɲ/ | Sonoridad, resonancias nasales y transición al núcleo | Abre la vía nasal y mantiene el tono MIDI si el gesto es sonoro. |
| Líquidas /l ɾ r/ y deslizamientos | Transiciones de formantes, interrupción corta o varias en la vibrante | Transición lateral, toque o vibración cuando la evidencia alcanza. Un deslizamiento entra o sale del núcleo sin disparar otra sílaba. |
| Vocales /a e i o u/ | Sonoridad estable y formantes | Mueve el tracto hacia la vocal. Una nota nueva durante el núcleo conserva la vocal y permite el melisma. |

Ejemplo con «tres». El cierre y el estallido de /t/ disparan el ataque, /ɾ/ enlaza con /e/, la garganta sostiene /e/ durante las notas de un melisma y /s/ cierra la sílaba una sola vez. Cuando el micrófono detecta /s/ al final de una palabra, la coda puede terminar aunque el acorde ya haya recibido note-off. En un coro, las consonantes iniciales deben quedar próximas entre cantantes para preservar inteligibilidad. El escalonado puede aplicarse al núcleo y al fin de la coda, con un rango de dispersión menor para consonantes.

## Dos modos de entrada

- **Letra guiada.** Orfeón genera IPA y sílabas editables a partir del texto. Un alineador restringido a los siguientes fonemas esperados usa el micrófono para decidir tiempos. Es el primer prototipo porque el texto resuelve buena parte de las ambigüedades acústicas. Si cada cantante tiene letra distinta, el micrófono puede dirigir un cantante o dar pulsos de avance a todo el cuarteto. Cada parte pronuncia sus propios fonemas.
- **Articulación libre.** El mismo detector emite clases y candidatos IPA sin letra previa. Una consonante clara genera un gesto directo. Con confianza baja, conserva sólo la clase y su duración, o mantiene la vocal vigente. Un micrófono mono controla una línea o una articulación compartida. Se necesitarían entradas independientes para seguir varias personas a la vez.

## Integración y pruebas

1. Preparar clips cortos de habla y canto en español rioplatense con límites manuales de consonante y vocal. Cubrir pares como «pala/bala», «tasa/casa», «coro/corro», /s/ final, /tʃ/, grupos como /tr/ y variantes rioplatenses de /ʝ/. Separar hablantes entre desarrollo y evaluación.
2. Implementar detección y alineación fuera del hilo de audio. Pasar eventos acotados con tiempo, clase, confianza y posición de ataque, núcleo o coda. Transformarlos en gestos del planificador de CHIMERA con las alturas MIDI de cada parte. Validar cada fonema que el motor puede representar y acordar una aproximación audible para los restantes.
3. Probar un cantante y luego cuatro. Cancelar eventos pendientes al pulsar pánico o al desactivar el micrófono. No abrir el micrófono hasta activar el modo, ni guardar su audio. Dejar el monitoreo directo apagado y comprobar el comportamiento con auriculares antes de medir la realimentación por altavoces.
4. Medir latencia de gesto desde la captura (mediana y percentil 95), aciertos y falsos disparos por clase, precisión de los límites y errores de división silábica. Probar un modo inmediato con gestos de clase y un modo guiado con una ventana de anticipación corta, ajustable según la inteligibilidad. Como metas iniciales, buscar error mediano de límite menor a 30 ms en letra guiada y percentil 95 menor a 120 ms desde captura hasta gesto. Medir ambos también con la carga de cuatro voces.
5. Hacer escucha ciega comparando vocales solas, letra guiada y articulación libre. Pedir transcripción de sílabas, con atención especial a consonantes iniciales y finales y a la ausencia de ataques extra durante melismas. Probar después la entrada lateral en CLAP/VST3 con los mismos criterios.
