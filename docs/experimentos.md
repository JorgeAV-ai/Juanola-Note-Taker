# Registro de experimentos

Qué se ha probado, qué resultado dio y si se quedó o se descartó, para no repetir experimentos. Arriba, una tabla por área con todos los experimentos; abajo, el detalle (tablas por semilla, hallazgos), agrupado igual. La columna «Detalle» apunta a la sección de este documento o a otro.

**Antes de probar algo, busca aquí.** Al terminar un experimento, añade una fila aunque salga mal: un descarte medido vale tanto como una mejora.

Estados: ✅ adoptado · ❌ descartado (medido) · ⚗️ funciona pero queda como opción desactivada · ⏳ idea sin probar.

**Foco (desde el 9 nov): solo castellano e inglés.** Para acotar el tiempo, los experimentos se miden y se ajustan en reuniones en esos dos idiomas. Los guiones en portugués, francés, alemán e italiano se quedan en `fixtures/` pero no se usan para decidir; el resto de idiomas tiene que seguir funcionando, sin más.

## Cómo se mide

- Enhance: `cargo run --release -p juanola-enhance --example eval_enhance -- <segmentos.json> <hechos.facts.json> [notas.txt] --runs 3 --variant … [--template clave|archivo.json] [--title …]`. Guiones en `crates/enhance/fixtures/`. Para pasarlos a segmentos: `tools/script_to_segments.py`.
- Con plantilla mide la colocación y dice adónde fue cada hecho mal colocado (`id → apartado`). Para probar títulos sin tocar el código, pasa la plantilla como JSON con `--template archivo.json`: la clave de `section` en los hechos es el nombre del archivo.
- 3 semillas como mínimo. Entre semillas hay ±10 puntos de ruido, así que una diferencia menor no se toma como mejora.
- Interruptores del evaluador para apagar piezas (ablation): `noitn`, `+nodetail`, `+nopromote`, `+nocitelimit`, `+nofloor` (tope de Auto), `+nonotes` (notas del usuario fuera del prompt), `+nosuffix` (sufijo de idioma); `+gbnf` y `+tasks` encienden las opciones desactivadas.

## Ablation del pipeline (10 nov)

Parado a medias: el pipeline completo frente al mismo pipeline sin cada pieza adoptada, en las seis reuniones en castellano e inglés (plantilla y Auto, 3 semillas). Hay 4 de 6 reuniones, todas en castellano. Provisional: el mínimo de tareas de Auto y las pasadas de detalle pesan mucho; el tope de citas no se nota; la gramática solo añade tiempo. Tablas, lectura y cómo reanudar en [ablation/README.md](ablation/README.md).

# Resumen por área

## Enhance · formato y prompt

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Salida en JSON forzada con gramática | El modelo copia líneas de la transcripción y la calidad se hunde | ❌ | 6 nov | [historial](historial.md), punto 3 |
| Markdown (`## tema`, viñetas con `[n, m]`) que el código convierte a `EnhancedNotes` | Funciona; base de todo lo demás | ✅ | 6 nov | [historial](historial.md), punto 3 |
| Pedir «más viñetas» o «todas las cifras» en una pasada | Sin mejora estable | ❌ | 6 nov | [historial](historial.md), punto 3 |
| `prompt_suffix` por idioma para resúmenes que no son en español | Sin él se colaban «Yo»/«Ellos» en las viñetas | ✅ | 7 nov | [historial](historial.md), punto 4 |
| Notas del usuario separadas por bloque (`user_blocks`) | Auto no empeora (89 % de cobertura, igual que antes) | ✅ | 8 nov | «Plantillas: iteraciones» (A frente a B) |

## Enhance · números (ITN)

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| ITN: números escritos con letras pasados a dígitos antes del LLM | Cifras fieles del 86 % al 98-100 %; copiadas del 70 % al 12-24 % | ✅ | 6 nov | [historial](historial.md), punto 3 |
| ITN también en inglés con `text-processing-rs` 0.3.3 (NeMo, Apache-2.0, `normalize_sentence_lang`; el castellano sigue con el propio) | Weekly, 3 semillas. Auto: cobertura 63/65/63 → 88/63/74 %, cifras en las notas 85-115 → 132-216, tareas 3/7/5 → 5/5/9. Plantilla: igual (cobertura 77/88/70 → 77/79/81 %). Cifras fieles ~80-95 % con y sin él | ✅ | 10 nov | Explicaba el «~0 %» de cifras fieles en inglés: el evaluador no normalizaba el inglés. Ahora sí (y quita las comas de millar) |

## Enhance · generación

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Pasadas de detalle cada 4 min (`detail_chunk_secs = 240`) | Cobertura del 69-77 % al 81-96 %; tarda ~60-80 s frente a ~20 s | ✅ | 6 nov | [historial](historial.md), punto 3 |
| Resumir por fragmentos y reagrupar temas con el modelo (JSON) | 25-39 secciones o reagrupa de más; pierde tareas | ❌ | 6 nov | [historial](historial.md), punto 3 |
| Resumen y pasadas de detalle a la vez, en un lote multi-secuencia de llama.cpp (`complete_many`) | Mismas métricas en todas las semillas (las salidas no cambian) y 35-55 % menos tiempo: semanal en Auto 67 → 31 s, con plantilla 72 → 40 s; 1:1 en Auto 20 → 13 s. Con Q8_0 queda por debajo de lo que tardaba Q4_K_M una a una | ✅ | 9 nov | «Generación en lote», abajo |
| Flash attention o cambiar `n_ubatch` | No cambia la velocidad en contexto largo | ❌ | 6 nov | [historial](historial.md), punto 3 |
| Cerrar cada cita al llegar a 6 referencias, en el bucle de llama.cpp (`GenOptions::max_refs`), Auto y plantilla | Ninguna cita de más de 6 (antes hasta 410); topes de 3000 tokens de 6 tandas a 1 (otra causa); algo más de tareas y de colocación a los 60 min | ✅ | 9 nov | «Límite de citas» |
| Tope de Auto: base + 400, mínimo 800 (`summary_budget`); antes ~600 en reuniones de 4-5 min | Todos los resúmenes Auto cortos se cortaban antes de «Próximos pasos». Tareas en Auto: 1:1 1/5 → 4-5/5, ventas 1/4 → 2-4/4, stand-up 1/4 → 1-4/4, revisión trimestral 5-6 → 6-9/14 | ✅ | 9 nov | «Tareas en las pasadas de detalle» |
| Mostrar el resumen a los ~20 s y añadir los detalles después | — | ⏳ | | [historial](historial.md), punto 3 |

## Enhance · modelo

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Q8_0 (1,85 GB) frente a Q4_K_M, las 4 reuniones, Auto y plantilla | Reunión larga (17 min) mejor en todo: Auto con cobertura del 86 % al 96 %, tareas de 3/7 a 5/7 y cifras fieles del 91 % al 98 %; con plantilla, colocación del 83 % al 88 % y tareas de 3/7 a 4/7. Reuniones cortas mixtas: en el 1:1 empeora (colocación del 100 % al 77 %, «Feedback» relleno 3 de 3, tareas en Auto de 4/5 a 1/5); el stand-up y ventas, dentro del ruido. Tarda 1,35-2 × | ✅ adoptado (9 nov) | 9 nov | «Q8_0 frente a Q4_K_M», abajo |
| Modelo mayor, cuantizado en GGUF: Qwen3.5-9B Q4_K_M (4 bits, 5,7 GB) y Qwen3.5-4B Q8_0 (8 bits, 4,5 GB), frente a LFM3-1.7B Q8_0; sin razonamiento, reuniones de 60 min | Mismas tareas acertadas (dentro del ruido), pero escribe la mitad (6-17 frente a 15-33). Cobertura: el 9B peor (54-82 %), el 4B igual o mejor en Auto (80-89 % frente a 70 % en la revisión). Tarda 2 × (4B, ~210 s) y 3-4 × (9B, 330-520 s; 2 min solo en leer el prompt de 20.000 tokens) | ❌ | 10 nov | «Techo con un modelo mayor» |
| QAD-Q4_0 o Q6_K de LFM3 | — | ⏳ | | [historial](historial.md), punto 3 |

