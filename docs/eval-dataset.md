# Plan: dataset multilingüe end-to-end para elegir la combinación STT + LLM

Fecha: 8 de noviembre de 2026. Estado: propuesta, sin nada construido todavía.

## 0. Resumen

**No existe un dataset público de reuniones multilingüe con audio + transcripción + resumen**, que es lo que haría falta para medir el pipeline completo (audio dual → STT → enhance) de una vez. Lo más cercano es AMI (inglés), ELITR Minuting Corpus (inglés y checo, sin audio público) y MLC-SLM (11 idiomas conversacionales, pero el audio lo distribuye Nexdata bajo acuerdo). Por eso el plan construye el dataset en cuatro capas, cada una con un propósito y un coste distintos:

| Capa | Qué mide | Fuente | Idiomas | Coste |
|---|---|---|---|---|
| A. STT corto real | WER/CER por idioma, detección de idioma, velocidad | FLEURS + Common Voice 17 + MLS (Hugging Face) | es, en, pt, fr, de, it (los del selector) | Bajo: solo descargar y correr |
| B. Reuniones y conversaciones reales | Pipeline completo contra transcripción humana (y resúmenes donde los hay) | AMI, ICSI, SUMM-RE, MSLT, Bundestag, DiPCo… (detalle en §2b) | en, fr, de bien cubiertos; es, pt, it solo con parlamento, llamadas de pago o permiso | Bajo-medio |
| C. Reuniones sintéticas controladas | Pipeline completo con verdad exacta (hechos, tareas, hablante, canal, tiempos) | Guiones generados + TTS multilingüe (Qwen3-TTS en `mlx-audio`) + degradaciones acústicas | es, en, pt, fr, de, it | Medio: hay que construir el generador; es el núcleo del plan |
| D. Reuniones reales propias | Calibración: ¿lo que dice C se parece a la realidad? | Grabadas con Juanola por el usuario, anotadas a mano | es + al menos otro | Alto por reunión, pocas unidades |

La capa C es la que permite comparar combinaciones con la misma entrada y separar los errores del STT de los del LLM. Las capas A, B y D sirven para comprobar que C no engaña (el TTS es demasiado limpio y demasiado regular).

## 1. Qué combinaciones se quieren comparar

Ejes que hoy se pueden variar sin tocar la arquitectura (todo comprobado en la API de Hugging Face el 8 nov 2026):

- **STT**: `mlx-community/Qwen3-ASR-0.6B-4bit` (actual) frente a `mlx-community/Qwen3-ASR-1.7B-4bit` (Apache 2.0, existe). Qwen3-ASR cubre 30 idiomas, entre ellos los seis del selector.
- **Idioma**: fijado por el usuario frente a «Detectar» con `LanguageLock` (umbral 8 s / 60 %).
- **Preprocesado del enhance**: ITN sí/no. Ojo: `crates/enhance/src/itn.rs` solo convierte números en español; para los otros idiomas hace falta ITN propio o medir sin él.
- **LLM del enhance**: `LiquidAI/LFM3-1.7B-GGUF` Q4_K_M (actual) y Q8_0; `Qwen/Qwen3-1.7B-GGUF`; `unsloth/Qwen3-4B-Instruct-2507-GGUF`; `unsloth/gemma-3n-E4B-it-GGUF`; `google/gemma-3-4b-it-qat-q4_0-gguf`. No existe LFM3 de más de 1,7B (la familia es 1.7B, Audio-2B, Omni-2B y VL-2B). Cada modelo nuevo necesita su plantilla de chat en `crates/llm/src/chat_format.rs` o usar la del GGUF si `minijinja` la soporta.
- **Pipeline**: solo resumen frente a híbrido (`detail_chunk_secs` 240, probar 180 y 360), y prompts en español con `prompt_suffix` frente a prompts traducidos al idioma de la reunión.

Con 2 STT × 2 idiomas-modo × 5 LLM × 2 pipelines son 40 celdas; el STT solo hay que correrlo 4 veces por reunión porque sus salidas se guardan y el enhance se evalúa encima. Conviene fijar primero el STT (capa A + C) y después barrer los LLM sobre las transcripciones ganadoras.

