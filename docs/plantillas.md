# Plan: plantillas en el enhance

Fecha: 8 de noviembre de 2026. **Estado (12 nov):** fases 0 y 1 hechas (núcleo, las cinco plantillas integradas, tests y evaluador). Fases 2 y 3 hechas en la rama `feat/plantillas-app` (§7), con sus autotests en verde en la .app (12 nov). Lo medido está en [experimentos.md](experimentos.md), tablas «Enhance · plantillas» y «Plantillas: iteraciones».

Objetivo: comprobar que LFM3-1.7B sirve para generar notas con plantillas (1:1, stand-up, llamada de ventas…) y que en la app falle lo menos posible. Se asume que el modelo basta; no se cambia de modelo ni se entrena.

## 0. Punto de partida (8 nov)

- `Template` existe en `crates/core-types/src/lib.rs:128` con `purpose`, `style` y `structure` (texto libre). La tabla `templates` existe en `0001_init.sql`, pero `repo::templates` está vacío, no hay comandos Tauri ni interfaz, y `enhance_note` pasa siempre `template: None` (`apps/desktop/src-tauri/src/commands.rs:254`).
- Si llega una plantilla, `build_messages` solo añade una frase al contexto (`enhance.rs:168`). El prompt sigue pidiendo temas libres, así que el modelo ignora la estructura.
- El post-proceso está pensado para temas libres: `merge_sections` fusiona títulos parecidos y crea «Próximos pasos», `localize_heading` traduce ese título, `resolve` quita secciones sin citas y crea «Tus notas».
- **Restricción medida:** obligar a LFM3 a escribir JSON con gramática desde el primer token hunde la calidad (`enhance.rs:3`), y una pasada de tareas en JSON dio peores tareas (`enhance.rs:647`). **Por eso la estructura no se fuerza con JSON**: el modelo sigue escribiendo Markdown con citas y el código encaja el resultado en la plantilla.

## 1. Principios para que falle poco

1. **Auto solo cambia en las notas del usuario.** Sin plantilla se ejecuta el pipeline de hoy; lo único que cambia en Auto es el tratamiento de las notas del usuario (§4.5), que corrige un fallo y se comprueba con `eval_enhance.rs` contra las cifras actuales antes de darlo por bueno. Lo demás va por una rama aparte.
2. **La estructura la garantiza el código, no el modelo.** Los títulos de la salida son siempre los de la plantilla, en su orden, aunque el modelo escriba otros.
3. **Nunca se pierde una nota por culpa de la plantilla.** Si la salida no encaja, se reintenta una vez y, si vuelve a fallar, se genera con Auto y se avisa al usuario.
4. **Los apartados vacíos se ven, pero no se rellenan con inventos.** Un apartado sin viñetas citadas aparece con la línea «No se habló de esto», que pone el código y no el modelo. Así se ve que la plantilla se aplicó y qué faltó en la reunión (en un 1:1 o una llamada de ventas, eso es información), y las notas con la misma plantilla tienen siempre la misma forma.
5. **Primero plantillas integradas** (definidas en código, revisadas y probadas). El editor de plantillas del usuario llega después, con validación al guardar.

## 2. Modelo de datos

En `crates/core-types`:

```rust
pub struct TemplateSection {
    pub heading: String,          // título exacto que se muestra
    pub hint: String,             // qué va aquí; solo para el prompt
    pub kind: SectionKind,        // Topics | Tasks
}
pub enum SectionKind { Topics, Tasks }
```

- `Template.structure` pasa de `String` a `Vec<TemplateSection>`. En la base de datos se guarda como JSON en la misma columna `structure TEXT`, así que **no hace falta migración**. Al leer, si el texto no es JSON válido (formato antiguo), cada línea no vacía se convierte en un apartado `Topics` con `hint` vacío.
- Validación (función `Template::validate`, usada al guardar y antes del enhance): de 1 a 10 apartados, títulos de 1 a 60 caracteres sin repetir (comparando sin mayúsculas ni tildes), como mucho un apartado `Tasks`, `hint` de hasta 200 caracteres y sin `#` ni saltos de línea en los títulos.
- La plantilla elegida se guarda en `notes.template_id` (la columna ya existe). `None` significa Auto. «Generar de nuevo» usa la misma.