## Enhance · tareas

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Pasada aparte solo para tareas, en JSON (quién/qué/cuándo) | 0-2 de 6 tareas, con frases copiadas | ❌ | 6 nov | [historial](historial.md), punto 3 |
| Tareas también en las pasadas de detalle (`## Tareas`), añadidas a «Próximos pasos» sin duplicados (`detail_tasks`) | 1-3 tareas acertadas más, pero escribe el doble (29-41 en una reunión de 14), muchas falsas o atribuidas a «Yo» | ⚗️ `detail_tasks` | 9 nov | «Tareas en las pasadas de detalle» |
| Revisión a mano de las tareas «fallidas» de LFM3 (reuniones de 60 min, 12 ejecuciones) | El evaluador es justo: 43 % acertadas; 20 % están en las notas con dueño y acción, pero como dato en otro apartado; ~28 % aparecen sin dueño o con otro; 8 % no aparecen | ✅ medido | 10 nov | «Revisión de las tareas fallidas» |
| Encargos de otros apartados repetidos en «Próximos pasos», y cada tarea con el nombre delante (prompt de Auto y de plantilla) | Tareas acertadas 73 → 68 de 168 (ruido); siguen 36 como dato en otro apartado. Revisión en Auto: tareas 22 → 14, cobertura 70 → 80-84 %. Lo sigue a medias: entran algunas («Tomás arregla el fallo de Lince») y también temas sin dueño («Becaria de datos: 6 meses…») | ❌ | 10 nov | «Revisión de las tareas fallidas» |
| Regla que copia a «Próximos pasos» los encargos con dueño de otros apartados (`promote_commitments`, solo es/en): nombre de la transcripción + «to»/«will»/futuro, o verbo en 3.ª persona con plazo | Reuniones de 60 min: tareas acertadas 73 → 81 de 168, escritas 260 → 289; cobertura y colocación idénticas. Entrevista en Auto 7 → 11 de 24. 1:1, ventas y stand-up, mismas tareas (0-2 escritas más) | ✅ | 10 nov | «Revisión de las tareas fallidas» |

## Enhance · plantillas

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Encaje determinista de títulos (`fit_to_template`) y relleno «No se habló de esto» | Los títulos son siempre los de la plantilla | ✅ | 8 nov | [plantillas.md](plantillas.md) §4.2 |
| `## Otros temas` opcional como salida en el prompt + «no fuerces el contenido» | Colocación de la semanal del 71 % al 87 % | ✅ | 8 nov | «Plantillas: iteraciones», iteración 2 |
| Comparar cifras normalizadas al colocar («112k» = «112.000») | Arregla los fallos de colocación por cifras | ✅ | 8 nov | «Plantillas: iteraciones», iteración 2 |
| Tope de tokens propio (`template_budget`: Auto + 150 por apartado, mínimo 800) | El 1:1 dejaba de cortarse antes de «Próximos pasos» (tareas de 1/5 a 4/5) | ✅ | 8 nov | «Plantillas: iteraciones», iteración 3 |
| Instrucción explícita «cada tarea va solo en `## Próximos pasos`» | Hunde las tareas de la semanal (3/0/0 de 7 frente a 3/5/3) | ❌ | 8 nov | «Plantillas: iteraciones», iteración 3 |
| Quitar viñetas repetidas entre apartados (`drop_cross_duplicates`) | Menos repeticiones; la tarea repetida se queda en el apartado de tareas | ✅ | 8 nov | «Plantillas: iteraciones», iteraciones 3 y 4 |
| Quitar `**` dentro de las viñetas | Limpieza | ✅ | 8 nov | «Plantillas: iteraciones», iteración 3 |
| Descripciones más concretas («solo…», «si nadie dio ninguna, va vacío») | «Feedback» del 1:1 vacío en 2 de 3 semillas (antes 0 de 3); el stand-up no mejora (36 %) | ✅ | 8 nov | «Plantillas: iteraciones», iteración 4 |
| Gramática GBNF sobre Markdown (títulos fijos y citas obligatorias) | Salida limpia, pero la colocación baja (67 % → 58 %) y tarda ~9 s más | ⚗️ `markdown_grammar` | 9 nov | «Plantillas: iteraciones», iteración 5 |
| Stand-up: «Hecho desde la última» → «Terminado» | Colocación del 36 % al 67 %; «Otros temas» del 25-29 % al 0-13 % | ✅ | 9 nov | «Plantillas: iteraciones», iteración 5 |
| Ventas: «Contexto y necesidades · Objeciones» → «Situación del cliente · Dudas y reparos» | Colocación del 71 % al 93 %; copiadas del 52-58 % al 26-33 % | ✅ | 9 nov | «Plantillas: iteraciones», iteración 6 |
| Ventas: «Situación y problemas» / «Dudas sobre la oferta» (y sus cruces) | 93-100 %, pero con más copiadas o alguna tarea perdida | ❌ | 9 nov | «Plantillas: iteraciones», iteración 6 |
| Ventas: descripción con «un competidor más barato» | 100 %, pero calcada del guion (sobreajuste) | ❌ | 9 nov | «Plantillas: iteraciones», iteración 6 |
| Semanal: «Cifras y resultados · Qué se decidió» o «Cifras de la semana · Acuerdos» | 65-92 % y 80-86 % frente a 78-83 %: dentro del ruido | ❌ | 9 nov | «Plantillas: iteraciones», iteración 6 |

**Lecciones que se repiten:**
- El título pesa más que la descripción.
- Las instrucciones extra en el prompt suelen empeorar otra métrica: revisa siempre tareas y cobertura, no solo la métrica que buscas.
- El modelo es muy sensible a cambios pequeños en la entrada.

**Sin resolver:**
- Detectar el desajuste: una plantilla que no encaja con la reunión no dispara reintento ni caída a Auto.
- Viñetas copiadas en el stand-up (47-65 %).
- Tiempo con plantilla: el resumen escribe ~1700 tokens en la reunión larga y es ahora casi todo el tiempo (~40 s).
- ~~Medir la plantilla de entrevista con cliente~~: hay guion desde el 9 nov (`entrevista_cliente`, 31 min; colocación 57-86 %).

## STT

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Parakeet TDT 0.6B v3 (sherpa-onnx, CPU) como segunda opinión de Qwen3-ASR | Con cifras normalizadas comete un 25 % menos de errores (WER 5,9 frente a 7,9 %) y marca el 79 % de los errores de Qwen3-ASR, pero va a 1-3 × tiempo real en CPU | ❌ retirado | 9 nov | «STT: Parakeet v3» |
| Qwen3-ASR 0.6B y 1.7B, en 4 y 8 bits | 1.7B de 4 bits: WER 7,9 → 6,3 % y el único que acierta «Vitoria»; el de 8 bits no se distingue y pesa 0,9 GB más | ✅ 1.7B 4 bits | 9 nov | «STT: Qwen3-ASR» |
| 1.7B en directo, solo y con el enhance a la vez | Finales a ~3 s de mediana (0.6B: 0,75 s); con el enhance en la GPU, los dos se quedan más de un minuto atrás | medido | 9 nov | «En directo» |
| Revisar con el 1.7B solo las frases donde el 0.6B duda (confianza por token) | Con umbral 0,3, un tercio de la mejora revisando el 19 % del audio | medido, sin adoptar | 9 nov | «Revisar con el 1.7B solo lo que el 0.6B duda» |
| Contexto de la reunión en el prompt de Qwen3-ASR | Con los términos de la reunión, WER 6,3 → 3,7 % y nombres 16 → 28 de 30; un contexto equivocado apenas estorba | ✅ | 9 nov | «Contexto en el prompt de Qwen3-ASR» |
| 0.6B para los parciales y 1.7B para los finales | — | ⏳ | | «En directo» |

## Audio y detección

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Detección: `kAudioStreamPropertyIsActive` para saber si una app graba | Vale siempre 1 | ❌ | 8 nov | [historial](historial.md), fase 2 |
| Detección: `AVCaptureDevice.isInUseByAnotherApplication` | Falso con Chrome grabando | ❌ | 8 nov | [historial](historial.md), fase 2 |
| Fin de llamada tras 5 s sin micrófono | Para la grabación al silenciarse; pasa a 15 s (`END_WAIT`) | ❌ → 15 s ✅ | 8 nov | [historial](historial.md), fase 2 |
| Idioma «Detectar» frase a frase en Qwen3-ASR | Salta de idioma en frases cortas; se fija con `LanguageLock` (8 s y 60 %) | ✅ | 7 nov | [historial](historial.md), punto 4 |
| Cancelación de eco con LocalVQE (GGML, Apache-2.0; mic + `system.wav` como referencia) | — | ⏳ | | [historial](historial.md), punto 1 (AEC pendiente) |

# Detalle

## Q8_0 frente a Q4_K_M (9 nov)

`LiquidAI/LFM3-1.7B-GGUF`, código de `3c58b52`, 3 semillas, mediana. No existe «Q8_K_M»: las cuantizaciones publicadas son Q4_K_M, QAD-Q4_0, Q5_K_M, Q6_K, Q8_0, NVFP4, F16 y BF16.

