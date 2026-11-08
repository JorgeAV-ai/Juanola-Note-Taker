//! Repositorios por agregado. Consultas en tiempo de ejecución (`sqlx::query`),
//! sin macros, para no necesitar `DATABASE_URL` al compilar.

pub mod folders;
pub mod notes;
pub mod segments;
pub mod workspaces;

pub mod people {}
pub mod templates {}
pub mod recipes {}
pub mod chats {}
pub mod settings {}