## 2. Lo que existe y para qué sirve (verificado)

### Habla corta multilingüe, con transcripción humana

| Dataset | Repo HF | Licencia | Notas |
|---|---|---|---|
| FLEURS | `google/fleurs` | CC BY 4.0 | 102 idiomas, lectura de Wikipedia, n-way paralelo (la misma frase en todos los idiomas: ideal para medir detección de idioma). Qwen3-ASR publica WER en FLEURS, así que sirve de cordura. |
| Common Voice 17 | `mozilla-foundation/common_voice_17_0` | CC0 (requiere aceptar condiciones en HF) | Voces de voluntarios, micrófonos malos, acentos variados. Lo más parecido al canal «Me» con un Bluetooth en manos libres. |
| Multilingual LibriSpeech | `facebook/multilingual_librispeech` | CC BY 4.0 | Audiolibros; limpio y largo. Útil para el canal del sistema (voz nítida) y para medir el bucle del decodificador en segmentos largos. |
| VoxPopuli | `facebook/voxpopuli` | CC0 / otros | Parlamento Europeo, habla semiespontánea, 16 idiomas. Buen complemento de es/fr/de/it. |

### Reuniones y conversación

| Dataset | Acceso | Qué trae | Limitación |
|---|---|---|---|
| AMI Meeting Corpus | `knkarthick/AMI` en HF (CC BY 4.0, acceso automático) y ami-corpus.org | ~100 h de reuniones en inglés, audio headset y de sala, transcripción, resúmenes abstractivos, decisiones y acciones | Solo inglés. Es la única referencia real con resúmenes humanos. |
| ELITR Minuting Corpus | LINDAT, hdl.handle.net/11234/1-4692, CC BY-NC-SA 4.0 | 120 reuniones en inglés y 59 en checo (~180 h) con transcripciones corregidas y varias minutas por reunión | **Sin audio** (el paper lo excluye por privacidad). Sirve para evaluar el enhance a partir de texto, no el STT. |
| MLC-SLM (Interspeech 2025) | Texto de evaluación en `bsmu/MLC-SLM-Eval` (CC BY-SA 4.0); audio completo en nexdata.ai/competition/mlc-slm | 1 604 h de conversaciones de ~20 min entre dos personas en 11 idiomas (incluye es, pt, fr, de, it, en con acentos) | El audio exige registrarse en Nexdata y aceptar sus condiciones. Si el usuario lo consigue, es el mejor sustituto real de la capa C. Sin resúmenes. |
| MeetingBank | `huuuyeah/meetingbank` | Plenos municipales en inglés con resúmenes | Inglés, CC BY-NC-SA. Prescindible teniendo AMI. |
| FREDSum | arXiv 2312.04843 | Debates políticos franceses transcritos con resúmenes | Francés, debates, no reuniones. Opcional para enhance solo texto. |
| VoxSumm | arXiv 2608.10359 | Noticias largas habladas en 24 idiomas con resúmenes | Noticias, un solo hablante. Descartado. |
| Emilia | `amphion/Emilia-Dataset` | Habla espontánea masiva en 6 idiomas | Transcripción automática, no humana. Descartado como referencia. |

Descartado por licencia o alcance: CallHome y Fisher (LDC, de pago), AISHELL-4 y AliMeeting (solo mandarín), NOTSOFAR (inglés, acuerdo propio).

## 2b. Capa B: reuniones reales que sí se pueden obtener (búsqueda del 8 nov 2026)

Criterio: audio descargable, transcripción humana, varios hablantes y habla espontánea. Lo que falta en todos los casos salvo AMI es un resumen humano, así que en la capa B el enhance se evalúa con `facts.json` escritos a mano a partir de la transcripción (unos 20 min por reunión), y con los resúmenes de AMI donde existen.

### Reuniones de verdad (varios hablantes, mesa o videollamada)

