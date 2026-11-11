# Ablation del pipeline · resultados parciales (10 nov)

Pregunta: ¿cuánto aporta cada pieza del enhance? Se compara el pipeline completo (`full`) con el mismo pipeline quitando una sola pieza cada vez.

**Estado: incompleto.** La ejecución se paró el 10 nov a mitad de la cuarta reunión y no se ha reanudado. Las conclusiones de abajo son provisionales: solo hay reuniones en castellano y faltan las dos más largas.

## Montaje

- Modelo LFM3-1.7B Q8_0, 3 semillas por variante. Segmentos generados desde los guiones con `tools/script_to_segments.py`, sin STT.
- Cada reunión se corre con su plantilla y en Auto.
- Variantes (interruptores de `eval_enhance`):
  - `full`: el pipeline tal como está (`itn`).
  - `noitn`: sin pasar los números escritos con letras a dígitos.
  - `nodetail`: sin las pasadas de detalle.
  - `nopromote`: sin promover tareas a su sección.
  - `nocitelimit`: sin el tope de citas por viñeta.
  - `nonotes`: sin las notas del usuario en el prompt.
  - `nofloor`: sin el mínimo de tareas en Auto (solo Auto).
  - `gbnf`: con la gramática encendida (solo plantilla; está desactivada en el pipeline).
  - `nosuffix`: sin el sufijo de idioma (solo en `weekly_planning`, en inglés).
- Columnas: media de las semillas. «Tareas» suma las tareas del guion encontradas sobre las esperadas en las tres semillas. «Colocación» es el porcentaje de hechos que caen en su apartado de la plantilla. «s» es el tiempo por resumen.

| Reunión | Idioma | Plantilla | Auto |
|---|---|---|---|
| stand_up | es | ✅ | ✅ |
| uno_a_uno | es | ✅ | ✅ |
| llamada_ventas | es | ✅ | ✅ |
| entrevista_cliente | es | ✅ | ⏸️ hasta `nofloor` semilla 0 |
| weekly_planning | en | ⬜ | ⬜ |
| revision_trimestral | es | ⬜ | ⬜ |

También falta la medición de Q4_K_M (`tools/ablation/run_q4.sh`), que no llegó a empezar.

## Lectura provisional

Con la misma semilla, una variante que no toca nada sale idéntica a `full`. Por eso las diferencias pequeñas sí son efecto de la pieza quitada, aunque con 3 semillas y 4 reuniones no se puede generalizar.

- **`nofloor`** (mínimo de tareas en Auto) es la pieza que más se nota. Sin ella, las tareas encontradas bajan en las cuatro reuniones:

  | Reunión | Con mínimo | Sin mínimo |
  |---|---|---|
  | stand_up | 9/12 | 5/12 |
  | uno_a_uno | 11/15 | 6/15 |
  | llamada_ventas | 9/12 | 3/12 |
  | entrevista_cliente | — | 4/8 (una semilla) |

  Se queda.
- **`nodetail`** (pasadas de detalle):
  - Pesa poco en las reuniones cortas.
  - Pesa mucho en la entrevista: la cobertura baja del 78 al 51 % con plantilla y del 84 al 59 % en Auto.
  - Cuesta cerca del doble de tiempo (79 frente a 48 s). Se queda.
- **`nopromote`** (promover tareas a su sección):
  - Igual que `full` en tres reuniones.
  - En la entrevista en Auto, sin ella las tareas bajan de 11/24 a 7/24.
  - Se queda, pendiente de confirmar con más reuniones.
- **`nocitelimit`** (tope de citas por viñeta): sin efecto medible en ninguna reunión. Si se confirma en las que faltan, es candidata a quitarse.
- **`noitn`**: resultado mixto en castellano.
  - Las cifras fieles suben a veces sin ITN (stand_up con plantilla: 82 → 100 %), pero sobre menos cifras en las notas.
  - La cobertura sube en unas reuniones y baja en otras.
  - Su beneficio claro estaba en inglés (ver `experimentos.md`), y esa reunión aún no se ha corrido.
