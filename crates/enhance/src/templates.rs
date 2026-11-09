//! Plantillas integradas: definidas en código (se versionan con la app y no
//! hace falta sembrarlas), con UUID fijos para que `notes.template_id` las
//! referencie. Los títulos están escritos a mano en los seis idiomas del
//! selector y se usan los del idioma del resumen; el propósito, el estilo y
//! las descripciones solo van al prompt, que está en español.

use juanola_core_types::{Id, SectionKind, Template, TemplateSection};

use crate::language::language_index;

pub const ONE_ON_ONE: Id = Id::from_u128(0x6a75_616e_6f6c_4100_8000_0000_0000_0001);
pub const STAND_UP: Id = Id::from_u128(0x6a75_616e_6f6c_4100_8000_0000_0000_0002);
pub const WEEKLY_TEAM: Id = Id::from_u128(0x6a75_616e_6f6c_4100_8000_0000_0000_0003);
pub const CUSTOMER_INTERVIEW: Id = Id::from_u128(0x6a75_616e_6f6c_4100_8000_0000_0000_0004);
pub const SALES_CALL: Id = Id::from_u128(0x6a75_616e_6f6c_4100_8000_0000_0000_0005);

/// Texto en los seis idiomas, en el orden de `language::LANGUAGES`:
/// español, inglés, portugués, francés, alemán, italiano.
type L6 = [&'static str; 6];

struct Builtin {
    id: Id,
    name: L6,
    purpose: &'static str,
    style: &'static str,
    sections: &'static [(L6, &'static str, SectionKind)],
}

const NEXT_STEPS: (L6, &str, SectionKind) = (
    ["Próximos pasos", "Next steps", "Próximos passos", "Prochaines étapes", "Nächste Schritte", "Prossimi passi"],
    "cada tarea: quién, qué y cuándo",
    SectionKind::Tasks,
);

const TOPICS: L6 = ["Temas tratados", "Topics discussed", "Temas discutidos", "Sujets abordés", "Besprochene Themen", "Argomenti trattati"];

const BUILTINS: &[Builtin] = &[
    Builtin {
        id: ONE_ON_ONE,
        name: ["1:1", "1:1", "1:1", "1:1", "1:1", "1:1"],
        purpose: "reunión individual entre una persona y su responsable",
        style: "cercano y concreto",
        sections: &[
            (
                ["Cómo está", "How they're doing", "Como está", "Comment ça va", "Wie es geht", "Come sta"],
                "estado de ánimo, carga de trabajo y preocupaciones de la persona",
                SectionKind::Topics,
            ),
            (TOPICS, "asuntos de trabajo de los que se habló, con sus decisiones y cifras", SectionKind::Topics),
            (
                ["Feedback", "Feedback", "Feedback", "Retours", "Feedback", "Feedback"],
                "solo opiniones explícitas sobre cómo trabaja alguien: elogios, críticas o sugerencias de mejora; si nadie dio ninguna, va vacío",
                SectionKind::Topics,
            ),
            NEXT_STEPS,
        ],
    },
    Builtin {
        id: STAND_UP,
        name: ["Stand-up", "Stand-up", "Stand-up", "Stand-up", "Stand-up", "Stand-up"],
        purpose: "reunión diaria breve de seguimiento del equipo",
        style: "breve, por persona",
        sections: &[
            (
                ["Terminado", "Done", "Concluído", "Terminé", "Erledigt", "Completato"],
                // «Hecho desde la última» se medía peor: el modelo lo leía como «un hecho»
                // y mandaba lo terminado a «Otros temas» (colocación 36 % frente a 67 %).
                "solo lo que cada persona ya terminó o dejó listo antes de esta reunión (ayer terminé, arreglé, cerré)",
                SectionKind::Topics,
            ),
            (
                ["Plan de hoy", "Today's plan", "Plano de hoje", "Plan du jour", "Plan für heute", "Piano di oggi"],
                "solo lo que cada persona va a hacer hoy (hoy empiezo, hoy sigo), no lo ya terminado ni lo que la frena",
                SectionKind::Topics,
            ),
            (
                ["Bloqueos", "Blockers", "Bloqueios", "Blocages", "Blocker", "Blocchi"],
                "lo que impide avanzar o se está esperando de otros, y quién puede ayudar",
                SectionKind::Topics,
            ),
            NEXT_STEPS,
        ],
    },
    Builtin {
        id: WEEKLY_TEAM,
        name: ["Reunión semanal de equipo", "Weekly team meeting", "Reunião semanal de equipe", "Réunion d'équipe hebdomadaire", "Wöchentliches Team-Meeting", "Riunione settimanale del team"],
        purpose: "reunión semanal del equipo para revisar el estado y decidir",
        style: "conciso, con las cifras exactas",
        sections: &[
            (
                ["Métricas y estado", "Metrics and status", "Métricas e status", "Indicateurs et état", "Kennzahlen und Status", "Metriche e stato"],
                "cifras, resultados y cómo van los proyectos",
                SectionKind::Topics,
            ),
            (TOPICS, "asuntos de los que se habló y lo que se dijo de cada uno", SectionKind::Topics),
            (
                ["Decisiones", "Decisions", "Decisões", "Décisions", "Entscheidungen", "Decisioni"],
                "lo que se acordó o decidió en la reunión",
                SectionKind::Topics,
            ),
            NEXT_STEPS,
        ],
    },
    Builtin {
        id: CUSTOMER_INTERVIEW,
        name: ["Entrevista con cliente", "Customer interview", "Entrevista com cliente", "Entretien client", "Kundeninterview", "Intervista al cliente"],
        purpose: "entrevista para entender a un cliente o usuario",
        style: "fiel a lo que dijo el cliente",
        sections: &[
            (
                ["Contexto del cliente", "Customer context", "Contexto do cliente", "Contexte du client", "Kontext des Kunden", "Contesto del cliente"],
                "quién es, su empresa, su función y su situación",
                SectionKind::Topics,
            ),
            (
                ["Problemas y necesidades", "Problems and needs", "Problemas e necessidades", "Problèmes et besoins", "Probleme und Bedürfnisse", "Problemi ed esigenze"],
                "qué le cuesta al cliente y qué necesita resolver",
                SectionKind::Topics,
            ),
            (
                ["Lo que usan hoy", "What they use today", "O que usam hoje", "Ce qu'ils utilisent aujourd'hui", "Was sie heute nutzen", "Cosa usano oggi"],
                "herramientas, procesos o competidores que usa ahora",
                SectionKind::Topics,
            ),
            (
                ["Citas destacables", "Notable quotes", "Citações de destaque", "Citations marquantes", "Bemerkenswerte Zitate", "Citazioni notevoli"],
                "frases del cliente que vale la pena recordar",
                SectionKind::Topics,
            ),
            NEXT_STEPS,
        ],
    },
    Builtin {
        id: SALES_CALL,
        name: ["Llamada de ventas", "Sales call", "Chamada de vendas", "Appel commercial", "Verkaufsgespräch", "Chiamata di vendita"],
        purpose: "llamada comercial con un posible cliente",
        style: "concreto, con cifras y fechas",
        sections: &[
            // «Contexto y necesidades» y «Objeciones» se intercambiaban: las dudas del
            // cliente sobre la oferta iban a contexto y sus problemas a objeciones
            // (colocación 71 % frente a 100 %).
            (
                ["Situación del cliente", "Customer situation", "Situação do cliente", "Situation du client", "Situation des Kunden", "Situazione del cliente"],
                "a qué se dedica el cliente, cómo trabaja hoy, qué le cuesta y qué objetivo tiene",
                SectionKind::Topics,
            ),
            (
                ["Dudas y reparos", "Concerns and objections", "Dúvidas e objeções", "Doutes et réserves", "Bedenken und Einwände", "Dubbi e riserve"],
                "lo que preocupa al cliente de la solución que se le ofrece: integración, datos, adopción por su equipo o la competencia",
                SectionKind::Topics,
            ),
            (
                ["Presupuesto y plazos", "Budget and timeline", "Orçamento e prazos", "Budget et délais", "Budget und Zeitplan", "Budget e tempistiche"],
                "presupuesto, precios, quién decide y fechas",
                SectionKind::Topics,
            ),
            NEXT_STEPS,
        ],
    },
];