| Corpus | Idioma | Qué es | Acceso y licencia | Para qué nos vale |
|---|---|---|---|---|
| **AMI Meeting Corpus** | en | ~100 h, 171 reuniones de 4 personas (2/3 son escenario de diseño de un mando a distancia, el resto reales), audio por auricular y de sala, transcripción, resúmenes abstractivos, decisiones y acciones | Descarga libre en groups.inf.ed.ac.uk/ami/download (headset mix ~30 MB por reunión, 24 GB todo); espejo `edinburghcstr/ami` en HF (CC BY 4.0). OpenSLR lo lista como CC BY-NC-SA 2.0; la web oficial dice CC BY. Para evaluación interna da igual. | La única con resúmenes humanos: pipeline completo con ROUGE/BERTScore y recall de decisiones y acciones. Mezcla headset = canal «Me»; micrófono de sala = canal del sistema con reverberación. |
| **ICSI Meeting Corpus** | en | ~70 h, 75 reuniones reales de grupos de trabajo (no escenario), auricular + mesa, transcripción, actos de diálogo | Descarga libre en groups.inf.ed.ac.uk/ami/icsi, CC BY 4.0 (la versión LDC es de pago, no hace falta) | Reuniones técnicas reales con jerga, acentos no nativos y solapes: lo más parecido a una reunión de ingeniería. Sin resúmenes. |
| **SUMM-RE** | fr | 283 conversaciones de ~20 min tipo reunión (informar, decidir, planificar), 3-4 hablantes, una pista de audio por hablante, grabadas en laboratorio, estudio, **Zoom** y casa | `linagora/SUMM-RE` en HF, CC BY-SA 4.0, 94 GB. Dev (36) y test (37) transcritos a mano; train con Whisper. El corpus completo con resúmenes todavía no está publicado en HF. | El mejor hallazgo fuera del inglés: formato de reunión, pistas separadas (se pueden mezclar en «Me» y «Them» a voluntad) y parte grabada por Zoom. Son conversaciones con escenario, no reuniones de empresa. |
| **MSLT** (Microsoft Speech Language Translation) | en, fr, de | Conversaciones bilaterales por **Skype** entre consultores sobre temas dados; audio, transcripción con disfluencias y limpia, traducción | Zip de 2 GB en el Microsoft Download Center (id 54689); licencia no publicada en la ficha, revisar el zip | Audio de videollamada real por canal: idéntico a nuestro caso de uso. Conjuntos dev y test pequeños (horas, no días). |
| **DiPCo** (Dinner Party Corpus) | en | ~5 h, 10 sesiones de 4 personas cenando, micrófonos cercanos y arrays lejanos | Zenodo 8122551 (13 GB) y `huckiyang/DiPCo`, CDLA-Permissive 1.0 | Caso duro: solapes, ruido de fondo y campo lejano. Útil para el nivel «dura» de la capa C. |
| **MLC-SLM** (Interspeech 2025) | en (5 acentos), fr, de, it, pt, es, ja, ko, ru, th, vi | 1 604 h de conversaciones de ~20 min a dos, con segmentación, hablante y transcripción | Texto de evaluación libre en `bsmu/MLC-SLM-Eval` (CC BY-SA 4.0); **el audio solo con registro en nexdata.ai/competition/mlc-slm** | Si se consigue, cubre de golpe los seis idiomas con habla real. Es el único camino real para es, pt e it. |

### Habla espontánea de varios hablantes, pero no reuniones

