//! Estado de los permisos de audio del sistema operativo.
//!
//! Si macOS deniega el micrófono, Core Audio no da error: entrega silencio. Por
//! eso se consulta el permiso antes de grabar. Para el audio del sistema
//! (process tap) no hay API pública de consulta; la app lo deduce del audio
//! (callbacks que solo traen ceros).

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Granted,
    Denied,
    /// Aún no se ha preguntado: macOS lo pedirá al abrir el micrófono.
    NotDetermined,
    /// Plataforma sin consulta, o la consulta falló.
    Unknown,
}

/// Permiso de micrófono de este proceso (en macOS, el de la .app que lo lanza).
pub fn mic_permission() -> Permission {
    imp::mic_permission()
}

/// Pide el permiso de micrófono si aún no se ha decidido y espera la respuesta
/// del usuario (como mucho `timeout`). Abrir el micrófono con el diálogo de
/// macOS en pantalla bloquea Core Audio hasta que se responde; pedirlo antes
/// evita confundir esa espera con un dispositivo colgado.
pub fn request_mic_permission(timeout: std::time::Duration) -> Permission {
    match imp::mic_permission() {
        Permission::NotDetermined => imp::request_mic(timeout),
        other => other,
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::Permission;
    use objc2::msg_send;
    use objc2::runtime::AnyClass;
    use objc2_foundation::NSString;

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {
        static AVMediaTypeAudio: &'static NSString;
    }

    pub fn request_mic(timeout: std::time::Duration) -> Permission {
        let Some(cls) = AnyClass::get(c"AVCaptureDevice") else { return Permission::Unknown };
        let (tx, rx) = std::sync::mpsc::channel();
        let block = block2::RcBlock::new(move |granted: objc2::runtime::Bool| {
            let _ = tx.send(granted.as_bool());
        });
        let () = unsafe { msg_send![cls, requestAccessForMediaType: AVMediaTypeAudio, completionHandler: &*block] };
        match rx.recv_timeout(timeout) {
            Ok(true) => Permission::Granted,
            Ok(false) => Permission::Denied,
            Err(_) => Permission::NotDetermined,
        }
    }

    pub fn mic_permission() -> Permission {
        let Some(cls) = AnyClass::get(c"AVCaptureDevice") else { return Permission::Unknown };
        // AVAuthorizationStatus: 0 sin decidir, 1 restringido, 2 denegado, 3 concedido.
        let status: isize = unsafe { msg_send![cls, authorizationStatusForMediaType: AVMediaTypeAudio] };
        match status {
            0 => Permission::NotDetermined,
            1 | 2 => Permission::Denied,
            3 => Permission::Granted,
            _ => Permission::Unknown,
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::Permission;

    pub fn mic_permission() -> Permission {
        Permission::Unknown
    }

    pub fn request_mic(_timeout: std::time::Duration) -> Permission {
        Permission::Unknown
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn consulta_no_falla() {
        // Desde un binario de test el valor depende del terminal; solo se
        // comprueba que la llamada a AVFoundation funciona.
        let p = super::mic_permission();
        assert_ne!(p, super::Permission::Unknown, "{p:?}");
    }
}