- **`nonotes`** (notas del usuario):
  - Baja el porcentaje de texto copiado.
  - A veces sube las tareas y a veces la cobertura cae (stand_up con plantilla: 95 → 83 %).
  - Sin dirección clara. Hay que mirarlo con la reunión en inglés.
- **`gbnf`**: calidad parecida a `full` y bastante más lenta (llamada_ventas 29 → 49 s, uno_a_uno 24 → 39 s). No compensa encenderla.

## Cómo reanudar

```bash
cargo build --release -p juanola-enhance --example eval_enhance
tools/ablation/run_ablation.sh runs/ablation entrevista_cliente:auto weekly_planning revision_trimestral
python3 tools/ablation/resumen.py runs/ablation/eval.log
```

Después, la cuantización, con la ruta del Q4_K_M:

```bash
JUANOLA_GGUF=/ruta/LFM3-1.7B-Q4_K_M.gguf tools/ablation/run_q4.sh runs/ablation_q4
```

El log en bruto de esta tanda está en [`eval-parcial-10nov.log`](eval-parcial-10nov.log).

## Tablas

Generadas con `python3 tools/ablation/resumen.py docs/ablation/eval-parcial-10nov.log`.

### stand_up · plantilla

| Variante | Semillas | Cobertura | Tareas | Cifras fieles | Copiadas | Colocación | s |
|---|---|---|---|---|---|---|---|
| full | 3 | 95 % | 12/12 | 82 % | 53 % | 71 % | 22 |
| noitn | 3 | 95 % | 12/12 | 100 % | 46 % | 71 % | 24 |
| nodetail | 3 | 89 % | 12/12 | 100 % | 66 % | 73 % | 22 |
| nopromote | 3 | 95 % | 12/12 | 82 % | 53 % | 71 % | 28 |
| nocitelimit | 3 | 95 % | 12/12 | 82 % | 53 % | 71 % | 23 |
| nonotes | 3 | 83 % | 12/12 | 84 % | 36 % | 71 % | 25 |
| gbnf | 3 | 95 % | 12/12 | 83 % | 45 % | 77 % | 24 |

### stand_up · auto

| Variante | Semillas | Cobertura | Tareas | Cifras fieles | Copiadas | s |
|---|---|---|---|---|---|---|
| full | 3 | 100 % | 9/12 | 84 % | 38 % | 22 |
| noitn | 3 | 86 % | 12/12 | 93 % | 16 % | 20 |
| nodetail | 3 | 89 % | 9/12 | 100 % | 48 % | 16 |
| nopromote | 3 | 100 % | 9/12 | 84 % | 39 % | 23 |
| nocitelimit | 3 | 100 % | 9/12 | 84 % | 38 % | 26 |
| nonotes | 3 | 97 % | 10/12 | 81 % | 50 % | 21 |
| nofloor | 3 | 94 % | 5/12 | 81 % | 42 % | 19 |

### uno_a_uno · plantilla

| Variante | Semillas | Cobertura | Tareas | Cifras fieles | Copiadas | Colocación | s |
|---|---|---|---|---|---|---|---|
| full | 3 | 100 % | 13/15 | 100 % | 21 % | 76 % | 24 |
| noitn | 3 | 100 % | 15/15 | 97 % | 26 % | 81 % | 24 |
| nodetail | 3 | 88 % | 13/15 | 100 % | 22 % | 81 % | 23 |
| nopromote | 3 | 100 % | 13/15 | 100 % | 20 % | 76 % | 26 |
| nocitelimit | 3 | 100 % | 14/15 | 100 % | 19 % | 76 % | 26 |
| nonotes | 3 | 100 % | 15/15 | 98 % | 11 % | 76 % | 23 |
| gbnf | 3 | 100 % | 13/15 | 98 % | 24 % | 81 % | 39 |

### uno_a_uno · auto