| Corpus | Idioma | Qué es | Acceso y licencia | Limitación |
|---|---|---|---|---|
| **MeetingBank** | en | 1 366 plenos municipales de 6 ciudades de EE. UU., 3 579 h de vídeo, transcripciones, actas y resúmenes por punto del orden del día | Transcripciones y resúmenes en Zenodo 7989108 y `huuuyeah/meetingbank`; audio en `huuuyeah/MeetingBank_Audio`; CC BY-NC-SA 4.0 | Son plenos, no reuniones de trabajo; muy largos. Sirve para medir el enhance por tramos. |
| **Earnings-22** | en (acentos de todo el mundo) | 119 h de llamadas de resultados completas con transcripción | `distil-whisper/earnings22` en HF, CC BY-SA 4.0 | Preguntas y respuestas con cifras: estupendo para la fidelidad de números del enhance. Monólogo en gran parte. |
| **ASR Bundestag** | de | 610 h alineadas de plenos **y reuniones de comisiones** del Bundestag | `D4ve-R/bundestag-asr` en HF (condiciones de uso del Bundestag); `Yaasr/bundestagv2` está cerrado por dudas legales sobre redistribuir el audio | Las comisiones son lo más parecido a una reunión en alemán. Alineación por subtítulos, no transcripción verbatim. |
| **FOLK** (IDS Mannheim) | de | ~400 interacciones reales transcritas, entre ellas reuniones de trabajo y de asociaciones | Registro gratuito en la DGD (dgd.ids-mannheim.de) acreditando fines científicos o docentes; descarga limitada | Reuniones alemanas reales, pero acceso con solicitud y uso no comercial. |
| **KIParla** | it | 228-350 h de conversación cotidiana transcrita a mano | Transcripciones en CLARIN-IT (CC BY-NC-SA 4.0); **el audio exige solicitud a los coordinadores** por RGPD | Conversación real en italiano; el audio depende de que lo concedan. |
| **CORAA v1.1** | pt-BR | 290 h; incluye entrevistas y diálogos espontáneos de NURC-Recife, SP-2010 y C-ORAL-Brasil | GitHub nilc-nlp/CORAA y espejos en HF (`gabrielrstan/CORAA-v1.1`), CC BY-NC-ND 4.0 | Portugués de Brasil, segmentado en frases cortas, no reuniones. |
| **Europarl-ST** | en, es, fr, de, it, pt (+ro, pl, nl) | Debates del Parlamento Europeo 2008-2012, audio + transcripción + traducción | mllp.upv.es/europarl-st, CC BY-NC 4.0 | Discursos semiespontáneos de un solo orador por turno. Cubre los seis idiomas con el mismo formato: bueno para WER comparable entre idiomas, flojo como «reunión». |
| **VoxPopuli** | 16 idiomas, incluidos los seis | Parlamento Europeo, 1 800 h transcritas | `facebook/voxpopuli`, CC0 | Igual que Europarl-ST, más grande y con licencia más limpia. |
| **TEDx Spanish** | es | 24 h de charlas, transcripción sin puntuación | OpenSLR 67, CC BY-NC-ND 4.0 | Monólogo. Única fuente abierta de español espontáneo largo. |
| **Albayzín RTVE** | es | Programas de RTVE con transcripción humana (RTVE2020) incluidos debates | Licencia con RTVE por formulario (gratis, solo investigación, revocable) | Debates reales en español; hay que pedirlo y aceptar sus condiciones. |

### Descartado o sin salida

- **ELITR Minuting Corpus**: sin audio (excluido por privacidad). Solo texto.
- **Fisher Spanish, CallHome (es, de, …), HUB5, Verbmobil, ICSI vía LDC**: de pago en LDC o ELRA.
- **AISHELL-4, AliMeeting**: mandarín.
- **Spotify Podcast Dataset**: inglés (y algo de portugués), transcripción automática, acceso por solicitud.
- **Granary, Emilia, YODAS**: transcripción automática, no sirven de referencia.
- **Common Voice Spontaneous Speech**: anunciado (CC0), sin publicación confirmada.

### Qué descargar primero para la capa B

1. **AMI** test set oficial (16 reuniones, headset mix + sala) con sus resúmenes, decisiones y acciones: referencia end-to-end en inglés. ~1 GB.
2. **ICSI**, 10 reuniones con auricular: reuniones técnicas reales. ~2 GB.
3. **SUMM-RE** dev + test (73 conversaciones, 84 h de pistas): francés real tipo reunión, mezclando las pistas de los hablantes en dos canales. Pesa, así que descargar solo las columnas de dev/test (~25 GB).
4. **MSLT** dev y test en fr y de (2 GB): audio de Skype por canal.
5. **DiPCo** (13 GB): caso duro en inglés.
6. **Europarl-ST** o **VoxPopuli** en es, pt, it, de: WER comparable en los seis idiomas mientras no haya reuniones en esos idiomas. VoxPopuli está en HF y pesa menos por idioma.
7. En paralelo, **pedir el audio de MLC-SLM a Nexdata**: es lo único que da conversación real en español, portugués e italiano.

