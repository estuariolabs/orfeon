# Orfeón

Coro de cantantes modelados físicamente. Sin samples y sin modelos neuronales: cada cantante
es una simulación propia de pulmones, cuerdas vocales, garganta, boca y nariz, con su cuerpo,
su rostro y su manera de cantar.

Libre (GPL-3.0-or-later). El motor de voz es [CHIMERA](https://github.com/jamesondh/chimera-vocal-synth)
de Jameson Hodge (MIT), usado como dependencia sin modificar. Ver [`NOTICE.md`](NOTICE.md).

## Estado

| Parte | Estado |
|---|---|
| Núcleo (`crates/orfeon-core`) | Compila. 12 tests pasan. |
| Línea de comandos (`crates/orfeon-cli`) | Compila y rinde audio. Es la forma verificada de escucharlo. |
| App independiente (`crates/orfeon-app`) | Compila para Windows y Linux. Probada bajo Wine con una placa de sonido virtual: abre el audio, canta y responde a los comandos. **No se probó con parlantes reales ni con un teclado MIDI.** |
| Plugin CLAP / VST3 (`crates/orfeon-plugin`) | Compila y se empaqueta para Linux y Windows. **Nunca se cargó en un DAW.** |
| Sonido | Nadie lo escuchó todavía: estilos, programas y niveles se eligieron a mano y por medición. |

## Qué hace

- **Hasta 64 cantantes**, cada uno con su propia simulación de voz.
- **Rostro y cuerpo**: cuello, tamaño, mandíbula, labios, nariz y edad cambian la anatomía que
  se simula. Hay un rostro común y una "variedad" que aparta a cada cantante de él.
- **Siete tipos de voz** (bajo, barítono, tenor, contralto, mezzo, soprano, niño) y **ocho
  conjuntos** (mixto, femenino, masculino, niños y cada cuerda sola).
- **Tres modos**: *Coro* (cada nota va al cantante siguiente, prefiriendo a quien le quede en
  registro), *Tutti* (todos cantan la última nota, cada uno en su octava) y *Solo* (un
  cantante, ligado).
- **Ocho estilos**: Clásico, Antiguo, Pop, Gospel, Folk, Susurro, Belting, Rasgado.
- **Carta de vocales** en dos ejes, con movimiento automático y diferencias entre cantantes.
- **Cada cantante por su cuenta**: afinación que deriva, vibrato propio, entrada más o menos
  tardía, intensidad que se balancea.
- **Escenario, sala y micrófono**: lugar de cada cantante (cuerdas de izquierda a derecha,
  graves atrás), cinco salas y distancia del micrófono.
- **Expresión por nota** (MPE): afinación, presión y brillo de cada nota.
- **36 programas** de fábrica.

## Probarlo

Hace falta [Rust](https://rustup.rs) (probado con 1.97).

### 1. Tests y primera escucha (lo verificado)

```sh
cargo test --release                 # 12 tests, unos 10 s
cargo run --release -p orfeon-cli -- presets
cargo run --release -p orfeon-cli -- render coro.wav --preset 1
cargo run --release -p orfeon-cli -- render vocales.wav --preset 1 --demo vocales
cargo run --release -p orfeon-cli -- render solo.wav --preset 29 --demo melodia
cargo run --release -p orfeon-cli -- singers --preset 2     # quiénes forman el coro
```

Para rendir tu propia música, un archivo de texto con una nota por línea
(`inicio duración nota [velocidad]`, en segundos y número MIDI):

```
# acorde de do mayor y una nota arriba
0.0 2.0 48
0.0 2.0 55
0.0 2.0 64
1.0 1.0 72 0.9
```

```sh
cargo run --release -p orfeon-cli -- render salida.wav --score notas.txt --preset 7
```

Opciones: `--mode coro|tutti|solo`, `--singers N`, `--style 1..8`, `--room 1..5`,
`--vowel a|e|i|o|u`, `--voices N`, `--eco`, `--threads N`, `--tail SEG`.

### 2. La app independiente (sin DAW)

```sh
cargo run --release -p orfeon-app
```

Abre la salida de audio por defecto y el primer teclado MIDI que encuentre. Se toca con el
teclado MIDI o con comandos (`n 48 55 64 72`, `demo`, `p catedral`, `ayuda`).

Al arrancar mide cuántas voces aguanta la máquina y fija ese tope (si en calidad Alta entran
menos de 4, pasa a Eco). El coro se calcula en un hilo propio con prioridad de audio y deja
un colchón de 40 ms; la placa de sonido sólo copia de ese colchón. `info` muestra la carga y
los cortes; si hay cortes, `latencia 80` agranda el colchón y `voces N` baja el tope. En Linux hace
falta `libasound2-dev` y `pkg-config` para compilarla.

### 3. Cuántas voces aguanta tu máquina

```sh
cargo run --release -p orfeon-cli -- bench
cargo run --release -p orfeon-cli -- bench --eco
```

Mide la carga para 1, 2, 4, 8, 16, 32 y 64 voces. Por debajo de ~70 % se puede tocar en vivo.
Usá el resultado para fijar "Voces a la vez" en el plugin.

### 4. El plugin

```sh
cargo xtask bundle orfeon-plugin --release
```

Deja `target/bundled/Orfeon.clap` y `Orfeon.vst3` para el sistema donde lo compiles (hay que
compilarlo en Windows para Windows, en macOS para macOS). Copialos a la carpeta de plugins:

| Sistema | CLAP | VST3 |
|---|---|---|
| Windows | `C:\Program Files\Common Files\CLAP` | `C:\Program Files\Common Files\VST3` |
| macOS | `~/Library/Audio/Plug-Ins/CLAP` | `~/Library/Audio/Plug-Ins/VST3` |
| Linux | `~/.clap` | `~/.vst3` |

Qué probar, en este orden:

1. Que el DAW lo liste como instrumento y cargue sin errores.
2. Tocar una nota: tiene que sonar una "a" cantada.
3. Tocar un acorde de cuatro notas: cuatro cantantes distintos, repartidos en el estéreo.
4. Mover "Programa" de `Manual` a cualquier otro: cambia el coro entero.
5. Con "Programa" en `Manual`: mover Modo, Estilo, las dos perillas de Vocal y Sala.
6. Mirar el medidor de CPU del DAW con 4 y 8 notas. Si se entrecorta: bajar "Voces a la vez",
   poner "Calidad" en `Eco`, o subir el tamaño de buffer del DAW.

No tiene interfaz propia: los controles aparecen en el editor genérico del DAW. La
compilación y el empaquetado se probaron sólo en Linux; cargarlo y tocarlo en un DAW no se
probó en ningún sistema. Todo lo que habla con el DAW está en
`crates/orfeon-plugin/src/lib.rs`.

## Cuánto cuesta

Cada cantante que suena es una simulación completa. Medido en una máquina virtual de 4
núcleos (la tuya probablemente sea más rápida); 100 % = tiempo real:

| Voces a la vez | Alta, 4 hilos | Eco, 4 hilos | Alta, 1 hilo |
|---|---|---|---|
| 1 | 23 % | 11 % | 36 % |
| 4 | 32 % | 20 % | 91 % |
| 8 | 67 % | 38 % | 216 % |
| 16 | 124 % | 58 % | 416 % |
| 32 | 222 % | 113 % | — |
| 64 | 427 % | 230 % | — |

- Se pueden **definir** 64 cantantes, pero en vivo suenan a la vez los que la máquina
  aguante. "Voces a la vez" es ese tope; al pasarlo, la nota nueva toma el lugar de la más vieja.
- Fuera de tiempo real no hay tope: la línea de comandos y el bounce offline del DAW rinden
  cualquier cantidad.
- **Eco** corre el motor a ~30 kHz y remuestrea: cuesta más o menos la mitad y pierde los
  agudos por encima de 15 kHz.

## Diferencias con los coros modelados comerciales

Orfeón apunta a las mismas funciones que los instrumentos comerciales de coro por modelado
físico (hasta 64 cantantes con rostro y cuerpo, modos coro/tutti/solo, estilos, sala y
micrófonos, carta de vocales, 36 programas). No usa código, programas ni nombres de ninguno.
Lo que hoy **no** iguala:

- **Voces simultáneas en vivo.** El motor de CHIMERA es mucho más pesado por voz: 8 a 16
  voces en una máquina común, no 64.
- **Interfaz.** No hay ventana propia ni caras dibujadas; sólo los controles del DAW.
- **Programas.** En el plugin se eligen con el control "Programa"; no aparecen en el
  navegador de presets del DAW.
- **Micrófonos.** Hay una distancia de micrófono, no modelos de micrófono ni varias tomas.
- **MPE.** Recibe expresiones por nota de CLAP y la rueda de afinación; no se probó con un
  controlador MPE.
- **Ajuste a oído.** Estilos y programas no fueron comparados contra grabaciones.

Lo que podría sumar más adelante y los comerciales no anuncian: CHIMERA ya canta consonantes
y letra (inglés y coreano). Orfeón todavía no lo expone.

## Cómo está hecho

```
crates/orfeon-core/     el coro (sin E/S)
  singer.rs             tipo de voz, rostro -> anatomía, rasgos personales
  vowel.rs              carta de vocales -> postura de la boca
  style.rs              los ocho estilos
  choir.rs              reparto de notas, dirección de cada voz, hilos, escenario
  room.rs               reverberación
  preset.rs             los 36 programas
  tests/choir.rs        tests
crates/orfeon-cli/      rinde WAV, lista programas, mide la carga
crates/orfeon-app/      app independiente: audio en tiempo real, MIDI y comandos
  (los comandos se interpretan en orfeon-core/src/console.rs, con sus tests)
crates/orfeon-plugin/   plugin CLAP/VST3 (nih-plug)
xtask/                  empaquetador del plugin (`cargo xtask bundle`)
```

- **Del rostro a la voz.** CHIMERA simula un tracto a partir de cuatro medidas (tamaño, largo
  de faringe, lengua, labios). Cada tipo de voz parte de una mezcla de sus tres cuerpos
  calibrados (hombre, mujer, niño); el rostro mueve esas medidas dentro de límites que el
  motor aguanta. La nariz abre un poco el velo. La edad acerca el cuerpo al de un niño o
  agrega aspereza y aire.
- **Qué controla Orfeón por voz**, en cada tramo de 128 muestras: altura (con desafinación,
  deriva, ataque desde abajo y expresión por nota), presión de pulmones, tensión, aire,
  brillo, aspereza, vibrato con entrada retardada, postura de la vocal y respiración.
- **Hilos.** Las voces activas se reparten entre hilos ayudantes; el resultado es idéntico
  muestra a muestra con cualquier cantidad de hilos (hay un test).
- **Tiempo real.** Tocar notas y mover controles no reserva memoria. Cambiar cantidad de
  cantantes, conjunto, elenco, rostro o calidad reconstruye las voces: reserva memoria y
  corta lo que sonaba.

## Compilar para Windows desde Linux

Con `mingw-w64` instalado y el destino de Rust agregado
(`rustup target add x86_64-pc-windows-gnu`):

```sh
cargo build --release --target x86_64-pc-windows-gnu -p orfeon-app -p orfeon-cli
cargo xtask bundle orfeon-plugin --release --target x86_64-pc-windows-gnu
```

Los `.exe` y los plugins salen enlazados estáticamente: no necesitan DLL aparte. El
`orfeon.exe` así compilado, corrido bajo Wine, rinde un WAV idéntico byte a byte al de la
versión de Linux.

## Licencia

GPL-3.0-or-later. Ver [`LICENSE`](LICENSE) y [`NOTICE.md`](NOTICE.md).