## 3. Plantillas integradas

En código (`crates/enhance/src/templates.rs`), no en la base de datos: así se versionan con la app y no hace falta sembrarlas. Tienen UUID fijos para que `notes.template_id` las referencie.

| Plantilla | Apartados |
|---|---|
| 1:1 | Cómo está · Temas tratados · Feedback · Próximos pasos (Tasks) |
| Stand-up | Terminado · Plan de hoy · Bloqueos · Próximos pasos (Tasks) |
| Reunión semanal de equipo | Métricas y estado · Temas tratados · Decisiones · Próximos pasos (Tasks) |
| Entrevista con cliente | Contexto del cliente · Problemas y necesidades · Lo que usan hoy · Citas destacables · Próximos pasos (Tasks) |
| Llamada de ventas | Situación del cliente · Dudas y reparos · Presupuesto y plazos · Próximos pasos (Tasks) |

- Cada plantilla integrada trae sus títulos en los seis idiomas del selector (es, en, pt, fr, de, it), escritos a mano. Se usa el del idioma del resumen (`summary_language`). Las plantillas del usuario se muestran tal cual las escribió, sin traducir.
- Un test comprueba que todas pasan `validate` y tienen los seis idiomas.

## 4. Cambios en el enhance (`crates/enhance`)

### 4.1 Prompt

Un prompt nuevo, `crates/prompts/templates/enhance.template.md`, para cuando hay plantilla. Conserva lo que ya funciona (transcripción numerada, citas `[n]`, idioma, no copiar frases) y cambia la parte de la estructura:

```
Escribe las notas usando exactamente estos apartados, en este orden, con estos títulos:
## Cómo está — estado de ánimo y carga de trabajo
## Temas tratados — …
## Próximos pasos — cada tarea: quién, qué y cuándo
Si de un apartado no se habló, escríbelo sin viñetas. No añadas otros apartados.
```

- El `purpose` y el `style` de la plantilla van en el contexto, como ahora.
- `prompt_suffix` tiene una variante para plantillas («usa los títulos exactamente como están»), sin la excepción de «Próximos pasos».
- `sections_hint` no se usa con plantilla: el número de apartados lo da la plantilla.

### 4.2 Encaje en la plantilla: `fit_to_template`

Sustituye a `merge_sections` cuando hay plantilla. Va después de `parse_markdown` y antes de `add_details` y `resolve`:

1. Cada sección del modelo se empareja con un apartado de la plantilla, en este orden:
   1. el título coincide sin mayúsculas, tildes ni puntuación;
   2. el título de la plantilla está contenido en el del modelo, o al revés;
   3. al menos la mitad de palabras en común (`heading_words`, como hoy);
   4. si el modelo escribió una sección de tareas (`is_next_steps`) y la plantilla tiene un apartado `Tasks`, va ahí.
2. Si una sección no encaja, sus viñetas se reparten una a una al apartado `Topics` con más palabras en común con su título y su `hint`. Si no hay ninguno con palabras en común, van a un apartado final **«Otros temas»** (localizado). Así nada de lo que dijo la reunión se pierde.
3. La salida tiene todos los títulos de la plantilla, en su orden. Un apartado sin viñetas recibe una sola viñeta con el texto «No se habló de esto» (localizado al idioma del resumen), con `source: None`, `is_user_text: false` y `is_action_item: false`. El frontend ya pinta sin lupa las viñetas sin `source` (`apps/desktop/src/lib/enhancedDoc.ts:37`), así que no hace falta tocar `EnhancedView`. Esta viñeta se añade después de `resolve`, para que el filtro de viñetas sin cita no la quite. «Otros temas» solo aparece si tiene viñetas.
4. `is_action_item` se decide por `kind == Tasks`, no por palabras clave del título.
5. Viñetas repetidas dentro de un apartado se quitan, como en `merge_sections`.

### 4.3 Resto del pipeline con plantilla

- **Pasada de detalle** (`add_details`): sin cambios. Ya solo inserta en secciones existentes. Basta con que excluya el apartado `Tasks` por `kind` y no por título.
- **Notas del usuario:** ver §4.5.
- **`localize_heading`:** solo se aplica a «Otros temas» y «Tus notas». Los títulos de la plantilla no se tocan.
- **Reunión corta** (`MIN_WORDS`): sin cambios.