Para cada reunión elegida se escribe `facts.json` a mano (20-30 hechos y tareas) con el mismo esquema que la capa C, de modo que `eval_enhance.rs` corra igual sobre reuniones reales y sintéticas.

## 3. Capa C: reuniones sintéticas multilingües (el trabajo principal)

Generaliza lo que ya hay: `crates/enhance/fixtures/reunion_producto.txt` (guion `Hablante|texto`, «Yo» al canal mic y el resto al sistema), `reunion_producto.facts.json` (36 hechos y tareas) y `notas_usuario.txt`. Hoy se sintetizó con `say`; la capa C lo sustituye por un TTS neuronal multilingüe con varias voces y añade degradaciones.

### 3.1 Guiones

- Por idioma, **5 reuniones de 12-18 minutos** con dominios distintos (producto, ventas, soporte, ingeniería, dirección) y 3-4 hablantes; el usuario («Yo») siempre participa. Total: 30 reuniones, ~7,5 h de audio.
- Cada guion lleva de serie: 25-40 hechos con cifras, fechas y nombres propios; 6-8 tareas con responsable y plazo; 2-3 cambios de tema con solapamiento parcial; 1-2 pasajes con jerga técnica y nombres extranjeros (lo que más rompe el STT); un pasaje con una cifra dicha con letras y otro con una fecha relativa («el jueves que viene»).
- Los guiones se generan con un LLM grande fuera del producto (el usuario elige cuál) a partir de una plantilla fija, **en el idioma destino directamente**, no traducidos: las traducciones producen frases que nadie diría en una reunión. Revisión humana rápida de cada guion (10 min) para quitar anglicismos raros.
- Junto al guion se genera `facts.json` con el mismo esquema que ya consume `eval_enhance.rs` (grupos de alternativas, `task: true`), y `notes.txt` con 4-6 notas sueltas que el usuario habría escrito.
- Para medir **detección de idioma** con cambios de código, 2 reuniones extra mezclan dos idiomas (es/en y pt/es), y 1 reunión en español incluye nombres de productos en inglés.

### 3.2 Síntesis de voz

- Modelo: `Qwen3-TTS-12Hz-1.7B` (Apache 2.0, 10 idiomas que incluyen los seis del selector), corriendo en `mlx-audio` sobre los pesos `mlx-community/Qwen3-TTS-12Hz-1.7B-Base-bf16` (o la variante CustomVoice para voces preset). Alternativa más ligera: Kokoro 82M (sin alemán claro). Chatterbox Multilingual (MIT, 23 idiomas) como segunda voz para que no todos los hablantes compartan timbre del mismo modelo.
- Una voz distinta por hablante y por idioma; el hablante «Yo» con la misma voz en todas las reuniones del idioma. Pequeñas pausas aleatorias (0,3-1,2 s) entre turnos y, en 10 % de los turnos, solapamiento de 200-500 ms con el turno anterior.
- Salida: `mic.wav` y `system.wav` a 16 kHz mono alineados, como produce `CpalCapture`, y `segments.ref.json` con el texto exacto, hablante, canal y los tiempos de inicio y fin de cada turno (los da el TTS al concatenar). Esta es la verdad para el WER y para el canal.

### 3.3 Degradaciones (hacen que C se parezca a D)

Tres versiones de cada reunión, generadas a partir de la limpia:

1. **Limpia**: tal cual sale del TTS.
2. **Realista**: canal mic con respuesta de manos libres Bluetooth (paso banda 300-3 400 Hz + remuestreo a 8 kHz y vuelta, nivel −6 dB, ruido de teclado y oficina a 25-30 dB SNR); canal sistema con códec de videollamada (Opus a 24 kb/s ida y vuelta, `ffmpeg`), ligera reverberación.
3. **Dura**: realista + eco del altavoz en el mic (−18 dB, retardo 40-80 ms, para el caso sin auriculares), SNR 15 dB y recortes de 100-300 ms al 2 % de los turnos (simulan los huecos que rellena el watchdog).