| Reunión · modo | Cobertura | Tareas | Colocación | Cifras fieles | Copiadas | Tiempo |
|---|---|---|---|---|---|---|
| 1:1 · plantilla | 100 → 100 % | 4/5 → 5/5 | 100 → **77 %** | 100 → 100 % | 16-50 → 6-21 % | 20 → 32 s |
| 1:1 · Auto | 100 → 100 % | 4/5 → **1/5** | — | ~100 % | 14-28 → 21-25 % | 20 → 20 s |
| Ventas · plantilla | 78 → 78 % | 4/4 → 4/4 | 93 → 100 % | 100 → 100 % | 26-33 → 21-45 % | 21 → 26 s |
| Ventas · Auto | 83 → 83 % | 1/4 → 1/4 | — | 92 → 100 % | 27-43 → 11-28 % | 21 → 24 s |
| Stand-up · plantilla | 83 → 92 % | 4/4 → 4/4 | 67 → 73 % | 100 → 100 % | 53-65 → 52-68 % | 15 → 29 s |
| Stand-up · Auto | 100 → 100 % | 2/4 → 1/4 | — | 100 → 100 % | 26-42 → 42-47 % | 19 → 20 s |
| Semanal · plantilla | 86 → 93 % | 3/7 → 4/7 | 83 → 88 % | 92 → 97 % | 17-22 → 8-15 % | 53 → 72 s |
| Semanal · Auto | 86 → **96 %** | 3/7 → **5/7** | — | 91 → 98 % | 9-14 → 4-15 % | 53 → 67 s |

Decisión del usuario (9 nov): se adopta Q8_0 en el catálogo (`models.toml`, carpeta `llm/lfm3-1.7b-q8`), por la mejora en la reunión larga. Las mediciones anteriores de este registro son con Q4_K_M. Además, con Q8_0 «Feedback» del 1:1 se rellena en las 3 semillas (con Q4_K_M, 1 de 3). Archivo descargado en `<datos>/models/llm/lfm3-1.7b-q8/` (SHA-256 `8f6b2418…20b6`). Para medir con él: `JUANOLA_GGUF=<ruta> eval_enhance …`.

## Techo con un modelo mayor (10 nov) ❌

Pregunta: ¿las tareas (5-9 de 14 en las reuniones de una hora) están limitadas por el tamaño del modelo? Mismo pipeline y prompts, sin `detail_tasks`, 3 semillas; Qwen con la plantilla ChatML y `<think>` vacío (se elige por `general.architecture`).

Versiones, todas cuantizadas en GGUF:

- **LFM3-1.7B Q8_0** (8 bits, 1,85 GB): el actual, la referencia.
- **Qwen3.5-4B Q8_0** (8 bits, 4,5 GB), de `unsloth/Qwen3.5-4B-GGUF`.
- **Qwen3.5-9B Q4_K_M** (4 bits, 5,7 GB), de `unsloth/Qwen3.5-9B-GGUF`. El 9B en Q8_0 (~9,5 GB) no se probó: el 4B en 8 bits ya cubre la comparación sin pérdida de cuantización.

Tareas acertadas · escritas · cobertura · segundos:

| Guion · modo | LFM3-1.7B Q8_0 | Qwen3.5-4B Q8_0 | Qwen3.5-9B Q4_K_M |
|---|---|---|---|
| revision_trimestral (es) · plantilla | 5/5/7 · 17-19 · 89-91 % · 122-137 | 10/6/5 · 7-9 · 75-86 % · 207-222 | 5/6/6 · 6-7 · 68-82 % · 362-418 |
| revision_trimestral · Auto | 6/9/7 · 15-18 · 70 % · 99-115 | 6/5/3 · 4-7 · 80-89 % · 215-223 | 7/7/7 · 9-10 · 70-82 % · 359-521 |
| weekly_planning (en) · plantilla | 3/8/9 · 30-33 · 70-87 % · 108-112 | 5/5/6 · 8 · 70-78 % · 206-213 | 5/6/4 · 12-15 · 72-76 % · 326-422 |
| weekly_planning · Auto | 2/7/5 · 18-26 · 61-63 % · 90-103 | 5/6/2 · 2-8 · 59-85 % · 194-214 | 6/6/5 · 13-17 · 54-67 % · 332-339 |

- **Tareas:** ninguno acierta claramente más. Los Qwen son más precisos (casi todo lo que escriben es una tarea real) y más estables entre semillas; LFM3 escribe el doble y acierta lo mismo.
- **Cobertura:** el 9B resume más y lista menos datos; el 4B empata con plantilla y gana en Auto.
- **Colocación:** igual (85-96 %). Cifras fieles igual (la weekly da ~0 % en todos los modelos: es cosa del guion).
- **Tiempo:** el 4B tarda ~2 × lo de LFM3 y el 9B 3-4 ×; al 9B, leer el prompt del resumen (20.550 tokens) le cuesta 116-126 s.

Decisión: se queda LFM3. El techo de las tareas no es el tamaño del modelo; está en cómo se piden o en cómo se puntúan (las «acertadas» exigen casar con el guion). El soporte de Qwen en llama.cpp (`qwen_prompt`) queda por si hace falta otro modelo.

## Generación en lote (9 nov)

Las pasadas de detalle no dependen del resumen: solo se juntan al final. `ChatModel::complete_many` las genera junto con el resumen. En llama.cpp van en un contexto con una secuencia por petición (`n_seq_max`, caché KV unificada): el prefill va secuencia a secuencia y cada paso de decodificación lleva un token de cada secuencia viva, que cuesta casi lo mismo que uno solo. Con plantilla, los detalles salen en el lote del primer intento y los reutilizan el reintento y la caída a Auto. `JUANOLA_LLM_NO_BATCH=1` vuelve a generarlas una a una para comparar.

Desglose antes del cambio (semanal en Auto, Q8_0, con la GPU compartida con otro proceso): resumen 9,8 s de prefill + 32,6 s de escritura (971 tokens); cada pasada de detalle, ~2 s + ~11-13 s (360-450 tokens). Escribir era el 82 % del tiempo.

| Q8_0, mediana de 3 semillas | Una a una | En lote |
|---|---|---|
| 1:1 · plantilla / Auto | 33 / 20 s | 21 / 13 s |
| Ventas · plantilla / Auto | 26 / 24 s | 17 / 15 s |
| Stand-up · plantilla / Auto | 29 / 20 s | 19 / 11 s |
| Semanal · plantilla / Auto | 72 / 67 s | 40 / 31 s |

Medido con la máquina cargada (OrbStack y otros procesos usando CPU y GPU): los tiempos absolutos varían, pero la comparación alternando los dos modos daba 63 → 37 s.

## Límite de citas (9 nov) ✅

El modelo enumeraba líneas seguidas dentro de una cita (`[109, 110, …, 130]`, hasta 410 números) y gastaba así el tope de tokens. Ahora, si una cita abierta ya tiene 6 referencias y el siguiente token trae una coma, el backend de llama.cpp mete `]\n` en su lugar: cierra la cita y la línea (si solo cerrara el corchete, el modelo seguiría con «, 120, 121…» fuera). No cuesta tiempo, a diferencia de una gramática (+9 s con la de Markdown). Se aplica al resumen y a las pasadas de detalle (`MAX_REFS = 6` en el enhance).

Medido en las cuatro reuniones que tenían citas largas, 3 semillas, antes → después (GPU compartida: los tiempos son ruidosos):

| Guion · modo | Cobertura | Tareas por semilla | Colocación | Tiempo | Tandas en el tope |
|---|---|---|---|---|---|
| point_individuel (fr) · plantilla | 55-76 → 62-72 % | 3/3/4 → 4/3/4 de 8 | 88-100 → 90-100 % | 84-102 → 64-70 s | 0 → 0 |
| point_individuel (fr) · Auto | 62-76 → 62-76 % | 1/1/5 → 1/1/5 | — | 54-128 → 58-69 s | 0 → 0 |
| team_meeting (de) · plantilla | 74-81 → 70-89 % | 3/2/5 → 7/2/5 de 8 | 50-86 → 47-83 % | 66-127 → 69-144 s | 1 → 1 |
| team_meeting (de) · Auto | 85-93 → 78-96 % | 6/6/2 → 6/7/2 | — | 61-67 → 84-137 s | 0 → 0 |
| revision_trimestral (es) · plantilla | 55-86 → 55-91 % | 1/4/1 → 1/5/4 de 14 | 71-84 → 68-87 % | 170-196 → 143-162 s | 1 → 0 |
| revision_trimestral (es) · Auto | 70-84 → 64-70 % | 6/4/4 → 6/5/5 | — | 131-183 → 135-147 s | 1 → 0 |
| weekly_planning (en) · plantilla | 46-72 → 65-85 % | 2/2/6 → 3/4/8 de 14 | 62-88 → 88-93 % | 140-227 → 127-149 s | 1 → 0 |
| weekly_planning (en) · Auto | 41-65 → 54-74 % | 2/2/2 → 2/2/2 | — | 105-132 → 127-138 s | 1 → 0 |

