//! Persistencia local en SQLite (sqlx) con migraciones embebidas.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    /// Una fila con datos que no se pueden interpretar (uuid o fecha mal formados).
    #[error("fila corrupta: {0}")]
    Corrupt(String),
    /// Datos que no se pueden guardar (por ejemplo, una plantilla que no pasa `validate`).
    #[error("{0}")]
    Invalid(String),
}

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Abre (o crea) la base de datos y aplica las migraciones de `./migrations`.
    pub async fn open(path: &Path) -> Result<Self, DbError> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new().max_connections(8).connect_with(opts).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

pub mod repo;

#[cfg(test)]
mod tests {
    use super::*;
    use juanola_core_types::{Channel, NoteStatus, Segment};

    #[tokio::test]
    async fn notes_and_segments_roundtrip() {
        let dir = std::env::temp_dir().join(format!("juanola-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir.join("t.sqlite")).await.unwrap();
        let ws = repo::workspaces::ensure_personal(&db).await.unwrap();
        repo::workspaces::ensure_personal(&db).await.unwrap(); // idempotente
        let n = repo::notes::create(&db, ws, "Reunión").await.unwrap();
        repo::notes::set_title(&db, n.id, "Reunión semanal").await.unwrap();
        repo::notes::set_status(&db, n.id, NoteStatus::Ready).await.unwrap();
        repo::notes::set_enhanced_doc(&db, n.id, Some(&serde_json::json!({"sections": []}))).await.unwrap();
        let seg = Segment {
            id: uuid::Uuid::new_v4(),
            note_id: n.id,
            channel: Channel::System,
            speaker_id: None,
            start: 1.5,
            end: 3.0,
            text: "hola".into(),
            confidence: None,
            is_final: true,
        };
        repo::segments::insert(&db, &seg).await.unwrap();
        let got = repo::notes::get(&db, n.id).await.unwrap().unwrap();
        assert_eq!(got.title, "Reunión semanal");
        assert_eq!(got.status, NoteStatus::Ready);
        assert!(got.enhanced_doc.is_some());
        assert_eq!(repo::notes::list_recent(&db, 10).await.unwrap().len(), 1);
        let segs = repo::segments::list(&db, n.id).await.unwrap();
        assert_eq!((segs.len(), segs[0].channel, segs[0].text.as_str()), (1, Channel::System, "hola"));
        assert_eq!(repo::segments::last_end(&db, n.id).await.unwrap(), 3.0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn folders_and_trash() {
        let dir = std::env::temp_dir().join(format!("juanola-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir.join("t.sqlite")).await.unwrap();
        let ws = repo::workspaces::ensure_personal(&db).await.unwrap();
        let a = repo::notes::create(&db, ws, "a").await.unwrap();
        let b = repo::notes::create(&db, ws, "b").await.unwrap();
        let calls = repo::folders::create(&db, ws, "Customer calls").await.unwrap();
        let other = repo::folders::create(&db, ws, "Otra").await.unwrap();
        repo::folders::set_note_folder(&db, a.id, Some(calls.id)).await.unwrap();
        repo::folders::set_note_folder(&db, b.id, Some(calls.id)).await.unwrap();
        // Mover a otra carpeta la quita de la primera.
        repo::folders::set_note_folder(&db, b.id, Some(other.id)).await.unwrap();
        assert_eq!(repo::folders::note_folder(&db, b.id).await.unwrap(), Some(other.id));
        let list = repo::folders::list(&db, ws).await.unwrap();
        assert_eq!(list.iter().map(|f| (f.name.as_str(), f.note_count)).collect::<Vec<_>>(), [("Customer calls", 1), ("Otra", 1)]);
        repo::folders::rename(&db, other.id, "Clientes").await.unwrap();

        // Papelera: fuera de las listas y de los recuentos; se restaura.
        repo::notes::trash(&db, a.id).await.unwrap();
        assert_eq!(repo::notes::list_recent(&db, 10).await.unwrap().len(), 1);
        assert!(repo::folders::notes(&db, calls.id).await.unwrap().is_empty());
        assert_eq!(repo::folders::list(&db, ws).await.unwrap()[0].note_count, 0);
        assert_eq!(repo::notes::list_trashed(&db).await.unwrap()[0].id, a.id);
        repo::notes::restore(&db, a.id).await.unwrap();
        assert_eq!(repo::folders::notes(&db, calls.id).await.unwrap()[0].id, a.id);

        // Borrar una carpeta deja sus notas sin carpeta.
        repo::folders::delete(&db, calls.id).await.unwrap();
        assert_eq!(repo::folders::note_folder(&db, a.id).await.unwrap(), None);
        assert_eq!(repo::notes::list_recent(&db, 10).await.unwrap().len(), 2);

        // Borrar para siempre solo desde la papelera.
        repo::notes::delete_forever(&db, b.id).await.unwrap();
        assert!(repo::notes::get(&db, b.id).await.unwrap().is_some());
        repo::notes::trash(&db, b.id).await.unwrap();
        repo::notes::delete_forever(&db, b.id).await.unwrap();
        assert!(repo::notes::get(&db, b.id).await.unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn settings_and_calendar_events() {
        use juanola_core_types::{Attendee, CalendarEvent};
        let dir = std::env::temp_dir().join(format!("juanola-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir.join("t.sqlite")).await.unwrap();
        assert_eq!(repo::settings::get::<bool>(&db, "detect.enabled").await.unwrap(), None);
        repo::settings::set(&db, "detect.enabled", &false).await.unwrap();
        repo::settings::set(&db, "detect.enabled", &true).await.unwrap();
        assert_eq!(repo::settings::get::<bool>(&db, "detect.enabled").await.unwrap(), Some(true));

        let start = chrono::Utc::now();
        let mut ev = CalendarEvent {
            id: "apple:X:1".into(),
            provider: "apple".into(),
            calendar_id: "c".into(),
            title: "Sync".into(),
            start,
            end: start + chrono::Duration::minutes(30),
            attendees: vec![Attendee { email: "a@x.com".into(), name: Some("Ana".into()), declined: false, is_self: false }],
            conference_url: Some("https://meet.google.com/abc".into()),
            recurring_event_id: None,
            description: None,
        };
        repo::events::upsert(&db, &ev).await.unwrap();
        ev.title = "Sync semanal".into();
        repo::events::upsert(&db, &ev).await.unwrap();
        let got = repo::events::get(&db, &ev.id).await.unwrap().unwrap();
        assert_eq!((got.title.as_str(), got.attendees.len()), ("Sync semanal", 1));
        assert_eq!(got.start.timestamp(), start.timestamp());

        let ws = repo::workspaces::ensure_personal(&db).await.unwrap();
        let n = repo::notes::create(&db, ws, "Sync").await.unwrap();
        assert!(repo::notes::find_by_event(&db, &ev.id).await.unwrap().is_none());
        repo::notes::set_calendar_event(&db, n.id, Some(&ev.id)).await.unwrap();
        assert_eq!(repo::notes::find_by_event(&db, &ev.id).await.unwrap().unwrap().id, n.id);
        repo::notes::trash(&db, n.id).await.unwrap();
        assert!(repo::notes::find_by_event(&db, &ev.id).await.unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn templates_and_note_template() {
        use juanola_core_types::{SectionKind, Template, TemplateSection};
        let dir = std::env::temp_dir().join(format!("juanola-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir.join("t.sqlite")).await.unwrap();
        let sec = |h: &str, kind| TemplateSection { heading: h.into(), hint: "pista".into(), kind };
        let mut t = Template {
            id: uuid::Uuid::new_v4(),
            name: "Retro".into(),
            purpose: "retrospectiva".into(),
            style: "breve".into(),
            structure: vec![sec("Bien", SectionKind::Topics), sec("Acciones", SectionKind::Tasks)],
            is_builtin: false,
            shared_with_workspace: false,
        };
        repo::templates::upsert(&db, &t).await.unwrap();
        t.name = "Retrospectiva".into();
        repo::templates::upsert(&db, &t).await.unwrap();
        let got = repo::templates::get(&db, t.id).await.unwrap().unwrap();
        assert_eq!((got.name.as_str(), got.structure.clone()), ("Retrospectiva", t.structure.clone()));
        assert_eq!(repo::templates::list(&db).await.unwrap().len(), 1);

        // Una plantilla inválida o integrada no se guarda.
        let mut bad = t.clone();
        bad.id = uuid::Uuid::new_v4();
        bad.structure.clear();
        assert!(matches!(repo::templates::upsert(&db, &bad).await, Err(DbError::Invalid(_))));
        let mut builtin = t.clone();
        builtin.is_builtin = true;
        assert!(repo::templates::upsert(&db, &builtin).await.is_err());

        // La nota guarda la plantilla; al borrarla vuelve a Auto.
        let ws = repo::workspaces::ensure_personal(&db).await.unwrap();
        let n = repo::notes::create(&db, ws, "Retro").await.unwrap();
        assert_eq!(n.template_id, None);
        repo::notes::set_template(&db, n.id, Some(t.id)).await.unwrap();
        assert_eq!(repo::notes::get(&db, n.id).await.unwrap().unwrap().template_id, Some(t.id));
        repo::templates::delete(&db, t.id).await.unwrap();
        assert!(repo::templates::get(&db, t.id).await.unwrap().is_none());
        assert_eq!(repo::notes::get(&db, n.id).await.unwrap().unwrap().template_id, None);

        // Asignar una plantilla del usuario que ya no existe no toca la nota.
        assert!(!repo::notes::set_user_template(&db, n.id, t.id).await.unwrap());
        assert_eq!(repo::notes::get(&db, n.id).await.unwrap().unwrap().template_id, None);
        repo::templates::upsert(&db, &t).await.unwrap();
        assert!(repo::notes::set_user_template(&db, n.id, t.id).await.unwrap());
        assert_eq!(repo::notes::get(&db, n.id).await.unwrap().unwrap().template_id, Some(t.id));
        assert!(!repo::notes::set_user_template(&db, uuid::Uuid::new_v4(), t.id).await.unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }
}