Todo con `ffmpeg` y `sox`, o en Rust con `rubato` ya presente; los parámetros se guardan en `meta.json`.

### 3.4 Formato de disco

```
datasets/
  README.md                 # procedencia y licencia de cada fuente
  synthetic/<lang>/<meeting_id>/
    script.txt              # Hablante|texto (mismo formato que fixtures/)
    facts.json              # mismo esquema que eval_enhance.rs
    notes.txt               # notas del usuario
    meta.json               # idioma(s), voces, duración, versión de degradación
    clean/{mic,system}.wav  realistic/… hard/…
    segments.ref.json       # verdad: texto, canal, hablante, start, end
  real/en/ami/<meeting>/…   # convertido al mismo esquema (hechos extraídos de los resúmenes de AMI)
  real/own/<lang>/<fecha>/… # capa D
  short/<lang>/{fleurs,cv17,mls}.jsonl   # ruta del clip + transcripción normalizada
runs/<fecha>-<etiqueta>/    # salidas: segments.json por celda, EnhancedNotes, métricas CSV
```

El dataset no va al repo git (pesa gigas): `datasets/` y `runs/` en `.gitignore`, con un script que lo regenera de forma determinista (semilla fija para voces, pausas y ruido).

## 4. Métricas

**STT (capas A, C, D)**
- WER y CER por idioma, por canal y por degradación. Normalización multilingüe antes de comparar: minúsculas, sin puntuación, Unicode NFKC, números a dígitos. Hay que usar un normalizador por idioma (el `norm()` de `eval_enhance.rs` solo quita tildes españolas); conviene adoptar el normalizador de Whisper/Open ASR Leaderboard para ser comparables con las cifras publicadas.
- Canal correcto (ya se mide: 136/136), tasa de segmentos en bucle, latencia (mediana, p90) con `simulate_stream`.
- Detección de idioma: idioma final fijado correcto sí/no, segundos hasta fijarlo, y porcentaje de segmentos con idioma equivocado antes de fijar. Con las reuniones mezcladas: porcentaje del segundo idioma transcrito en el idioma correcto.

**Enhance (capas B, C, D)**
- Las que ya calcula `eval_enhance.rs`: cobertura de hechos alcanzables, tareas con responsable, cifras fieles al tramo citado, secciones y duplicados, viñetas copiadas, tiempo.
- Nuevas: **cobertura total** (sobre todos los hechos, no solo los alcanzables), que es la métrica end-to-end y cuya diferencia con la cobertura alcanzable es la pérdida atribuible al STT; **idioma de salida** correcto (porcentaje de viñetas en el idioma de la reunión, con una heurística de palabras funcionales como `summary_language` o un clasificador pequeño); **referencias válidas** (ya se comprueba contra el esquema).
- En AMI: además ROUGE-L y BERTScore de las viñetas de la IA frente al resumen abstractivo humano, y recall de las «decisions»/«actions» anotadas, usando sus hechos como `facts.json`.

**Agregado**: una tabla por combinación con WER medio, cobertura total media, tareas correctas, cifras fieles, tiempo total por minuto de reunión y memoria pico. La decisión final se toma mirando cobertura total y tareas en la versión «realista», no en la limpia.

## 5. Herramientas a construir