### 4.4 Reintento y caída a Auto

En `enhance_with`, con plantilla:

1. Primer intento.
2. Se considera fallido si se cumple cualquiera de estas condiciones:
   - ninguna sección del modelo encaja en la plantilla por título (pasos 1.1 a 1.3 del §4.2);
   - más del 50 % de las viñetas acaba en «Otros temas»;
   - más de la mitad de los apartados de la plantilla queda vacía. Una nota con casi todo «No se habló de esto» no sirve, y suele significar que la plantilla no encaja con la reunión.
3. Si falla, segundo intento con otra semilla y **con el motivo del fallo añadido al final del prompt**, como hace Char. Por ejemplo: «El intento anterior no siguió la plantilla: escribiste `## Resumen` y `## Varios`. Usa exactamente estos títulos y en este orden: `## Cómo está`, `## Temas tratados`, `## Feedback`, `## Próximos pasos`.» El mensaje lo genera el código a partir de lo que encontró `fit_to_template`.
4. Si vuelve a fallar, enhance en Auto. `EnhanceReport` gana `template_fallback: bool`, y la app muestra «No se pudo aplicar la plantilla; se usó Auto».

`EnhanceReport` gana también `unmatched_bullets` (viñetas que fueron a «Otros temas»), `empty_sections` y `template_retries`, para el log y la evaluación.

### 4.5 Notas del usuario

Las notas crudas del usuario deben guiar el resumen: marcan lo que le importó. Hoy solo se pasan como contexto y se colocan por palabras en común, y tienen un fallo con las listas.

**Paso 0, fallo de hoy (independiente de las plantillas).** `user_blocks` (`enhance.rs:46`) solo mira los bloques de primer nivel y concatena el texto de sus hijos sin separador: una lista con «demo jueves» y «llamar a Luis» se convierte en una única nota «demo juevesllamar a Luis», que llega así al prompt y a las notas mejoradas. El test `blocks_from_prosemirror` no lo detecta porque usa una lista de un solo elemento. Detectado leyendo el código; falta reproducirlo en la app. Arreglo:

- Cada elemento de lista (`listItem`, `taskItem`) y cada párrafo o título es su propio `UserBlock`, con su id (`attrs.id`, o `b<n>` por posición en el recorrido) y con espacios entre nodos de texto hermanos.
- `UserBlock` gana `kind: Paragraph | Heading(level) | Task { checked } | ListItem` y `emphasis: bool` (alguna marca de negrita o subrayado).
- Tests: lista de varios elementos, lista anidada, lista de tareas, título con párrafos debajo, y texto con negrita.
- `Citation::UserNote.block_id` sigue apuntando al bloque concreto. Hay que comprobar que el frontend resuelve los ids de los elementos de lista, y si no, asignarles `attrs.id` en el editor.

**Prompt.** En `enhance.user.md` y en `enhance.template.md`, la lista de notas se presenta así:

```
Notas que tomó el usuario durante la reunión (marcan lo que le importó;
desarrolla esos temas con lo que se dijo en la transcripción, sin copiarlas):
- [importante] presupuesto Q3
- [tarea] llamar a Luis
## Contratación          ← título del usuario
- Elena empieza en enero
```

Los títulos del usuario se mantienen como títulos. Las notas con negrita o subrayado, y las que están bajo un título `###`, llevan `[importante]`. Las tareas llevan `[tarea]`.

**Colocación** (en `resolve`, en este orden de prioridad):

1. Una **tarea** del usuario va al apartado de tareas (`kind == Tasks` con plantilla, o «Próximos pasos» en Auto) con `is_action_item: true` y su `done`. Si la plantilla no tiene apartado de tareas, se coloca como nota normal.
2. Una nota bajo un **título del usuario** que coincide con un apartado (el mismo emparejado que en §4.2) va a ese apartado.
3. El resto, como hoy: al apartado con más palabras en común.
4. Si no encaja en ninguno: en Auto, a «Tus notas» al principio, como hoy; **con plantilla, a «Otros temas»**, para no añadir un apartado que la plantilla no tiene.