fn build(b: &Builtin, lang: usize) -> Template {
    Template {
        id: b.id,
        name: b.name[lang].into(),
        purpose: b.purpose.into(),
        style: b.style.into(),
        structure: b
            .sections
            .iter()
            .map(|(h, hint, kind)| TemplateSection { heading: h[lang].into(), hint: (*hint).into(), kind: *kind })
            .collect(),
        is_builtin: true,
        shared_with_workspace: false,
    }
}

/// Las plantillas integradas con los títulos en `language` (el nombre que da
/// [`crate::language::summary_language`]; español si no se reconoce).
pub fn builtin_templates(language: &str) -> Vec<Template> {
    let lang = language_index(language);
    BUILTINS.iter().map(|b| build(b, lang)).collect()
}

/// Una plantilla integrada por id, con los títulos en `language`.
pub fn builtin_template(id: Id, language: &str) -> Option<Template> {
    let lang = language_index(language);
    BUILTINS.iter().find(|b| b.id == id).map(|b| build(b, lang))
}

pub fn is_builtin(id: Id) -> bool {
    BUILTINS.iter().any(|b| b.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::LANGUAGE_NAMES;

    #[test]
    fn builtins_are_valid_in_every_language() {
        assert_eq!(LANGUAGE_NAMES.len(), 6);
        for lang in LANGUAGE_NAMES {
            let all = builtin_templates(lang);
            assert_eq!(all.len(), BUILTINS.len());
            for t in &all {
                t.validate().unwrap_or_else(|e| panic!("{} en {lang}: {e}", t.name));
                assert!(t.structure.iter().all(|s| !s.heading.is_empty() && !s.hint.is_empty()));
                assert_eq!(t.structure.last().unwrap().kind, SectionKind::Tasks);
            }
        }
    }

    #[test]
    fn titles_follow_the_summary_language() {
        let en = builtin_template(WEEKLY_TEAM, "inglés").unwrap();
        assert_eq!(en.structure[2].heading, "Decisions");
        let es = builtin_template(WEEKLY_TEAM, "desconocido").unwrap();
        assert_eq!(es.structure[2].heading, "Decisiones");
        // Cada idioma tiene sus propios títulos (salvo los que coinciden de verdad).
        let fr = builtin_template(SALES_CALL, "francés").unwrap();
        assert_eq!(fr.structure[0].heading, "Situation du client");
    }

    #[test]
    fn ids_are_unique_and_fixed() {
        let ids: std::collections::HashSet<Id> = BUILTINS.iter().map(|b| b.id).collect();
        assert_eq!(ids.len(), BUILTINS.len());
        assert_eq!(ONE_ON_ONE.to_string(), "6a75616e-6f6c-4100-8000-000000000001");
        assert!(is_builtin(SALES_CALL) && !is_builtin(Id::nil()));
    }
}
