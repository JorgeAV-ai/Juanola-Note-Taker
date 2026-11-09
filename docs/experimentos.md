# Registro de experimentos

Qué se ha probado, qué resultado dio y si se quedó o se descartó, para no repetir experimentos. El detalle (tablas por semilla, salidas) está en el documento enlazado en cada fila.

**Antes de probar algo, busca aquí.** Al terminar un experimento, añade una fila aunque salga mal: un descarte medido vale tanto como una mejora.

Estados: ✅ adoptado · ❌ descartado (medido) · ⚗️ funciona pero queda como opción desactivada · ⏳ idea sin probar.

## Cómo se mide

- Enhance: `cargo run --release -p juanola-enhance --example eval_enhance -- <segmentos.json> <hechos.facts.json> [notas.txt] --runs 3 --variant … [--template clave|archivo.json] [--title …]`. Guiones en `crates/enhance/fixtures/`. Para pasarlos a segmentos: `tools/script_to_segments.py`.
- Con plantilla mide la colocación y dice adónde fue cada hecho mal colocado (`id → apartado`). Para probar títulos sin tocar el código, pasa la plantilla como JSON con `--template archivo.json`: la clave de `section` en los hechos es el nombre del archivo.
- 3 semillas como mínimo. Entre semillas hay ±10 puntos de ruido, así que una diferencia menor no se toma como mejora.

## Enhance en Auto (LFM3-1.7B; Q4_K_M hasta el 9 nov, Q8_0 desde entonces)

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Salida en JSON forzada con gramática | El modelo copia líneas de la transcripción y la calidad se hunde | ❌ | 6 nov | HANDOFF, punto 3 |
| Markdown (`## tema`, viñetas con `[n, m]`) que el código convierte a `EnhancedNotes` | Funciona; base de todo lo demás | ✅ | 6 nov | HANDOFF, punto 3 |
| ITN: números escritos con letras pasados a dígitos antes del LLM | Cifras fieles del 86 % al 98-100 %; copiadas del 70 % al 12-24 % | ✅ | 6 nov | HANDOFF, punto 3 |
| Pasadas de detalle cada 4 min (`detail_chunk_secs = 240`) | Cobertura del 69-77 % al 81-96 %; tarda ~60-80 s frente a ~20 s | ✅ | 6 nov | HANDOFF, punto 3 |
| Resumir por fragmentos y reagrupar temas con el modelo (JSON) | 25-39 secciones o reagrupa de más; pierde tareas | ❌ | 6 nov | HANDOFF, punto 3 |
| Pedir «más viñetas» o «todas las cifras» en una pasada | Sin mejora estable | ❌ | 6 nov | HANDOFF, punto 3 |
| Pasada aparte solo para tareas, en JSON (quién/qué/cuándo) | 0-2 de 6 tareas, con frases copiadas | ❌ | 6 nov | HANDOFF, punto 3 |
| Flash attention o cambiar `n_ubatch` | No cambia la velocidad en contexto largo | ❌ | 6 nov | HANDOFF, punto 3 |
| `prompt_suffix` por idioma para resúmenes que no son en español | Sin él se colaban «Yo»/«Ellos» en las viñetas | ✅ | 7 nov | HANDOFF, punto 4 |
| Notas del usuario separadas por bloque (`user_blocks`) | Auto no empeora (89 % de cobertura, igual que antes) | ✅ | 8 nov | plantillas §6.4 (A frente a B) |
| Resumen y pasadas de detalle a la vez, en un lote multi-secuencia de llama.cpp (`complete_many`) | Mismas métricas en todas las semillas (las salidas no cambian) y 35-55 % menos tiempo: semanal en Auto 67 → 31 s, con plantilla 72 → 40 s; 1:1 en Auto 20 → 13 s. Con Q8_0 queda por debajo de lo que tardaba Q4_K_M una a una | ✅ | 9 nov | «Generación en lote», abajo |
| Q8_0 (1,85 GB) frente a Q4_K_M, las 4 reuniones, Auto y plantilla | Reunión larga (17 min) mejor en todo: Auto con cobertura del 86 % al 96 %, tareas de 3/7 a 5/7 y cifras fieles del 91 % al 98 %; con plantilla, colocación del 83 % al 88 % y tareas de 3/7 a 4/7. Reuniones cortas mixtas: en el 1:1 empeora (colocación del 100 % al 77 %, «Feedback» relleno 3 de 3, tareas en Auto de 4/5 a 1/5); el stand-up y ventas, dentro del ruido. Tarda 1,35-2 × | ✅ adoptado (9 nov) | 9 nov | «Q8_0 frente a Q4_K_M», abajo |
| QAD-Q4_0, Q6_K o un modelo mayor solo para las tareas | — | ⏳ | | HANDOFF, punto 3 |
| Mostrar el resumen a los ~20 s y añadir los detalles después | — | ⏳ | | HANDOFF, punto 3 |