La línea «No se habló de esto» se añade **después** de colocar las notas del usuario. Un apartado que solo tiene notas del usuario no la lleva.

Más adelante, si el reintento resulta frecuente: generar en streaming (`ChatModel::stream`) y cortar en cuanto el primer título no encaje, como hace Char, para no esperar a la salida completa antes de reintentar.

## 5. Cambios en la app

### 5.1 Base de datos (`crates/db`)

- `repo::templates`: `list`, `get`, `upsert`, `delete`. Las integradas se combinan con las de la base de datos en `list` y no se pueden borrar ni editar (se pueden duplicar).
- `repo::notes::set_template(id, Option<Id>)`.

### 5.2 Comandos Tauri (`commands.rs`)

- `list_templates() -> Vec<Template>`.
- `enhance_note(id, template_id: Option<Option<Id>>)`: si llega, se guarda en la nota; si no, se usa el que tenga la nota. Se carga la plantilla y se le pasa a `EnhanceInput`. Si la plantilla ya no existe (borrada), se usa Auto y se limpia `template_id`.
- El evento `note://status` lleva un aviso cuando hubo `template_fallback`.
- Más adelante: `save_template` y `delete_template`, con `validate`.

### 5.3 Frontend

- `lib/ipc.ts` y `dev/mockIpc.ts`: tipos `Template` y `TemplateSection` y las llamadas nuevas.
- `NoteView.tsx`: el menú «Opciones de Enhanced» (línea 144) gana un submenú **Plantilla** con Auto y las plantillas (marcando la actual). Elegir una regenera con ella, pasando por el mismo aviso si la nota está editada. El botón «Enhance notes» de la primera vez sigue usando Auto, o la plantilla ya fijada en la nota.
- Aviso discreto cuando hubo caída a Auto.
- Sin cambios en `EnhancedView` ni en `enhancedDoc.ts`: la salida sigue siendo `EnhancedNotes`.
- El editor de plantillas («Manage templates») queda para la fase 3.

### 5.4 Autotest

`JUANOLA_AUTOTEST`: añadir un paso que regenera la nota del flujo con «Reunión semanal de equipo» y comprueba en el log que los títulos son un subconjunto ordenado de los de la plantilla y que no hubo caída a Auto.

## 6. Cómo se comprueba que sirven

### 6.1 Tests unitarios (sin modelo)

- `fit_to_template`: títulos exactos, con tildes cambiadas, parecidos, inventados (van a su apartado por palabras o a «Otros temas»), tareas a `Tasks`, orden final de la plantilla, apartados vacíos con «No se habló de esto» localizado y sin `source`.
- Mensaje de reintento: nombra los títulos que escribió el modelo y los que esperaba.
- Notas del usuario (§4.5): `user_blocks` con listas de varios elementos, anidadas y de tareas; tarea del usuario en el apartado de tareas; nota bajo un título del usuario en el apartado que le corresponde; nota sin encaje en «Otros temas» con plantilla y en «Tus notas» sin ella; apartado con solo notas del usuario sin «No se habló de esto».
- Lectura de `structure` antigua en texto libre.
- `validate`: cada regla.
- Reintento y caída: un `ChatModel` falso que devuelve basura la primera vez y Markdown válido la segunda, y otro que siempre devuelve basura (debe acabar en Auto con `template_fallback`).
- Plantillas integradas: válidas y con los seis idiomas.

### 6.2 Evaluación con el modelo (`eval_enhance.rs`)