| Variante | Semillas | Cobertura | Tareas | Cifras fieles | Copiadas | s |
|---|---|---|---|---|---|---|
| full | 3 | 93 % | 11/15 | 95 % | 25 % | 19 |
| noitn | 3 | 100 % | 11/15 | 100 % | 28 % | 20 |
| nodetail | 3 | 91 % | 11/15 | 100 % | 26 % | 20 |
| nopromote | 3 | 93 % | 11/15 | 95 % | 25 % | 26 |
| nocitelimit | 3 | 93 % | 11/15 | 95 % | 25 % | 26 |
| nonotes | 3 | 100 % | 13/15 | 100 % | 13 % | 34 |
| nofloor | 3 | 93 % | 6/15 | 95 % | 24 % | 25 |

### llamada_ventas · plantilla

| Variante | Semillas | Cobertura | Tareas | Cifras fieles | Copiadas | Colocación | s |
|---|---|---|---|---|---|---|---|
| full | 3 | 76 % | 12/12 | 100 % | 36 % | 98 % | 29 |
| noitn | 3 | 89 % | 12/12 | 100 % | 37 % | 96 % | 22 |
| nodetail | 3 | 70 % | 12/12 | 100 % | 36 % | 98 % | 17 |
| nopromote | 3 | 76 % | 12/12 | 100 % | 36 % | 98 % | 22 |
| nocitelimit | 3 | 76 % | 12/12 | 100 % | 36 % | 98 % | 27 |
| nonotes | 3 | 79 % | 12/12 | 100 % | 26 % | 100 % | 27 |
| gbnf | 3 | 78 % | 12/12 | 96 % | 29 % | 100 % | 49 |

### llamada_ventas · auto

| Variante | Semillas | Cobertura | Tareas | Cifras fieles | Copiadas | s |
|---|---|---|---|---|---|---|
| full | 3 | 90 % | 9/12 | 100 % | 14 % | 33 |
| noitn | 3 | 87 % | 12/12 | 100 % | 21 % | 24 |
| nodetail | 3 | 80 % | 9/12 | 100 % | 15 % | 21 |
| nopromote | 3 | 90 % | 9/12 | 100 % | 14 % | 26 |
| nocitelimit | 3 | 90 % | 9/12 | 100 % | 14 % | 29 |
| nonotes | 3 | 83 % | 11/12 | 96 % | 17 % | 29 |
| nofloor | 3 | 90 % | 3/12 | 100 % | 16 % | 23 |

### entrevista_cliente · plantilla

| Variante | Semillas | Cobertura | Tareas | Cifras fieles | Copiadas | Colocación | s |
|---|---|---|---|---|---|---|---|
| full | 3 | 78 % | 16/24 | 90 % | 26 % | 71 % | 79 |
| noitn | 3 | 73 % | 17/24 | 96 % | 26 % | 81 % | 105 |
| nodetail | 3 | 51 % | 16/24 | 87 % | 26 % | 87 % | 48 |
| nopromote | 3 | 78 % | 16/24 | 90 % | 26 % | 71 % | 102 |
| nocitelimit | 3 | 78 % | 16/24 | 90 % | 26 % | 71 % | 86 |
| nonotes | 3 | 79 % | 18/24 | 88 % | 26 % | 72 % | 76 |
| gbnf | 3 | 84 % | 15/24 | 91 % | 24 % | 74 % | 112 |

### entrevista_cliente · auto

| Variante | Semillas | Cobertura | Tareas | Cifras fieles | Copiadas | s |
|---|---|---|---|---|---|---|
| full | 3 | 84 % | 11/24 | 86 % | 19 % | 89 |
| noitn | 3 | 83 % | 12/24 | 89 % | 14 % | 100 |
| nodetail | 3 | 59 % | 8/24 | 76 % | 7 % | 52 |
| nopromote | 3 | 84 % | 7/24 | 86 % | 18 % | 87 |
| nocitelimit | 3 | 84 % | 11/24 | 86 % | 19 % | 96 |
| nonotes | 3 | 81 % | 15/24 | 82 % | 14 % | 123 |
| nofloor | 1 | 85 % | 4/8 | 86 % | 19 % | 89 |
