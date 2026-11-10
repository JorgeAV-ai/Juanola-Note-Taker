# Documentación

Qué hay en cada documento y cuándo leerlo.

| Documento | Para qué sirve | Cuándo leerlo |
|---|---|---|
| [HANDOFF.md](HANDOFF.md) | Estado actual del proyecto: qué funciona, decisiones cerradas, normas de trabajo, cómo arrancar y probar, y lo que queda | Siempre, lo primero |
| [experimentos.md](experimentos.md) | Registro de todo lo medido (enhance, STT, audio): resultado y si se adoptó o descartó, con una tabla por área arriba y el detalle abajo | Antes de probar un cambio de prompt, modelo o pipeline, y después, para anotarlo |
| [plan.md](plan.md) | Plan de partida: inventario de Granola (qué replicar), arquitectura, slots de modelo y roadmap por fases con su estado | Para saber qué falta por replicar y en qué fase va |
| [modelos-mvp.md](modelos-mvp.md) | Qué modelo ocupa cada slot (LLM, STT, embeddings, VAD) y por qué; runtimes por plataforma | Antes de proponer o cambiar un modelo |
| [plantillas.md](plantillas.md) | Diseño de las plantillas del enhance: modelo de datos, encaje, reintentos, notas del usuario, criterios y fases | Para trabajar en plantillas (lo siguiente es conectarlas a la app) |
| [eval-dataset.md](eval-dataset.md) | Plan del dataset de evaluación en cuatro capas (STT corto, reuniones reales, sintéticas, propias) | Para ampliar la evaluación o validar con audio real |
| [historial.md](historial.md) | Cómo se construyó cada parte (6-8 nov): detalles, fallos encontrados y cómo se arreglaron | Cuando haga falta el porqué de algo del código |

`assets/` guarda el logo (`logo.svg`).