- Opción `--template <id|archivo.json>`.
- `facts.json` admite un campo opcional `section` por hecho y plantilla (`{"semanal": "Decisiones"}`). Métrica nueva: **colocación**, el porcentaje de hechos cubiertos que están en el apartado esperado.
- Más métricas: viñetas en «Otros temas», apartados vacíos, reintentos, caídas a Auto, y cobertura frente a la misma reunión en Auto.
- Notas del usuario: cada fixture lleva su `notes.txt` con listas, tareas `[ ]`, un título y una nota en negrita, y `facts.json` indica el apartado esperado de cada nota por plantilla (`"user_notes": {"llamar a Luis": {"semanal": "Próximos pasos"}}`). Métrica nueva: **colocación de notas del usuario**. Para medir si las notas guían el resumen: cobertura de los hechos relacionados con las notas `[importante]`, con y sin notas.
- `facts.json` puede marcar apartados que deben quedar vacíos en una reunión (`"empty": {"1on1": ["Feedback"]}`), para comprobar que el modelo no los rellena con contenido de otro apartado.
- Fixtures: la reunión de producto actual (encaja con «Reunión semanal de equipo») y tres guiones nuevos de 8-10 min **solo en texto**: un 1:1, una llamada de ventas y un stand-up, cada uno con su `facts.json`. Un script `tools/script_to_segments.py` convierte `Hablante|texto` en `segments.json` con tiempos estimados (~2,5 palabras/s), sin TTS ni STT, porque lo que se evalúa es el enhance.
- Caso de desajuste: aplicar «Entrevista con cliente» a la reunión de producto. No debe inventar contenido; lo que no encaje va a «Otros temas», y si queda vacía más de la mitad de la plantilla debe caer a Auto (§4.4).
- 3 semillas por combinación; se reporta la mediana.

### 6.3 Criterios para darlo por bueno

| Métrica | Umbral |
|---|---|
| Títulos de la salida = títulos de la plantilla, en orden | 100 % (por construcción) |
| Cobertura de hechos con plantilla frente a Auto | no más de 5 puntos por debajo |
| Colocación de hechos en su apartado | ≥ 80 % |
| Tareas con responsable | igual o mejor que Auto |
| Viñetas en «Otros temas» con la plantilla adecuada | ≤ 10 % |
| Apartados vacíos que deberían tener contenido | ≤ 1 por nota |
| Notas del usuario en su apartado esperado | ≥ 90 % (las tareas del usuario, 100 %) |
| Notas del usuario fusionadas o perdidas | 0 |
| Apartados que deberían quedar vacíos y se rellenan | 0 |
| Caídas a Auto | 0 en las plantillas integradas con su reunión; 100 % en el caso de desajuste si queda vacía más de la mitad |
| Tiempo | ≤ 1,3 × Auto |

Si la colocación no llega al 80 %, antes de tocar el modelo se prueban dos variantes en `eval_enhance.rs`: (a) poner los `hint` más concretos; (b) una gramática GBNF **sobre Markdown** (no JSON) que fija los títulos `## …` de la plantilla y deja libre el texto de las viñetas. La variante (b) no se ha probado; la mala experiencia con JSON no garantiza que vaya igual de mal, pero tampoco que vaya bien.

### 6.4 Resultados

Las iteraciones de la primera evaluación (8-9 nov) están en [experimentos.md](experimentos.md), «Plantillas: iteraciones», y el resumen de lo adoptado y descartado, en su tabla «Enhance · plantillas».

## 7. Fases

0. **Arreglo de las notas del usuario** (§4.5, paso 0), *hecho*: `user_blocks` por elemento, con tipo y énfasis, y sus tests. Va antes que todo porque afecta también a Auto, que es lo que usa hoy la app.
1. **Núcleo y evaluación** (*hecho*; la calidad se sigue midiendo en [experimentos.md](experimentos.md)): §2, §3, §4 y §6 (tests unitarios, `--template`, fixtures en texto). Termina con la tabla de §6.3. Si no se cumplen los umbrales, se itera aquí; la app no se toca.
2. **Conexión con la app** (*hecha el 12 nov; `JUANOLA_AUTOTEST=1` en la .app: todo OK, el enhance con «Reunión semanal de equipo» tardó 9,9 s en la reunión del autotest, sin reintentos ni caída a Auto*): `repo::templates` (solo las del usuario; las integradas las combina `list_templates`) y `notes::set_template`; `list_templates(language)` y `enhance_note(id, template)`, donde `template` es «auto», un id o nada (la de la nota), porque un `Option<Option<Id>>` no distingue `null` de ausente al deserializar; el aviso va en `note://status` como `notice: "template_fallback"` y lo traduce el frontend. Submenú «Plantilla» en las opciones del resumen y paso `templateStep` en `JUANOLA_AUTOTEST=1`. En el mock, `?template=fallback` muestra el aviso.
3. **Plantillas del usuario** (*hecha el 12 nov*): «Manage templates» (crear, duplicar una integrada, editar apartados con título, `hint` y tipo, borrar), `save_template` y `delete_template` con `validate`, y el menú `/` del editor. El encaje del §4.2 hace que una plantilla mal escrita por el usuario no rompa la nota.
   - **Dónde:** Ajustes › Plantillas (`TemplatesSettings.tsx`), con «Gestionar plantillas…» al final del submenú. El editor repite las reglas de `validate` en el frontend para dar el error en el idioma de la interfaz; el backend vuelve a validar. Al borrar una plantilla, sus notas vuelven a Auto (los resúmenes ya hechos no cambian).
   - **Menú `/`:** en una línea vacía de las notas, «/» y lo que siga filtra las plantillas por nombre; la elegida sustituye la línea por sus apartados como títulos `###` y queda fijada en la nota (`set_note_template`), así que «Generar resumen» la usa. Los títulos salen en el idioma de la interfaz.
   - **Autotest:** `JUANOLA_AUTOTEST=ui` termina con `templatesFlow` (duplicar, validación, guardar, «/», borrar y limpieza), contra la base de datos real y sin modelo.
   - El editor de notas no tenía estilo para los títulos (`#`, `##`, `###` se veían como texto normal): ahora se pintan como los del resumen.