## Enhance con plantilla

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Encaje determinista de títulos (`fit_to_template`) y relleno «No se habló de esto» | Los títulos son siempre los de la plantilla | ✅ | 8 nov | plantillas §4.2 |
| `## Otros temas` opcional como salida en el prompt + «no fuerces el contenido» | Colocación de la semanal del 71 % al 87 % | ✅ | 8 nov | §6.4, iteración 2 |
| Comparar cifras normalizadas al colocar («112k» = «112.000») | Arregla los fallos de colocación por cifras | ✅ | 8 nov | §6.4, iteración 2 |
| Tope de tokens propio (`template_budget`: Auto + 150 por apartado, mínimo 800) | El 1:1 dejaba de cortarse antes de «Próximos pasos» (tareas de 1/5 a 4/5) | ✅ | 8 nov | §6.4, iteración 3 |
| Instrucción explícita «cada tarea va solo en `## Próximos pasos`» | Hunde las tareas de la semanal (3/0/0 de 7 frente a 3/5/3) | ❌ | 8 nov | §6.4, iteración 3 |
| Quitar viñetas repetidas entre apartados (`drop_cross_duplicates`) | Menos repeticiones; la tarea repetida se queda en el apartado de tareas | ✅ | 8 nov | §6.4, iteraciones 3 y 4 |
| Quitar `**` dentro de las viñetas | Limpieza | ✅ | 8 nov | §6.4, iteración 3 |
| Descripciones más concretas («solo…», «si nadie dio ninguna, va vacío») | «Feedback» del 1:1 vacío en 2 de 3 semillas (antes 0 de 3); el stand-up no mejora (36 %) | ✅ | 8 nov | §6.4, iteración 4 |
| Gramática GBNF sobre Markdown (títulos fijos y citas obligatorias) | Salida limpia, pero la colocación baja (67 % → 58 %) y tarda ~9 s más | ⚗️ `markdown_grammar` | 9 nov | §6.4, iteración 5 |
| Stand-up: «Hecho desde la última» → «Terminado» | Colocación del 36 % al 67 %; «Otros temas» del 25-29 % al 0-13 % | ✅ | 9 nov | §6.4, iteración 5 |
| Ventas: «Contexto y necesidades · Objeciones» → «Situación del cliente · Dudas y reparos» | Colocación del 71 % al 93 %; copiadas del 52-58 % al 26-33 % | ✅ | 9 nov | §6.4, iteración 6 |
| Ventas: «Situación y problemas» / «Dudas sobre la oferta» (y sus cruces) | 93-100 %, pero con más copiadas o alguna tarea perdida | ❌ | 9 nov | §6.4, iteración 6 |
| Ventas: descripción con «un competidor más barato» | 100 %, pero calcada del guion (sobreajuste) | ❌ | 9 nov | §6.4, iteración 6 |
| Semanal: «Cifras y resultados · Qué se decidió» o «Cifras de la semana · Acuerdos» | 65-92 % y 80-86 % frente a 78-83 %: dentro del ruido | ❌ | 9 nov | §6.4, iteración 6 |

**Lecciones que se repiten:**
- El título pesa más que la descripción.
- Las instrucciones extra en el prompt suelen empeorar otra métrica: revisa siempre tareas y cobertura, no solo la métrica que buscas.
- El modelo es muy sensible a cambios pequeños en la entrada.

**Sin resolver:**
- Detectar el desajuste: una plantilla que no encaja con la reunión no dispara reintento ni caída a Auto.
- Viñetas copiadas en el stand-up (47-65 %).
- Tiempo con plantilla: el resumen escribe ~1700 tokens en la reunión larga y es ahora casi todo el tiempo (~40 s).
- Medir la plantilla de entrevista con cliente: no hay guion.

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

## Otras áreas (descartes medidos)

| Experimento | Resultado | Estado | Fecha | Detalle |
|---|---|---|---|---|
| Detección: `kAudioStreamPropertyIsActive` para saber si una app graba | Vale siempre 1 | ❌ | 8 nov | HANDOFF, detección |
| Detección: `AVCaptureDevice.isInUseByAnotherApplication` | Falso con Chrome grabando | ❌ | 8 nov | HANDOFF, detección |
| Fin de llamada tras 5 s sin micrófono | Para la grabación al silenciarse; pasa a 15 s (`END_WAIT`) | ❌ → 15 s ✅ | 8 nov | HANDOFF, detección |
| Idioma «Detectar» frase a frase en Qwen3-ASR | Salta de idioma en frases cortas; se fija con `LanguageLock` (8 s y 60 %) | ✅ | 7 nov | HANDOFF, punto 4 |

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
