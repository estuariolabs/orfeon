# Créditos y licencias de terceros

Orfeón es software libre bajo GPL-3.0-or-later (ver `LICENSE`).

## CHIMERA

El motor de voz (pulmones, cuerdas vocales, tracto, nariz, posturas calibradas de las vocales)
es **CHIMERA**, de Jameson Hodge: <https://github.com/jamesondh/chimera-vocal-synth>, licencia
MIT. Orfeón lo usa como dependencia, sin modificarlo, fijado al commit `edd2676`.

```
MIT License

Copyright (c) 2026 Jameson Hodge

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## nih-plug (sólo el plugin)

El plugin usa **nih-plug**, de Robbert van der Helm: <https://github.com/robbert-vdh/nih-plug>
(ISC). Su exportación VST3 depende de `vst3-sys` (GPL-3.0), por eso el plugin compilado se
distribuye bajo GPL-3.0.

## Lo que Orfeón no usa

No contiene código, programas, nombres, textos ni gráficos de ningún producto comercial. Las
funciones parecidas a las de otros instrumentos de coro están escritas desde cero sobre CHIMERA.