## 8. Cómo lo hacen otros (revisado el 8 nov 2026)

| | Char / Hyprnote (`fastrepl/anarlog`) | Meetily |
|---|---|---|
| Modelo | El del usuario (Ollama, LM Studio, nube); por defecto HyprLLM, fine-tune de Qwen3-1.7B | El del usuario (Ollama, nube) |
| Plantilla | Apartados con título y descripción | Apartados con título, instrucción, formato (`list`, `paragraph`, `string`) y formato por elemento |
| Salida | Markdown, apartados `# Título` | Markdown, apartados `**Título**` |
| Estructura | Prompt + comprobación en streaming del primer título (Levenshtein ≤ 30 %) + hasta 2 reintentos con el motivo del fallo | Solo el prompt |
| Apartado vacío | «Breve, sin inventar» | «None noted in this section.» |
| Idioma | Directamente en el idioma pedido | Resume en inglés y traduce; si falla, devuelve inglés |
| Citas a la transcripción | No | No |

Lo que se toma de ellos: Markdown en vez de JSON y apartados con título más instrucción (todos); el reintento con el motivo del fallo (Char); mostrar los apartados vacíos (Char y Meetily). El formato por apartado de Meetily (`list` o `paragraph`) queda para la fase 3. Lo que va más allá: el encaje determinista de `fit_to_template`, que ellos no necesitan porque cuentan con modelos grandes, y las citas por viñeta.

Fuentes: [prompts de Char](https://github.com/fastrepl/anarlog/tree/main/crates/template-app/assets), [validador de Char](https://github.com/fastrepl/anarlog/blob/main/apps/desktop/src/store/zustand/ai-task/task-configs/enhance-validator.ts), [procesador de Meetily](https://github.com/Zackriya-Solutions/meetily/blob/main/frontend/src-tauri/src/summary/processor.rs), [tipos de plantilla de Meetily](https://github.com/Zackriya-Solutions/meetily/blob/main/frontend/src-tauri/src/summary/templates/types.rs), [Launch HN de Hyprnote](https://hn.svelte.dev/item/44725306).

## 9. Riesgos

- **El modelo cambia los títulos o los traduce.** Lo absorbe `fit_to_template`; lo que se mide es la colocación.
- **Plantilla y reunión que no tienen nada que ver.** Si queda vacía más de la mitad de la plantilla, se reintenta y se cae a Auto con aviso (§4.4). Si encaja en parte, lo que sobra va a «Otros temas». No se fuerza el contenido en apartados que no le corresponden.
- **Prompt más largo.** La plantilla añade ~100-200 tokens; despreciable frente a la transcripción.
- **Títulos de las integradas en otros idiomas.** Escritos a mano y con test, pero conviene que los revise un hablante nativo de cada idioma.
- **Plantillas del usuario con títulos ambiguos** («Varios», «Notas»): la validación no los detecta; el reparto por `hint` y «Otros temas» lo amortigua.
