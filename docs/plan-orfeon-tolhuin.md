# Orfeón + Tolhuin en vivo

La meta es dirigir un coro desde los botones y el joystick de Tolhuin, con cuatro líneas reconocibles y fraseo expresivo. Usar HUMANist + CHORDimist como referencia de escucha. La comparación necesita el mismo MIDI, nivel igualado y versiones en seco, para separar conducción, timbre y mezcla.

## 1. Cuarteto coral

El programa 37 añade cuatro cantantes mixtos. La conducción por evento favorece el registro y los movimientos cortos de cada cantante. Un tono común sostenido conserva su cantante y ataque. El escalonado de grave a agudo se puede ajustar entre 0 y 40 ms por separación aproximada, con 24 ms en el preset. Sumado a la humanización, el último cantante puede entrar unos 80 ms después del primero. Los parámetros nuevos tienen IDs propios y valores iniciales desactivados para los proyectos anteriores.

Claude Code revisó el código público de Orfeón y propuso distinguir entre la conducción inferida desde notas y la identidad explícita de una cuerda. También señaló el orden de presets y enums, la persistencia del plugin, el presupuesto de latencia y la prueba de tonos comunes. El primer cambio usa un cálculo acotado por nota para funcionar con app, plugin y firmware actuales. La propuesta de Claude de agrupar eventos en unos 10 ms y buscar una asignación conjunta queda para la fase siguiente. El orden de llegada aún puede cambiar el reparto, no hay garantía de ausencia de cruces y notas simultáneas del mismo pitch copiadas a un solo canal no identifican dos cantantes.

## 2. Identidad y recuperación MIDI · 3–6 días de implementación y banco

Reutilizar el plan de integración y su instantánea de acorde. Probar primero USB con un único puerto, demora y secuencia observables y recuperación forzada. Para SATB, incorporar una variante explícita con canal por cuerda, usando el orden de la conducción en Tolhuin. Mantener el mapa estándar como alternativa seleccionable. Validar unísonos, cruces, ataques de distinto timestamp, cambio de puerto, pánico, desconexión y pérdida de trama. Medir el costo de agrupar diffs frente a enviar la identidad desde origen. Evaluar BLE con la misma secuencia y trazas.

## 3. Joystick y arreglos · 3–5 días

Crear una capa de expresión optativa. X cambia la vocal, Y la proyección entre suave e intensa. Suavizar los CC a nivel de audio y ajustar la rampa a oído. Probar tres arreglos con la misma progresión: entrada junta, entrada escalonada y respuesta alternada entre cuerdas. Grabar también Tolhuin arpegiado con audio propio y Orfeón sostenido. Criterios: gesto audible sin escalones, acorde que se completa dentro de 150 ms y ninguna nota residual tras soltar.

## 4. Letra y CPU · 8–14 días

Portar el frente español de Tolhuin a Rust y transformar la letra en sílabas IPA de CHIMERA. Preparar ataques, vocales y cierres fuera del audio. Cada cuerda tiene su línea y cursor. Un cambio armónico puede mantener la sílaba, y la interfaz permite marcar el siguiente ataque de texto. Empezar escuchando una frase sola y luego al unísono en cuatro cantantes. Medir claridad, ataques, cortes y costo de render en Mac y Acer. Publicar una comparación con HUMANist sólo después de escuchar la referencia en condiciones comparables.

## Cómo reproducir la comparación

Desde la rama local `codex/fraseo-coral`, renderizá el mismo archivo de notas dos veces con `--preset "Cuarteto coral" --score /ruta/cadencia-satb.txt --threads 4`. Sumá `--sin-fraseo` a una toma. La partitura conserva los tonos comunes y el par entregado está igualado en RMS. Es una comparación del fraseo interno, con la misma alineación de salida, sin captura de hardware ni audio de HUMANist.
