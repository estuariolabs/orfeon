# Orfeón 0.3.0

En la barra superior, elegí **Cuarteto**. A la izquierda aparecen los cuatro cantantes. Seleccioná uno para ajustar su rango y preset vocal, escribir su letra y probarlo con el teclado de pantalla.

| Canal MIDI | Parte | Rango inicial |
|---:|---|---|
| 1 | Soprano | C4–C6 |
| 2 | Bajo | E2–E4 |
| 3 | Contralto | F3–F5 |
| 4 | Tenor | C3–C5 |

Con **Convertir a sílabas** podés revisar y editar el IPA antes de **Aplicar letras**. Usá puntos entre sílabas, por ejemplo `kan.ta.al.sol`. Un guion separado prolonga la sílaba anterior sobre una nota ligada. Las correcciones de IPA quedan a cargo del intérprete en pronunciaciones ajenas a las reglas básicas de español rioplatense.

La app permite guardar y abrir el cuarteto en `Orfeon-Cuarteto.json` del directorio de inicio. Para elegir un archivo específico, iniciá la app con la variable `ORFEON_PROJECT`. En el plugin, los rangos y presets vocales se guardan como parámetros y las letras aplicadas como estado del proyecto del DAW.

`orfeon cuarteto salida` genera un WAV de 30 segundos con letra y cuatro pistas. `orfeon bench --threads 1` registra carga media, p50, p99 y máximo por bloque de 128 muestras.

La entrada por cuatro canales se puede probar desde MIDI o un DAW. La emisión por cuerda de Tolhuin requiere un perfil de firmware propio. El firmware de Tolhuin distribuido hasta ahora envía el acorde por un solo canal.
