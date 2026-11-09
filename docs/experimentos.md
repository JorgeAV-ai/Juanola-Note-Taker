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