1. `tools/evalset/gen_scripts.md`: plantilla de prompt para generar guion + facts + notas en un idioma dado. Salida revisada a mano y guardada.
2. `tools/evalset/synth.py` (venv aparte, fuera del repo como el de mlx-audio): guion → `clean/mic.wav`, `clean/system.wav`, `segments.ref.json`. Usa `mlx-audio` con Qwen3-TTS; asigna voces; inserta pausas y solapes con semilla.
3. `tools/evalset/degrade.sh`: `clean/` → `realistic/` y `hard/` con `ffmpeg`/`sox`; escribe los parámetros en `meta.json`.
4. `tools/evalset/fetch_short.py`: descarga N clips por idioma de FLEURS, CV17 y MLS (200 por fuente e idioma bastan; ~1 h por idioma) y escribe los `jsonl`.
5. `tools/evalset/ami.py`: convierte reuniones de AMI (10 del test set, con headset) al esquema común y extrae `facts.json` de las decisiones y acciones anotadas.
6. En Rust, un ejemplo `crates/stt/examples/eval_wer.rs` que corre `transcribe_session` sobre cada reunión y clip, aplica el normalizador y escribe CSV; y generalizar `eval_enhance.rs` a varios idiomas (normalizador por idioma, ITN opcional por idioma, idioma de salida) y a varios modelos GGUF (ruta y plantilla por parámetro).
7. `tools/evalset/run_matrix.sh`: recorre las celdas, guarda cada salida bajo `runs/` y produce la tabla final en Markdown.

## 6. Tamaño y tiempos estimados (M4 Pro)

| Pieza | Volumen | Tiempo estimado |
|---|---|---|
| Guiones (30 + 3 mezclados) | ~33 × 15 min | 1 día entre generación y revisión |
| TTS | ~8 h de audio | 2-4 h de máquina (Qwen3-TTS 1.7B va cerca de tiempo real en MLX; sin medir) |
| Degradaciones | ×3 | minutos |
| STT, 4 configuraciones × 3 degradaciones × 8 h | ~100 h de audio | el 0.6B transcribe 17 min en 62 s: unas 6 h; el 1.7B más, sin medir |
| Enhance, 10 celdas × 33 reuniones × 3 semillas | ~1 000 ejecuciones | a 60-80 s cada una, unas 20 h; se puede acotar a la degradación «realista» y 1 semilla primero |
| Capa A | 6 idiomas × 600 clips | 1-2 h |
| Capa B | 10 reuniones AMI (~35 min cada una) | 2-3 h |
| Capa D | 4-6 reuniones reales | depende del usuario; anotar 10-15 hechos por reunión cuesta ~20 min |

Fases propuestas: (1) capa A + `eval_wer.rs`, para fijar STT y normalizadores en dos días; (2) generador de la capa C con una reunión por idioma, comprobar que el TTS pronuncia bien cifras y nombres; (3) resto de guiones, degradaciones y barrido de LLM; (4) AMI y reuniones propias para validar las conclusiones.

## 7. Riesgos y cómo se cubren

- **El TTS es demasiado limpio y regular**: el WER sintético saldrá optimista. Se corrige con las degradaciones y se contrasta con CV17 (capa A) y las reuniones propias (capa D). Si el WER de D queda muy por encima de «realista», endurecer los parámetros.
- **Pronunciación de cifras y fechas por el TTS**: cada modelo las lee a su manera (y en español Qwen3-TTS tiende al acento latinoamericano). Escuchar los primeros guiones; si falla, escribir las cifras con letras en el guion, que además es lo que pasa en la realidad y ejercita el ITN.
- **Hechos en otros idiomas**: el emparejado de `facts.json` depende de la normalización; en alemán los compuestos y en francés las elisiones obligan a dar varias alternativas por hecho.
- **ITN solo en español**: medir el enhance en los otros idiomas sin ITN y anotar la diferencia; decidir si merece implementar ITN por idioma.
- **Varianza del LLM**: con 1,7B la cobertura oscila 15 puntos entre semillas. Tres semillas por celda y reportar mediana e intervalo, nunca una sola ejecución.
- **Licencias**: todo lo listado permite uso interno de evaluación. Common Voice exige aceptar condiciones en HF; AMI es CC BY 4.0; ELITR y MeetingBank son NC, válidos porque no se vende. Nexdata (MLC-SLM) queda fuera salvo que el usuario quiera registrarse.

## 8. Decisiones que necesita el usuario

