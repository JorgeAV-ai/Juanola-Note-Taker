# Plan: plantillas en el enhance

Fecha: 8 de noviembre de 2026. Estado: fases 0 y 1 construidas (núcleo, tests y evaluador); la evaluación no cumple aún todos los umbrales del §6.3 (ver §6.4). La app no está conectada.

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

Las notas crudas del usuario deben guiar el resumen, como en Granola: marcan lo que le importó. Hoy solo se pasan como contexto y se colocan por palabras en común, y tienen un fallo con las listas.

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

### 6.4 Resultados de la primera evaluación (8 nov)

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

Siguientes pasos de la fase 1, por orden:

1. ~~Salida explícita `## Otros temas`~~ (segunda iteración), ~~guiones nuevos~~ (tercera), ~~descripciones concretas~~ (cuarta) y ~~gramática GBNF~~ (quinta: no mejora).
2. ~~**Títulos**~~ (sexta: ventas pasa a 93 %; semanal sin mejora clara). Falta un guion de entrevista con cliente para medir esa plantilla.
3. **Viñetas copiadas** en reuniones cortas (47-65 % en el stand-up; ventas bajó a 26-33 % con los títulos nuevos): con plantilla se copia más que en Auto.
4. **Tiempo en reuniones cortas** (1,7 × Auto en el 1:1): bajar el mínimo de `template_budget` o el margen por apartado, comprobando que las tareas no vuelven a cortarse.
5. **Desajuste:** sigue sin detectarse.

## 7. Fases

0. **Arreglo de las notas del usuario** (§4.5, paso 0), *hecho*: `user_blocks` por elemento, con tipo y énfasis, y sus tests. Va antes que todo porque afecta también a Auto, que es lo que usa hoy la app.
1. **Núcleo y evaluación** (*núcleo, tests y evaluador hechos; iterando sobre la calidad, ver §6.4*): §2, §3, §4 y §6 (tests unitarios, `--template`, fixtures en texto). Termina con la tabla de §6.3. Si no se cumplen los umbrales, se itera aquí; la app no se toca.
2. **Conexión con la app:** §5.1, §5.2, §5.3 (submenú y aviso) y §5.4.
3. **Plantillas del usuario:** «Manage templates» (crear, duplicar una integrada, editar apartados con título, `hint` y tipo, borrar), `save_template` y `delete_template` con `validate`, y el menú `/` del editor. El encaje del §4.2 hace que una plantilla mal escrita por el usuario no rompa la nota.

## 8. Cómo lo hacen otros (revisado el 8 nov 2026)

| | Granola | Char / Hyprnote (`fastrepl/anarlog`) | Meetily |
|---|---|---|---|
| Modelo | Nube (OpenAI y Anthropic) | El del usuario (Ollama, LM Studio, nube); por defecto HyprLLM, fine-tune de Qwen3-1.7B | El del usuario (Ollama, nube) |
| Plantilla | Propósito, estilo y apartados | Apartados con título y descripción | Apartados con título, instrucción, formato (`list`, `paragraph`, `string`) y formato por elemento |
| Salida | Markdown | Markdown, apartados `# Título` | Markdown, apartados `**Título**` |
| Estructura | No publicado | Prompt + comprobación en streaming del primer título (Levenshtein ≤ 30 %) + hasta 2 reintentos con el motivo del fallo | Solo el prompt |
| Apartado vacío | No publicado | «Breve, sin inventar» | «None noted in this section.» |
| Idioma | Configurable | Directamente en el idioma pedido | Resume en inglés y traduce; si falla, devuelve inglés |
| Citas a la transcripción | Sí | No | No |

Lo que se toma de ellos: Markdown en vez de JSON y apartados con título más instrucción (todos); el reintento con el motivo del fallo (Char); mostrar los apartados vacíos (Char y Meetily). El formato por apartado de Meetily (`list` o `paragraph`) queda para la fase 3. Lo que va más allá: el encaje determinista de `fit_to_template`, que ellos no necesitan porque cuentan con modelos grandes, y las citas por viñeta.

Fuentes: [prompts de Char](https://github.com/fastrepl/anarlog/tree/main/crates/template-app/assets), [validador de Char](https://github.com/fastrepl/anarlog/blob/main/apps/desktop/src/store/zustand/ai-task/task-configs/enhance-validator.ts), [procesador de Meetily](https://github.com/Zackriya-Solutions/meetily/blob/main/frontend/src-tauri/src/summary/processor.rs), [tipos de plantilla de Meetily](https://github.com/Zackriya-Solutions/meetily/blob/main/frontend/src-tauri/src/summary/templates/types.rs), [plantillas de Granola](https://docs.granola.ai/help-center/taking-notes/customise-notes-with-templates), [Launch HN de Hyprnote](https://hn.svelte.dev/item/44725306).

## 9. Riesgos

- **El modelo cambia los títulos o los traduce.** Lo absorbe `fit_to_template`; lo que se mide es la colocación.
- **Plantilla y reunión que no tienen nada que ver.** Si queda vacía más de la mitad de la plantilla, se reintenta y se cae a Auto con aviso (§4.4). Si encaja en parte, lo que sobra va a «Otros temas». No se fuerza el contenido en apartados que no le corresponden.
- **Prompt más largo.** La plantilla añade ~100-200 tokens; despreciable frente a la transcripción.
- **Títulos de las integradas en otros idiomas.** Escritos a mano y con test, pero conviene que los revise un hablante nativo de cada idioma.
- **Plantillas del usuario con títulos ambiguos** («Varios», «Notas»): la validación no los detecta; el reparto por `hint` y «Otros temas» lo amortigua.