- **Guiones Unicode en los rangos:** el modelo escribe a veces `[162‑166, 167, …` (U+2011). Ni el corte ni `split_refs` lo tomaban por cita (la weekly con plantilla seguía llegando al tope). Ahora aceptan ‐ ‑ ‒ – —; la fila de la weekly es la de después de ese arreglo.
- **El tope que queda** (team_meeting, plantilla) no son citas: con «elftausendfünfhundert» el modelo se pone a razonar en inglés dentro de las notas («Actually line 251 says…», «Let's re-read») hasta agotar el presupuesto. Pendiente.
- **Tareas en Auto** no mejoran (weekly 2/14 en las tres semillas): ahí no se pierden por falta de tokens. Siguen siendo lo más flojo.

## Reuniones largas y en seis idiomas (9 nov)

Siete guiones nuevos en `crates/enhance/fixtures/` (con `.facts.json` y `.notas.txt`), escritos para parecer reuniones reales: charla, interrupciones, digresiones, una decisión que se corrige a mitad y tareas repartidas sin repaso final completo.

| Guion | Idioma | Min | Hechos (tareas) | Plantilla |
|---|---|---|---|---|
| `revision_trimestral` | español | 62 | 58 (14) | semanal |
| `weekly_planning` | inglés | 64 | 60 (14) | semanal |
| `entrevista_cliente` | español (México) | 31 | 35 (8) | entrevista |
| `chamada_vendas` | portugués | 32 | 35 (8) | ventas |
| `point_individuel` | francés | 31 | 37 (8) | 1on1 (con feedback) |
| `team_meeting` | alemán | 32 | 35 (8) | semanal |
| `intervista_cliente` | italiano | 31 | 35 (8) | entrevista |

El evaluador detecta el idioma como la app (o `--lang`), pone los títulos de la plantilla en ese idioma, normaliza los diacríticos de los seis idiomas y tiene `--check` (sin modelo: hechos inalcanzables, apartados desconocidos y duración).

Resultados (Q8_0 en lote, 3 semillas, GPU compartida con otros procesos: los tiempos están inflados):

| Guion | Plantilla: cobertura · tareas · colocación | Auto: cobertura · tareas | Tiempo (plantilla / Auto) |
|---|---|---|---|
| revision_trimestral (es, 60) | 55-86 % · 1-4 de 14 · 71-84 % | 70-84 % · 4-6 de 14 | 170-196 / 131-183 s |
| weekly_planning (en, 60) | 46-72 % · 2-6 de 14 · 62-88 % | 41-65 % · 2 de 14 | 140-227 / 105-132 s |
| entrevista_cliente (es, 30) | 67-70 % · 2-5 de 8 · 67-84 % | 74-89 % · 1 de 8 | 70-83 / 73-84 s |
| chamada_vendas (pt, 30) | 81-96 % · 3-6 de 8 · 82-92 % | 85-89 % · 1 de 8 | 63-69 / 82-93 s |
| point_individuel (fr, 30) | 55-76 % · 3-4 de 8 · 88-100 % | 62-76 % · 1-5 de 8 | 84-102 / 54-128 s |
| team_meeting (de, 30) | 74-81 % · 2-5 de 8 · 50-86 % | 85-100 % · 2-6 de 8 | 66-126 / 61-67 s |
| intervista_cliente (it, 30) | 63-74 % · 3-5 de 8 · 70-82 % | 44-48 % · 2 de 8 | 67-77 / 55-58 s |

Hallazgos:

- **Fallos encontrados y corregidos:**
  - Panic en el ITN con un carácter de varios bytes antes de «mil» o «millones» (`≈180 mil`, también `€112 mil` en español). En la app tumbaba el enhance, porque el ITN se usa también al colocar viñetas en cualquier idioma. Hay test.
  - El ITN (solo sabe español) se aplicaba en todos los idiomas: en portugués «vinte e cinco» → «vinte e 5». Ahora solo se aplica si el resumen es en español.
- **Citas desbocadas:** el modelo escribe listas larguísimas de líneas (`[96, 97, 98, …, 113]`). 7 de 42 resúmenes llegaron al tope de 3000 tokens, incluidos dos de 30 min en alemán, y los apartados del final (las tareas) se pierden. Es la causa más probable de las pocas tareas en las reuniones de 60 min.
- **Tareas:** el punto más débil, sobre todo en Auto (1 de 8 en tres de las reuniones de 30 min) y a los 60 min (1-6 de 14).
- **Tiempo a los 60 min (130-230 s):** el prefill del resumen tarda ~26 s (18K tokens) y los de las 15 pasadas de detalle otros ~27 s, uno tras otro antes de decodificar. Escribir el resumen tarda 50-110 s.
- **Cifras fieles** sale 0-23 % fuera del español, pero es un artefacto de la métrica: sin ITN la transcripción tiene los números con letras y las viñetas, en cifras.
- **ITN:** no convierte «uno coma ocho» (el «uno» delante de «coma» no cuenta como número).

Siguientes experimentos, por orden: ~~limitar las citas~~ (hecho, ver «Límite de citas»); arreglar la métrica de cifras fuera del español; procesar las pasadas de detalle y el prefill del resumen durante la reunión; extraer tareas también en las pasadas de detalle.

## Tareas en las pasadas de detalle (9 nov) ⚗️ y tope de Auto ✅

Idea: cada pasada de detalle (4 min) ve su trozo con más atención que el resumen; si también lista las tareas, se recuperan las que el resumen omite. Solo castellano e inglés.

- **Lo primero que salió:** en Auto, el tope de tokens de reuniones de 4-5 min (~600) cortaba **todos** los resúmenes antes de «Próximos pasos» (el 1:1, ventas y stand-up acababan justo en el tope; ventas a mitad de frase). Con plantilla ya tenía mínimo 800. Ahora Auto tiene la base + 400, con mínimo 800. Era la causa de las tareas en Auto a 1/4 o 1/5 desde hacía días.
- **Pasadas de detalle:** con las tareas al final del prompt nunca llegaban a escribirse (la lista de datos agotaba los 600 tokens). Van primero (`## Tareas` y luego `## Datos`).
- **Cuántas entran:** sin límite, ~58 «tareas» por reunión de una hora («Yo: Ask Aisha and Sophie if they are there», «Ellos: Confirm present»). Con 2 por fragmento, filtro de preguntas, de «Ellos…», de verbos de la propia reunión (ask, tell, preguntar, explicar…) y duplicados lejanos (dos tercios de palabras en común), siguen siendo 29-41. Muchas llevan «Yo:» aunque sean de otro.
- **El evaluador** ahora imprime `tareas escritas: N (en el guion: M)`: las acertadas solas no detectan la basura.

Tareas acertadas / escritas, 3 semillas, ambos con el tope nuevo (`+tasks` frente a sin él):

| Guion · modo | Con tareas en detalle | Sin ellas (por defecto) |
|---|---|---|
| entrevista_cliente (es, 30) · plantilla | 6/6/6 de 8 · 15-17 escritas | 6/5/5 · 8-9 |
| entrevista_cliente · Auto | 6/5/5 · 13-15 | 2/2/3 · 4-7 |
| revision_trimestral (es, 60) · plantilla | 8/8/7 de 14 · 36-40 | 5/5/7 · 17-19 |
| revision_trimestral · Auto | 8/5/7 · 29-37 | 6/9/7 · 15-18 |
| weekly_planning (en, 60) · plantilla | 6/9/8 de 14 · 37-41 | 3/8/9 · 30-33 |
| weekly_planning · Auto | 4/6/6 · 32-34 | 2/7/5 · 18-26 |

Cobertura y colocación, parecidas en los dos. Con el tope nuevo y sin tareas en detalle, frente a la mañana (tras el límite de citas): revisión trimestral con plantilla 1/5/4 → 5/5/7 de 14, Auto 6/5/5 → 6/9/7; weekly Auto 2/2/2 → 2/7/5; entrevista con plantilla 2-5 → 5-6 de 8. Las reuniones cortas se midieron con las tareas en detalle activadas, pero ahí añadían 0-2 por ejecución: la mejora es del tope.

Decisión: el tope se queda; las tareas en detalle quedan desactivadas (`detail_tasks`, `--variant x=itn+tasks` en el evaluador). Para reactivarlas haría falta filtrar con el modelo (¿es un compromiso?, ¿de quién?) o que el resumen elija entre las candidatas. Las de la weekly con plantilla siguen siendo muchas (30-33) aun sin ellas: es el propio resumen en inglés.

## Revisión de las tareas fallidas (10 nov)

Tras el techo con modelos mayores: ¿las tareas fallan de verdad o el evaluador es demasiado estricto? Una tarea cuenta como acertada si una viñeta marcada como tarea contiene al dueño y la acción (`facts.json`). Notas de LFM3-1.7B Q8_0 guardadas de las 12 ejecuciones (revisión trimestral y weekly, plantilla y Auto, 3 semillas) y cruzadas con las 14 tareas de cada guion (`audit_cat.py` en el scratchpad).

| Qué pasa con la tarea | Veces (de 168) |
|---|---|
| Acertada | 73 (43 %) |
| En las notas con dueño y acción, pero como dato en otro apartado («Nuria envía correo explicativo antes del viernes» dentro del descuento; «Hannah to prepare list of 200 accountants…» en ventas) | 34 (20 %) |
| La acción aparece, sin dueño o con otro («El cuestionario de seguridad se enviará para el lunes 17»; «Update revenue model… by Thursday» sin Aisha; la cena «reservada por Yo y Carla», que es de Raúl) | ~47 (28 %; casado solo por la acción, es una cota alta) |
| No aparece | 14 (8 %) |

- El evaluador es justo: pide el dueño y que esté en «Próximos pasos», que es lo que el usuario espera de ese apartado.
- El modelo casi nunca se olvida del encargo; falla en **dónde lo pone** y en **de quién es**. Con lo primero resuelto, rondaría el 64 %.
- Entre las 260 escritas como tarea sigue habiendo decisiones y estados («Portugal: aplazar versión…», «Feria stand: decidido no hacer stand»).

**Probado: pedirlo en el prompt ❌.** En Auto, «Próximos pasos» con «todos los encargos, también los que ya aparecen en otros apartados, empezando por el nombre de quien lo hará»; con plantilla, una excepción a «cada idea solo en su apartado». Mismas 12 ejecuciones:

| Guion · modo | Acertadas antes → después (de 42) | Como dato con dueño | Escritas | Cobertura |
|---|---|---|---|---|
| revision_trimestral · plantilla | 17 → 18 | 12 → 7 | 54 → 56 | 89-91 → 80-93 % |
| revision_trimestral · Auto | 22 → 14 | 8 → 8 | 48 → 38 | 70 → 80-84 % |
| weekly_planning · plantilla | 20 → 22 | 8 → 10 | 93 → 80 | 70-87 → 80-87 % |
| weekly_planning · Auto | 14 → 14 | 6 → 11 | 65 → 63 | 61-63 → 61-83 % |

El 1.7B no sigue la instrucción con fiabilidad: mete temas enteros con el nombre en medio («Postmortem caída: … Álvaro debe explicar en la reunión del 18») y en Auto deja fuera otras. Revertido.

**Adoptado: una regla sin el modelo ✅ (`promote_commitments`).** Tras resolver los detalles, cada viñeta de un apartado que no es de tareas se parte en cláusulas («;», « – », punto). Una cláusula se copia a «Próximos pasos» (desde el nombre; el original se queda) si un nombre propio de la transcripción (con mayúscula y no al principio de frase; sin días ni meses en inglés) hace de sujeto: en inglés «X to <verbo>» o «X will…»; en castellano un futuro («Nuria enviará…»); o, si la cláusula tiene plazo («para el», «antes del», «by», un día…), una palabra en tercera persona detrás (-a/-e en castellano, -s en inglés). Los duplicados se descartan con `add_tasks`. Desactivable con `+nopromote` en el evaluador.

Tres rondas sobre las mismas 12 ejecuciones (acertadas / escritas, de 168):

| Versión | Acertadas | Escritas |
|---|---|---|
| Sin regla | 73 | 260 |
| 1.ª: nombre + verbo con plazo | 81 | 364 (días y meses como nombres: «Thursday will be walked…»; «Tierramar es el grupo…») |
| 2.ª: sin días ni meses, ni «es/se/porque/is…» | 81 | 321 (fragmentos: «Santander, arrocerías y tabernas», «Lince otra vez») |
| 3.ª: forma de verbo según el idioma, sin coma ni posesivo detrás del nombre | **81** | **289** |

Lo que entra de más casi siempre es un encargo real que el guion no cuenta («Priya to chase HMRC by end of month», «Hannah will apply credit this week», «Marcos te llama antes del 18»). Quedan pocos falsos («Pro accounts linked to Thursday incident»). Reuniones cortas, con y sin la regla (3 semillas): entrevista en Auto 2/2/3 → 4/4/3 de 8; en el resto las mismas tareas y 0-2 escritas más. La cobertura, la colocación y el tiempo no cambian.

## Plantillas: iteraciones (8-9 nov)

Movido desde [plantillas.md](plantillas.md) §6.4 el 10 nov. Primera evaluación de la fase 1, con LFM3 Q4_K_M (las cifras no son comparables con las de Q8_0).

`reunion_producto` convertida con `tools/script_to_segments.py` (142 segmentos, 17 min), LFM3-1.7B Q4_K_M, 3 semillas, pasadas de detalle activadas. Notas del usuario: `notas_usuario.txt` (planas) o `notas_usuario_plantillas.txt` (con títulos, lista, tarea y negrita). Mediana entre paréntesis.

| Configuración | Cobertura | Tareas | Cifras fieles | Copiadas | s |
|---|---|---|---|---|---|
| A. Auto, código anterior, notas planas | 89 / 79 / 93 % (89) | 2 / 2 / 4 (2) | 85-93 % | 4-12 % | ~40 |
| B. Auto, código nuevo, notas planas | 93 / 89 / 86 % (89) | 3 / 4 / 2 (3) | 90-97 % | 11-12 % | ~42 |
| C. Auto, código nuevo, notas con estructura | 79 / 93 / 86 % (86) | 3 / 2 / 3 (3) | 90-95 % | 9-14 % | ~39 |
| D. «Reunión semanal de equipo» | 89 / 89 / 89 % (89) | 2 / 1 / 4 (2) | 93-98 % | 12-15 % | ~42 |
| E. Desajuste: «Entrevista con cliente» | 75 / 82 / 82 % (82) | 5 / 2 / 4 (4) | 95-98 % | 25-28 % | ~52 |

Con plantilla (D): colocación 58 / 83 / 71 % (71), viñetas en «Otros temas» 14 / 0 / 4 %, 0 apartados vacíos, notas del usuario en su apartado 3 / 5 / 3 de 6, 0 notas perdidas, 0 reintentos, 0 caídas.

Frente al §6.3:

- **Cumple:** títulos = plantilla (por construcción); cobertura igual que Auto; tiempo ≤ 1,3 × Auto; 0 notas perdidas; las tareas del usuario siempre en «Próximos pasos»; Auto no empeora con el cambio de las notas del usuario (B frente a A).
- **No cumple:** colocación (71 % frente a ≥ 80 %); notas del usuario en su apartado (50 % frente a ≥ 90 %); tareas con responsable (2 frente a 3 en Auto, dentro del ruido entre semillas que ya se veía en Auto).
- **Desajuste (E): no se comporta como se esperaba.** El modelo no deja apartados vacíos ni usa «Otros temas»: mete la reunión de producto en los apartados de la entrevista. En «Citas destacables» copia frases literales (25-28 % de viñetas copiadas) y en «Próximos pasos» repite tareas. Ninguno de los tres criterios de fallo del §4.4 lo detecta, así que no hay reintento ni caída a Auto.

Lo que se ve en las salidas:

- El modelo escribe apartados largos que mezclan categorías: «Decisiones» recoge también los planes de precios, la feria y algunas tareas, y en «Próximos pasos» acaban hechos que no son tareas (como en Auto).
- Las notas del usuario se colocan junto a las viñetas de la IA que más se les parecen. Es lo correcto, pero hereda dónde las puso el modelo («bug citas duplicadas» va a «Decisiones», porque ahí puso el modelo el arreglo de Javier). Las etiquetas de `user_notes` en `facts.json` quizá sean demasiado estrictas.
- «MRR 112k» no encaja con «112000 euros» al buscar palabras en común, porque la colocación compara palabras sin pasar por el ITN.

**Segunda iteración (8 nov).** Dos cambios: el prompt añade un `## Otros temas` opcional al final de la estructura y pide no forzar el contenido en un apartado que no le corresponde (lo que el modelo pone ahí se queda en «Otros temas» y cuenta para el criterio de fallo); y la colocación compara cifras normalizadas («112k» = «112.000»).

| | Cobertura | Tareas | Colocación | Notas del usuario | Otros temas | Copiadas | s |
|---|---|---|---|---|---|---|---|
| D. Semanal | 86 / 89 / 89 % (89) | 3 / 6 / 3 (3) | 87 / 88 / 83 % (87) | 5 / 5 / 6 de 6 (83 %) | 0 % | 16-23 % | ~46 |
| E. Desajuste | 61 / 75 / 86 % (75) | 0 / 2 / 2 (2) | — | — | 0 / 7 / 25 % | 22-28 % | ~53 |

- Con la plantilla adecuada ya se cumplen la colocación (87 % ≥ 80 %), las tareas (igual que Auto), «Otros temas» (0 % ≤ 10 %), la cobertura y el tiempo (1,15 × Auto). Las notas del usuario se quedan cerca del umbral (83 % frente a 90 %); la que falla suele ser «contratar móvil: remoto», que va a «Otros temas» porque la IA puso la contratación en «Próximos pasos».
- Sube la proporción de viñetas copiadas (12-15 % → 16-23 %).
- **El desajuste sigue sin detectarse.** El modelo usa «Otros temas» solo a veces (hasta un 25 %) y sigue rellenando los apartados de la entrevista. Sin una señal nueva no hay reintento ni caída a Auto. Como la plantilla la elige el usuario, el daño es una nota mal organizada, no una nota perdida.

**Tercera iteración (8 nov): tres guiones nuevos.** `uno_a_uno.txt` (1:1, 5 min, sin feedback a propósito: `"empty": {"1on1": ["Feedback"]}`), `llamada_ventas.txt` (5,5 min) y `stand_up.txt` (4 min), cada uno con `.facts.json` (hechos con `section`, `user_notes` y `empty`) y `.notas.txt` (título, lista, tarea y negrita). Son más cortos que los 8-10 min previstos. El evaluador gana `--title`. Cambios en el pipeline:

- **Tope de tokens propio con plantilla** (`template_budget`: el de Auto + 150 por apartado, mínimo 800). Con el tope de Auto, el 1:1 se cortaba a mitad de «Feedback» y nunca llegaba a «Próximos pasos» (1 de 5 tareas).
- **Viñetas repetidas entre apartados** (`drop_cross_duplicates`): si dos viñetas de apartados distintos citan una línea en común y comparten la mitad de sus términos, se queda la del apartado que mejor encaja.
- **Sin negritas** `**` dentro de las viñetas.
- **Probado y descartado:** una instrucción explícita «cada tarea va solo en `## Próximos pasos`». Hundía las tareas de la semanal (3 / 0 / 0 de 7, frente a 3 / 5 / 3 sin ella). Las tareas perdidas del 1:1 eran por el tope de tokens.

Resultado final (3 semillas, mediana; Auto con el mismo guion entre paréntesis):

| Plantilla | Cobertura | Tareas | Colocación | Notas del usuario | Otros temas | Copiadas | Tiempo |
|---|---|---|---|---|---|---|---|
| 1:1 | 100 % (100 %) | 4/5 (4/5) | 86 % | 3 de 5 (60 %) | 0 % | 23-25 % | ~18 s (1,5 × Auto) |
| Llamada de ventas | 78 % (83 %) | 3/4 (1/4) | 71 % | 4 de 4 | 3-4 % | 52-59 % | ~19 s (1,3 ×) |
| Stand-up | 92 % (100 %) | 1/4 (2/4) | 55 % | 4 / 4 / 3 de 4 | 10-40 % | 44-62 % | ~20 s (1,7 ×) |
| Reunión semanal | 86 % (86-89 %) | 3/7 (3/7) | 79 % | 5 / 5 / 6 de 6 | 0 % | 17-22 % | ~46 s (1,15 ×) |

- **Bien:** con plantilla las tareas igualan o superan a Auto, salvo en el stand-up; en ventas pasan de 1 a 3 de 4. No se pierde ninguna nota del usuario y la cobertura queda dentro de los 5 puntos, salvo en el stand-up.
- **Colocación por debajo del 80 %** en ventas (los retrasos, los campos personalizados de SAP y los servidores en Europa acaban en «Contexto y necesidades» en vez de en «Objeciones»; las etiquetas son discutibles) y sobre todo en el **stand-up (55 %)**. Ahí el modelo no separa «hecho» de «plan de hoy», manda mucho a «Otros temas» y copia frases (hasta un 62 %).
- **«Feedback» del 1:1 se rellena en las 3 semillas** con viñetas que no son feedback («Diego pide un plan de migración…»). El deduplicado no lo evita, porque no son repeticiones.
- **El tiempo pasa de 1,3 × Auto** en las reuniones cortas (1:1 y stand-up), porque el modelo escribe más con el tope nuevo.

**Cuarta iteración (8 nov).** Descripciones más concretas en «Feedback» («solo opiniones explícitas sobre cómo trabaja alguien…; si nadie dio ninguna, va vacío») y en los tres apartados del stand-up («solo lo que ya terminó…, no lo que hará hoy», etc.). Además, el deduplicado deja una tarea repetida siempre en el apartado de tareas: antes se quedaba en «Hecho desde la última» y se perdía como tarea.

| Plantilla | Cobertura | Tareas | Colocación | Notas del usuario | Otros temas | Tiempo |
|---|---|---|---|---|---|---|
| 1:1 | 100 % (Auto 100 %) | 4/5 (Auto 4/5) | 100 % | 5 de 5 (mediana) | 0 % | ~20 s (1,7 × Auto) |
| Llamada de ventas | 78 % (83 %) | **4/4** (1/4) | 71 % | 4 de 4 | 3-4 % | ~20 s (1,3 ×) |
| Stand-up | 83 % (100 %) | **4/4** (2/4) | 36 % | 3 de 4 | 25-29 % | ~15 s (1,25 ×) |
| Reunión semanal | 86 % (86-89 %) | 3/7 (3/7) | 83 % | 5 / 5 / 6 de 6 | 0 % | ~50 s (1,25 ×) |

- «Feedback» del 1:1 queda vacío en 2 de 3 semillas (antes, 0 de 3).
- Con plantilla, las tareas igualan o superan a Auto en las cuatro.
- **El stand-up es el punto débil y parece un límite del modelo:** el propio LFM3 escribe lo ya terminado bajo `## Otros temas` y las tareas bajo «Hecho desde la última» (visto en la traza), aunque la descripción lo diga. El encaje no puede corregirlo sin entender el texto.

**Quinta iteración (9 nov): gramática GBNF sobre Markdown (variante «b») y título del stand-up.**

- `EnhanceOptions::markdown_grammar` (desactivada por defecto) genera una gramática que fija los títulos `## …` de la plantilla, en su orden, más `## Otros temas` opcional. Cada viñeta tiene texto libre y acaba obligatoriamente en sus citas `[n, m]`. `GenOptions` gana `grammar` (GBNF directa) y el evaluador la activa con `+gbnf`. La gramática funciona (salida limpia, todas las viñetas citadas), pero **no corrige la colocación**: el modelo sigue eligiendo mal el apartado y además tarda más.
- El problema del stand-up era el título: «Hecho desde la última» se leía como «un hecho». Con **«Terminado»** (y la descripción «…antes de esta reunión (ayer terminé…)») el modelo deja de mandar lo terminado a «Otros temas».

| Stand-up | Cobertura | Tareas | Colocación | Otros temas | Tiempo |
|---|---|---|---|---|---|
| «Hecho desde la última» (cuarta iteración) | 83 % | 4/4 | 36 % | 25-29 % | ~15 s |
| «Terminado» | 92 % | 4/4 | **67 %** (55 / 67 / 80) | 0-13 % | ~18 s |
| «Terminado» + gramática | 100 / 83 / 100 % | 4/4 | 58 % | 12-19 % | ~27 s |

Se adopta «Terminado» en la plantilla integrada (en, pt, fr, de, it: Done, Concluído, Terminé, Erledigt, Completato) y la gramática queda como opción experimental. El stand-up sigue por debajo del 80 % de colocación y con muchas viñetas copiadas (47-65 %).

**Sexta iteración (9 nov): repaso de títulos.** El evaluador dice ahora adónde va cada hecho mal colocado (`id → apartado`). Con eso:

- **Ventas:** «Objeciones» y «Contexto y necesidades» se intercambiaban. Las dudas sobre SAP, Europa y Ruteo360 iban a contexto y el 12 % de retrasos (un problema del cliente) a objeciones. Se probaron cuatro combinaciones como plantillas JSON. Se adopta **«Situación del cliente» · «Dudas y reparos»**, con descripciones nuevas («a qué se dedica el cliente, cómo trabaja hoy, qué le cuesta y qué objetivo tiene»; «lo que preocupa al cliente de la solución que se le ofrece: integración, datos, adopción por su equipo o la competencia»). Con una descripción calcada del guion («un competidor más barato») salía 100 %, pero se descarta por sobreajustada.
- **Reunión semanal:** «Temas tratados» atrae decisiones y cifras, y algunas acaban en «Próximos pasos». Con «Cifras y resultados» / «Qué se decidió» (65-92 %) o «Cifras de la semana» / «Acuerdos» (80-86 %) no hay mejora clara frente a 78-83 %: se queda como está.
- **1:1:** 86-100 %, sin cambios. **Entrevista con cliente:** no tiene guion, así que no se toca sin medir.

| Llamada de ventas | Cobertura | Tareas | Colocación | Otros temas | Copiadas | Tiempo |
|---|---|---|---|---|---|---|
| Contexto y necesidades · Objeciones | 78 % | 4/4 | 71 % (71 / 71 / 69) | 3-4 % | 52-58 % | ~17 s |
| **Situación del cliente · Dudas y reparos** | 78 % (78 / 89 / 78) | 4/4 | **93 %** (86 / 94 / 93) | 6-11 % | **26-33 %** | ~21 s |

Ventas pasa el umbral de colocación y copia la mitad. El fallo que queda es el competidor, que el modelo pone en contexto o en «Próximos pasos».

Siguientes pasos de la fase 1 que quedaron (9 nov), por orden:

1. ~~Salida explícita `## Otros temas`~~ (segunda iteración), ~~guiones nuevos~~ (tercera), ~~descripciones concretas~~ (cuarta) y ~~gramática GBNF~~ (quinta: no mejora).
2. ~~**Títulos**~~ (sexta: ventas pasa a 93 %; semanal sin mejora clara). Falta un guion de entrevista con cliente para medir esa plantilla.
3. **Viñetas copiadas** en reuniones cortas (47-65 % en el stand-up; ventas bajó a 26-33 % con los títulos nuevos): con plantilla se copia más que en Auto.
4. **Tiempo en reuniones cortas** (1,7 × Auto en el 1:1): bajar el mínimo de `template_budget` o el margen por apartado, comprobando que las tareas no vuelven a cortarse.
5. **Desajuste:** sigue sin detectarse.

## STT: Parakeet v3 como segunda opinión (9 nov) ❌ retirado

Retirado el mismo día: el backend y la dependencia de sherpa-onnx se quitaron para probar antes Qwen3-ASR 1.7B (sección siguiente). La alineación, el normalizador de cifras y el CER se quedan en `crosscheck.rs`.

Idea: Qwen3-ASR transcribe en vivo; al terminar, Parakeet TDT 0.6B v3 (int8, sherpa-onnx en CPU) repasa los mismos segmentos. Donde los dos coinciden se da por bueno; donde no, hay un candidato a error. Herramientas: `tools/script_to_audio.py` (guion → `mic.wav` + `system.wav` con `say` y `ref.json`), `crates/stt/src/crosscheck.rs` (alineación y normalizador de cifras) y `crates/stt/examples/crosscheck_session.rs` (borrado; lo sustituye `compare_transcripts.rs`).

Llamada de ventas sintética (4,3 min, voces de `say`, 33 segmentos de Qwen3-ASR). *Recall* = errores de Qwen3-ASR que caen en un tramo marcado; *precisión* = marcas que coinciden con un error real.

WER y CER sobre texto normalizado (sin tildes, mayúsculas ni puntuación). *Mejor combinado* = elegir en cada tramo marcado la versión correcta según la referencia: es el techo de cualquier regla de arbitraje.

| Comparación | WER / CER Qwen3-ASR | WER / CER Parakeet | WER / CER mejor combinado | Palabras marcadas | Recall | Precisión |
|---|---|---|---|---|---|---|
| Sin normalizar cifras | 8,9 / 4,5 % | 10,0 / 5,6 % | 4,7 / 2,0 % | 12,1 % | 79 % (33/42) | 62 % (34/55) |
| Cifras con letras = dígitos («treinta y cinco mil» = «35.000», «%» = «por ciento») | 7,9 / 2,5 % | **5,9 / 1,8 %** | **3,8 / 1,3 %** | 8,1 % | 79 % (30/38) | **74 %** (31/42) |

- El normalizador quita las marcas falsas por formato (13 de 55) y cambia el orden de los modelos: sin él parecía que Parakeet iba peor; con él, comete un 25 % menos de errores que Qwen3-ASR.
- Lo que no se detecta son errores que los dos cometen igual («Victoria» por «Vitoria», «se caja» por «encaja») y algún artefacto de la referencia («Ruteo360» junto).
- El CER es mucho más bajo que el WER (2,5 % frente a 7,9 % en Qwen3-ASR): la mayoría de errores son palabras casi bien escritas («haces» por «hacéis», «tardaríais» por «tardarías»). Los que importan son nombres propios y términos raros («un mazón», «Hugo Severia», «promedor Route o»).
- Velocidad sin resolver: entre 1× y 3× tiempo real en CPU (4 hilos), tanto con la máquina muy cargada (carga media 21-25) como con carga 10 (271 s para 4,3 min). No es solo la carga: hay que mirar el int8 de ONNX Runtime, CoreML o el modelo fp32. Agrupar segmentos en lotes de `decode_multiple_streams` lo hace más lento y cambia algo el texto (el relleno).
- `transcribe_session` dejaba de esperar a los 30 s sin eventos y perdía la mayor parte de la reunión al volcar el audio de golpe: arreglado (600 s).
- Siguiente: decidir qué versión gana en cada tramo marcado (Parakeet acierta más, pero no siempre) y probar con audio real, que el TTS es demasiado limpio.

## STT: Qwen3-ASR 0.6B y 1.7B, en 4 y 8 bits (9 nov) ✅ 1.7B de 4 bits adoptado

Adoptado en `models.toml` el 9 nov (Mac con 16 GB o más; el 0.6B de 4 bits queda de respaldo con menos RAM). Primero se adoptó el de 8 bits por sus notas, pero se bajó al de 4: la diferencia de WER (3 palabras de 761) y de notas (6-11 puntos) está dentro del ruido, y el de 4 bits va un 20 % más rápido, ocupa 0,9 GB menos y es el único que acierta «Vitoria». Existen también de 5 bits (1,82 GB) y 6 bits (2,03 GB), sin medir. Sin medir todavía en directo con dos canales.

Modelos `mlx-community/Qwen3-ASR-{0.6B,1.7B}-{4bit,8bit}`: mismo tokenizador, y `qwen3_mlx.rs` los carga todos sin cambios. Misma llamada de ventas sintética (4,3 min, 761 palabras) y mismos 33 segmentos del VAD. Herramientas: `transcribe_session` con `JUANOLA_ASR_MODEL` y `crates/stt/examples/compare_transcripts.rs`. WER / CER con el normalizador de cifras. Tiempo con `transcribe_session` a toda velocidad y la máquina libre (carga media ~5); la salida es idéntica a la de las pasadas con la máquina cargada.

| Modelo | Disco | Total | mic (voz Mónica) | system (voces Eddy y Flo) | Tiempo (4,3 min) |
|---|---|---|---|---|---|
| 0.6B 4 bits (el actual) | 0,7 GB | 7,9 / 2,5 % | 5,4 / 1,1 % | 10,7 / 4,1 % | 17,8 s (15×) |
| 0.6B 8 bits | 1,0 GB | 6,7 / 2,0 % | 5,2 / 1,0 % | 8,5 / 3,2 % | 20,1 s (13×) |
| 1.7B 4 bits | 1,6 GB | 6,3 / 2,0 % | **2,7 / 0,5 %** | 10,5 / 3,7 % | 27,6 s (9×) |
| 1.7B 8 bits | 2,5 GB | **5,9 / 1,9 %** | 3,9 / 0,7 % | **8,2 / 3,4 %** | 34,2 s (8×) |

- Pasar a 8 bits mejora sobre todo el canal `system`, el de las voces sintéticas peores, en los dos tamaños. Pasar a 1.7B mejora sobre todo el micrófono.
- Con una sola reunión de 761 palabras, cada error es un 0,13 %: la diferencia entre el 1.7B de 4 bits y el de 8 bits (3 palabras) está dentro del ruido. Lo que sí es claro es que los tres superan al actual.
- Comparando el 0.6B con el 1.7B: el 1.7B arregla «Frakfurt», «propogo», «Raoul», «Victoria» → «Vitoria», «un mazón» → «almacén» y «promedor Route o» → «proveedor, Ruteo»; estropea «Bilbao» → «Limbao» y «SAP» → «sal».
- Todos van muy por encima del tiempo real en lote. Falta medir el directo: dos canales y parciales cada segundo, con el enhance compartiendo la GPU.
- Parte de los errores de `system` puede venir del TTS y no del STT: hay que confirmarlo con audio real.

### Notas generadas con cada transcripción (9 nov)

`eval_enhance` (LFM3-1.7B Q8_0, en lote, 3 semillas) sobre la transcripción de cada modelo, con las notas del usuario de la llamada de ventas. La referencia es el texto exacto del guion. La cobertura de `eval_enhance` se mide sobre los hechos *alcanzables* (los que están en la transcripción); para comparar modelos de STT aquí se da sobre los 18 hechos que alcanza la referencia. Script: `notas_stt.sh` en el scratchpad de la sesión.

| Transcripción | Hechos alcanzables | Plantilla: cobertura · tareas · copiadas | Auto: cobertura · tareas · copiadas |
|---|---|---|---|
| Referencia (texto perfecto) | 18 | 87 % · 4/4 · 12-29 % | 87 % · 1-4/4 · 9-13 % |
| 0.6B 4 bits (actual) | 17 | 89 % · 4/4 · 32-48 % | 72 % · 1/4 · 15-30 % |
| 0.6B 8 bits | 17 | 78 % · 3/4 · 23-42 % | 70 % · 1/4 · 29-35 % |
| 1.7B 4 bits | 18 | 87 % · 4/4 · 48-61 % | 80 % · 1/4 · 31-38 % |
| 1.7B 8 bits | 17 | **93 %** · 4/4 · 33-45 % | **91 %** · 1-4/4 · 7-20 % |

- El hecho que pierden los 0.6B y el 1.7B de 8 bits es «empezar por Vitoria»: escriben «Victoria». Solo el 1.7B de 4 bits acierta el nombre.
- El 1.7B de 8 bits da las mejores notas en los dos modos; en Auto, 19 puntos más que el actual, por encima del ruido de ±10.
- Pero las notas con el texto perfecto salen peor que con el 1.7B de 8 bits, y el 0.6B de 8 bits sale peor que el de 4 bits aunque transcribe mejor. El LLM es muy sensible a cambios pequeños de la entrada, así que con una sola reunión no se puede atribuir la diferencia al STT. Hace falta repetirlo en más reuniones.
- Las viñetas copiadas suben con cualquier transcripción de STT frente a la referencia (sin explicar).

### En directo (9 nov)

`crates/stt/examples/simulate_stream.rs`, ampliado a dos canales: la llamada de ventas sintética a ritmo real, con los dos canales a la vez y los parciales cada segundo, como en la app. El retraso es el tiempo desde que acaba un tramo de audio hasta que llega su texto. La máquina estaba cargada (carga media ~20 en las dos repeticiones, que se hicieron seguidas para que fueran comparables). La primera pasada del 1.7B, con carga 44, dio cifras parecidas a la repetición.

| Retraso del texto | 0.6B 4 bits | 1.7B 4 bits | 0.6B + enhance a la vez | 1.7B + enhance a la vez |
|---|---|---|---|---|
| Parciales: mediana / p90 / máx | **0,22 / 0,55 / 2,2 s** (235) | 1,13 / 2,95 / 11,2 s (152) | 6,7 / 24 / 48 s (18) | 6,3 / 28 / 74 s (21) |
| Finales: mediana / p90 / máx | **0,75 / 1,38 / 3,0 s** | 3,37 / 10,6 / 14,6 s | 45 / 70 / 86 s | 41 / 126 / 149 s |
| Decodificación media por resultado | 0,29 s | 1,12 s | 4,0 s | 5,1 s |

Entre paréntesis, el número de parciales que llegan: cuanto más lenta la decodificación, más parciales se saltan.

- **El 1.7B cuadruplica el coste de cada decodificación** en directo. El texto final llega unos 3 s tarde de mediana y hasta 15 s en las frases largas, frente a menos de 1 s y como mucho 3 s con el 0.6B. No se queda atrás: acaba al ritmo del audio.
- **Con el enhance usando la GPU a la vez, los dos modelos se quedan atrás**: más de un minuto de retraso, con cada decodificación cinco veces más lenta. Así que adelantar el resumen durante la reunión no se puede hacer tal cual: el enhance tendría que ceder la GPU, o esperar a los silencios.
- La mayor parte del coste en directo son los parciales (se vuelve a transcribir la frase entera cada segundo). Idea sin probar: el 0.6B para los parciales y el 1.7B solo para el texto final de cada frase.

### Revisar con el 1.7B solo lo que el 0.6B duda (9 nov)

El worker emite ahora la probabilidad de cada token elegido (`exp(máx − logsumexp)` de los logits que ya calcula el `argmax`) y su texto: `confidence` (la mínima) en cada segmento y `token_probs`/`token_texts` en los finales. La confianza de una palabra es la mínima de sus tokens con letras o cifras (la puntuación no cuenta). Análisis: `umbral.py` en el scratchpad, sobre la llamada de ventas sintética.

Como detector de errores del 0.6B (777 palabras, 73 con error):

| Umbral de la palabra | Palabras marcadas | Precisión | Errores cubiertos |
|---|---|---|---|
| < 0,3 | 1 % | 86 % | 8 % |
| < 0,5 | 4 % | 52 % | 22 % |
| < 0,7 | 11 % | 37 % | 42 % |
| < 0,9 | 22 % | 31 % | 73 % |

Los errores graves salen con confianza baja («un mazón» 0,17, «Frakfurt» 0,22, «dausan» 0,22, «asusais» 0,33, «propogo» 0,34, «Hugo» 0,34). Los que no se ven son los que el modelo da por seguros: «Victoria» por «Vitoria» (0,86-0,88), cifras («el 12», «del 5 %») y palabras cortas.

Revisando la frase entera con el 1.7B si alguna palabra baja del umbral:

| Umbral | Frases revisadas | Audio revisado | WER / CER |
|---|---|---|---|
| ninguno (0.6B) | 0/33 | 0 % | 7,9 / 2,5 % |
| < 0,3 | 7/33 | 19 % | 7,4 / 2,2 % |
| < 0,5 | 22/33 | 78 % | 6,6 / 2,1 % |
| < 0,8 | 30/33 | 97 % | 6,3 / 2,0 % |
| todo (1.7B) | 33/33 | 100 % | 6,3 / 2,0 % |

- Por frase ahorra poco: las frases duran ~8 s y casi todas tienen alguna palabra dudosa. Con un umbral de 0,3 se obtiene un tercio de la mejora revisando un 19 % del audio.
- Revisar solo la palabra dudosa necesitaría sus tiempos (Qwen3-ForcedAligner, que `mlxrs` ya trae) para recortar un trozo corto de audio.
- En directo, lo caro son los parciales, no los finales. Usar el 0.6B para los parciales y el 1.7B para *todos* los finales daría la calidad del 1.7B por menos coste que el 1.7B en todo; el umbral solo compensa si la GPU va justa (por ejemplo, con el enhance a la vez). Sin probar en directo.
- La confianza por palabra también sirve sola: subrayar en la transcripción las palabras por debajo de 0,5 (4 % del texto, la mitad son errores de verdad).

### Contexto en el prompt de Qwen3-ASR (9 nov) ✅ adoptado

El mensaje de sistema del prompt iba vacío; la implementación de referencia mete ahí su parámetro `context`. Ahora `Qwen3AsrMlx::context` lo rellena: `--context <texto>` en el worker, `WorkerConfig::context` en el cliente y `JUANOLA_ASR_CONTEXT=<archivo>` en `transcribe_session` y `simulate_stream`. Prueba sobre la llamada de ventas sintética (4,3 min); «nombres» = apariciones acertadas de 14 nombres propios y términos del guion (30 en total). Archivos en `scratchpad/ctx/`.

| Contexto | 1.7B 4 bits: WER / CER | Nombres | 0.6B: WER / CER | Nombres |
|---|---|---|---|---|
| ninguno | 6,3 / 2,0 % | 16/30 | 7,9 / 2,5 % | 17/30 |
| calendario: título y asistentes con su papel | 5,9 / 2,1 % | 19/30 | 8,3 / 2,7 % | 16/30 |
| calendario + lista de términos de la reunión | **3,7 / 1,3 %** | **28/30** | 7,9 / 2,4 % | 23/30 |
| calendario + términos + 27 términos que no salen | 3,7 / 1,3 % | 27/30 | — | — |
| contexto de otra reunión (nombres y términos ajenos) | 6,6 / 2,2 % | 17/30 | 8,8 / 3,2 % | 16/30 |

- Con el 1.7B, la lista de términos baja el WER del sistema del 10,5 al 5,1 %: «a P C barra cuatro Ana» → «SAP S/4HANA», «y niego el severio» → «Ignacio Etxeberria» (aún mal el nombre), «Limbao, Victoria» → «Bilbao, Vitoria», «sal» → «SAP». También arregla palabras comunes del entorno («cambios» → «campos», «entradas» → «entregas»).
- El título y los asistentes solos apenas cambian nada: los nombres de los asistentes ya salían bien.
- Un glosario largo con términos que no se dicen no estorba: ninguno de los 27 aparece inventado y el WER es el mismo. Un contexto equivocado empeora poco (6,3 → 6,6 % en el 1.7B).
- El 0.6B aprovecha menos el contexto: acierta más nombres (17 → 23) pero el WER no baja. Otro motivo para que los finales los haga el 1.7B.
- No se nota en la velocidad (25-30 s en las dos versiones, con la máquina cargada).
- Limitación: los términos son justo los del guion y el audio es TTS. Falta medirlo con grabaciones reales.
- En la app (`recording.rs`, `asr_context`; formato en `juanola_stt::context`): al pulsar Start se juntan el título de la nota, los asistentes del evento (los que no rechazaron), su descripción sin los datos de la videollamada (máx. 600 caracteres), las notas ya escritas (máx. 1000) y el vocabulario del usuario (Preferencias › Transcripción, clave `stt.glossary`, máx. 200 términos). Con ese formato, la misma prueba da 3,7 / 1,3 % y 28/30.
- En la .app (`JUANOLA_AUTOTEST=context`, 9 nov): el canal del sistema reproducido por los altavoces (auriculares Bluetooth) y capturado en vivo, con título y vocabulario, da 5,4 / 2,0 % de WER y 15/15 nombres del canal; sin conexión, el 1.7B daba 10,5 % sin contexto y 5,1 % con él. La pasada sin vocabulario se paró a los 48 s (Stop de la píldora pulsado a mano), pero sus dos frases traen los mismos fallos que sin conexión («Limbaú, Victoria»).
- Lo que se escribe durante la reunión no entra: el contexto se fija al arrancar el worker. Pasarlo en caliente necesitaría un mensaje nuevo por stdin.