1. Qué LLM externo genera los guiones (y si prefiere escribir alguno a mano).
2. Confirmar los seis idiomas del selector como alcance, o reducir a es/en/pt para la primera vuelta.
3. Si vale la pena pedir el audio de MLC-SLM a Nexdata para tener conversación real en todos los idiomas.
4. Presupuesto de disco y de horas de máquina para el barrido completo frente a una primera pasada reducida (STT 0.6B frente a 1.7B y LFM3 Q4 frente a Qwen3-4B, solo «realista»).

## Fuentes

- Qwen3-ASR, idiomas y conjuntos de evaluación: https://huggingface.co/Qwen/Qwen3-ASR-1.7B y https://arxiv.org/abs/2601.21337
- Open ASR Leaderboard (pistas multilingüe y long-form, normalizador): https://arxiv.org/html/2510.06961v3
- ELITR Minuting Corpus: https://aclanthology.org/2022.lrec-1.340 y https://lindat.mff.cuni.cz/services/catalog/view/http:__hdl_handle_net_11234_1-4692 ; AutoMin 2025: https://arxiv.org/pdf/2509.13814
- MLC-SLM: https://arxiv.org/html/2509.13785v1 y https://huggingface.co/datasets/bsmu/MLC-SLM-Eval
- AMI: https://groups.inf.ed.ac.uk/ami/download , https://groups.inf.ed.ac.uk/ami/corpus/overview.shtml , https://huggingface.co/datasets/edinburghcstr/ami ; ICSI: https://groups.inf.ed.ac.uk/ami/icsi/ y https://groups.inf.ed.ac.uk/ami/icsi/license.shtml
- SUMM-RE: https://huggingface.co/datasets/linagora/SUMM-RE y https://aclanthology.org/2024.jeptalnrecital-taln.35.pdf ; MSLT: https://www.microsoft.com/en-us/download/details.aspx?id=54689 y https://aclanthology.org/2016.iwslt-1.12 ; DiPCo: https://zenodo.org/record/8122551
- MeetingBank: https://huggingface.co/datasets/huuuyeah/meetingbank , https://huggingface.co/datasets/huuuyeah/MeetingBank_Audio , https://zenodo.org/records/7989108 ; Earnings-22: https://huggingface.co/datasets/distil-whisper/earnings22
- ASR Bundestag: https://arxiv.org/abs/2302.06008 y https://huggingface.co/datasets/D4ve-R/bundestag-asr ; FOLK: https://agd.ids-mannheim.de/download/FOLK_Flyer_Teilnehmende_2025_4_Seiten.pdf ; KIParla: https://dspace-clarin-it.ilc.cnr.it/items/f695a3ea-8f6a-44bb-808d-c8ffd011b686 ; CORAA: https://arxiv.org/abs/2110.15731
- Europarl-ST: https://arxiv.org/pdf/1911.03167 y https://www.mllp.upv.es/europarl-st/ ; TEDx Spanish: https://www.openslr.org/67/ ; Albayzín RTVE: https://catedrartve.unizar.es/rtvedatabase.html y https://catedrartve.unizar.es/docs/RTVE_database_license_v2025_v3.pdf
- FLEURS: https://huggingface.co/datasets/google/fleurs ; Common Voice 17: https://huggingface.co/datasets/mozilla-foundation/common_voice_17_0 ; MLS: https://www.openslr.org/94/ ; VoxPopuli: https://huggingface.co/datasets/facebook/voxpopuli
- FREDSum: https://arxiv.org/pdf/2312.04843 ; VoxSumm: https://arxiv.org/abs/2608.10359 ; EuroSpeech: https://arxiv.org/abs/2510.00514
- Qwen3-TTS: https://github.com/QwenLM/Qwen3-TTS y https://qwen.ai/blog?id=qwen3tts-0115 ; mlx-audio: https://github.com/Blaizzy/mlx-audio ; Chatterbox: https://huggingface.co/ResembleAI/chatterbox ; Kokoro: https://huggingface.co/hexgrad/Kokoro-82M
- Common Voice Spontaneous Speech (aún sin dataset publicado que se haya podido confirmar): https://www.mozillafoundation.org/en/blog/common-voice-navbar-changes-and-spontaneous-speech
