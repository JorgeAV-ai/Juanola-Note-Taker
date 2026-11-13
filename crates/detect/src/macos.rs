//! Procesos con la entrada de audio abierta, vía los objetos de proceso de
//! Core Audio (macOS 14.2+): `kAudioHardwarePropertyProcessObjectList` da un
//! objeto por proceso cliente de coreaudiod, y cada uno dice su PID, su bundle
//! id y si está grabando (`kAudioProcessPropertyIsRunningInput`). No pide
//! permisos: solo se lee el estado, no el audio.

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::NonNull;

use objc2_core_audio::{
    AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID, AudioObjectPropertyAddress, AudioObjectPropertyScope,
    AudioObjectPropertySelector, kAudioDevicePropertyDeviceIsRunningSomewhere, kAudioDevicePropertyNominalSampleRate,
    kAudioDevicePropertyStreams, kAudioDevicePropertyTransportType, kAudioDeviceTransportTypeBluetooth,
    kAudioDeviceTransportTypeBluetoothLE, kAudioHardwarePropertyDefaultInputDevice, kAudioHardwarePropertyDevices,
    kAudioHardwarePropertyProcessObjectList, kAudioObjectPropertyElementMain, kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal,
    kAudioObjectPropertyScopeInput, kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject, kAudioProcessPropertyBundleID,
    kAudioProcessPropertyDevices, kAudioProcessPropertyIsRunning, kAudioProcessPropertyIsRunningInput,
    kAudioProcessPropertyIsRunningOutput, kAudioProcessPropertyPID,
};
use objc2_core_foundation::{CFRetained, CFString};

use crate::MicUser;

fn address(selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress { mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain }
}

/// Lee una propiedad de tamaño fijo `T`.
fn get<T: Copy + Default>(object: AudioObjectID, selector: AudioObjectPropertySelector) -> Option<T> {
    let addr = address(selector);
    let mut value = T::default();
    let mut size = size_of::<T>() as u32;
    // SAFETY: los punteros apuntan a variables locales vivas y `size` es el de `value`.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast::<c_void>(),
        )
    };
    (status == 0 && size as usize == size_of::<T>()).then_some(value)
}

/// Lista de `AudioObjectID` de una propiedad de `object` en `scope`.
fn get_ids(object: AudioObjectID, selector: AudioObjectPropertySelector, scope: AudioObjectPropertyScope) -> Vec<AudioObjectID> {
    let addr = AudioObjectPropertyAddress { mSelector: selector, mScope: scope, mElement: kAudioObjectPropertyElementMain };
    let mut size = 0u32;
    // SAFETY: punteros a locales; sin datos de calificación.
    let status = unsafe { AudioObjectGetPropertyDataSize(object, NonNull::from(&addr), 0, std::ptr::null(), NonNull::from(&mut size)) };
    if status != 0 || size == 0 {
        return Vec::new();
    }
    // `size` podría no ser múltiplo de AudioObjectID: reservar de sobra para
    // que el búfer nunca sea más pequeño que lo que la API escribe.
    let mut ids = vec![0 as AudioObjectID; (size as usize).div_ceil(size_of::<AudioObjectID>())];
    // SAFETY: `ids` tiene al menos `size` bytes reservados.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new(ids.as_mut_ptr()).unwrap().cast::<c_void>(),
        )
    };
    if status != 0 {
        return Vec::new();
    }
    ids.truncate(size as usize / size_of::<AudioObjectID>());
    ids
}

/// Si el micrófono predeterminado lo está usando algún proceso.
pub(crate) fn default_input_running() -> bool {
    let system = kAudioObjectSystemObject as AudioObjectID;
    get::<AudioObjectID>(system, kAudioHardwarePropertyDefaultInputDevice).is_some_and(device_running)
}

/// Dispositivos de entrada que usa el proceso (`kAudioProcessPropertyDevices` en ámbito de entrada).
fn input_devices(object: AudioObjectID) -> Vec<AudioObjectID> {
    get_ids(object, kAudioProcessPropertyDevices, kAudioObjectPropertyScopeInput)
}

fn process_objects() -> Vec<AudioObjectID> {
    let addr = address(kAudioHardwarePropertyProcessObjectList);
    let system = kAudioObjectSystemObject as AudioObjectID;
    let mut size = 0u32;
    // SAFETY: punteros a locales; sin datos de calificación.
    let status = unsafe { AudioObjectGetPropertyDataSize(system, NonNull::from(&addr), 0, std::ptr::null(), NonNull::from(&mut size)) };
    if status != 0 || size == 0 {
        return Vec::new();
    }
    // `size` podría no ser múltiplo de AudioObjectID: reservar de sobra para
    // que el búfer nunca sea más pequeño que lo que la API escribe.
    let mut ids = vec![0 as AudioObjectID; (size as usize).div_ceil(size_of::<AudioObjectID>())];
    // SAFETY: `ids` tiene al menos `size` bytes reservados.
    let status = unsafe {
        AudioObjectGetPropertyData(
            system,
            NonNull::from(&addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new(ids.as_mut_ptr()).unwrap().cast::<c_void>(),
        )
    };
    if status != 0 {
        return Vec::new();
    }
    ids.truncate(size as usize / size_of::<AudioObjectID>());
    ids
}

fn bundle_id(object: AudioObjectID) -> String {
    string_prop(object, kAudioProcessPropertyBundleID)
}

/// Propiedad de tipo CFString (Core Audio la devuelve retenida).
fn string_prop(object: AudioObjectID, selector: AudioObjectPropertySelector) -> String {
    let addr = address(selector);
    let mut value: *const CFString = std::ptr::null();
    let mut size = size_of::<*const CFString>() as u32;
    // SAFETY: Core Audio escribe un CFStringRef retenido (regla «Copy»).
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast::<c_void>(),
        )
    };
    if status != 0 {
        return String::new();
    }
    match NonNull::new(value as *mut CFString) {
        // SAFETY: lo devolvió retenido, así que lo liberamos al soltarlo.
        Some(ptr) => unsafe { CFRetained::from_raw(ptr) }.to_string(),
        None => String::new(),
    }
}

fn device_running(dev: AudioObjectID) -> bool {
    get::<u32>(dev, kAudioDevicePropertyDeviceIsRunningSomewhere).is_some_and(|v| v != 0)
}

fn has_streams(dev: AudioObjectID, scope: AudioObjectPropertyScope) -> bool {
    !get_ids(dev, kAudioDevicePropertyStreams, scope).is_empty()
}

fn transport(dev: AudioObjectID) -> u32 {
    get::<u32>(dev, kAudioDevicePropertyTransportType).unwrap_or(0)
}

/// Auriculares Bluetooth en perfil manos libres (HFP): macOS cambia a ese
/// perfil al abrir su micrófono, y la frecuencia baja de 44,1/48 kHz a 16 o
/// 24 kHz. En A2DP (solo escuchar) se queda alta.
fn bluetooth_hands_free(dev: AudioObjectID) -> bool {
    let t = transport(dev);
    (t == kAudioDeviceTransportTypeBluetooth || t == kAudioDeviceTransportTypeBluetoothLE)
        && get::<f64>(dev, kAudioDevicePropertyNominalSampleRate).is_some_and(|r| r > 0.0 && r <= 24_000.0)
}

/// Si el proceso está grabando. `IsRunningInput` no basta: en macOS 26 el
/// auxiliar de audio de Chrome (Meet) graba con `IsRunningInput = 0`. Medido
/// (8 nov 2026, ver el ejemplo `audio_processes`):
/// - Grabando, Chrome está en marcha y su lista de dispositivos de entrada
///   incluye el micrófono, que figura en uso.
/// - Solo reproduciendo, esa lista incluye igualmente los dispositivos mixtos
///   (entrada y salida, p. ej. BlackHole) por los que suena, y figuran «en
///   uso» por la salida. `kAudioStreamPropertyIsActive` vale 1 siempre y
///   `AVCaptureDevice.isInUseByAnotherApplication` dio falso con Chrome
///   grabando, así que ninguno de los dos ayuda.
///
/// Por eso un dispositivo mixto solo cuenta si el proceso no lo usa también
/// de salida, o si son auriculares Bluetooth en manos libres.
fn is_recording(obj: AudioObjectID) -> bool {
    if get::<u32>(obj, kAudioProcessPropertyIsRunningInput).unwrap_or(0) != 0 {
        return true;
    }
    if get::<u32>(obj, kAudioProcessPropertyIsRunning).unwrap_or(0) == 0 {
        return false;
    }
    let outputs = get_ids(obj, kAudioProcessPropertyDevices, kAudioObjectPropertyScopeOutput);
    input_devices(obj)
        .into_iter()
        .filter(|&d| device_running(d))
        .any(|d| !has_streams(d, kAudioObjectPropertyScopeOutput) || !outputs.contains(&d) || bluetooth_hands_free(d))
}

pub(crate) fn mic_users() -> Vec<MicUser> {
    process_objects()
        .into_iter()
        .filter(|&obj| is_recording(obj))
        .map(|obj| MicUser { pid: get::<i32>(obj, kAudioProcessPropertyPID).unwrap_or(-1), bundle_id: bundle_id(obj) })
        .collect()
}

/// Todos los procesos cliente de Core Audio con sus indicadores (diagnóstico).
pub(crate) fn audio_processes() -> Vec<crate::AudioProcess> {
    process_objects()
        .into_iter()
        .map(|obj| crate::AudioProcess {
            pid: get::<i32>(obj, kAudioProcessPropertyPID).unwrap_or(-1),
            bundle_id: bundle_id(obj),
            running: get::<u32>(obj, kAudioProcessPropertyIsRunning).unwrap_or(0) != 0,
            input: get::<u32>(obj, kAudioProcessPropertyIsRunningInput).unwrap_or(0) != 0,
            output: get::<u32>(obj, kAudioProcessPropertyIsRunningOutput).unwrap_or(0) != 0,
            input_devices: input_devices(obj).len(),
        })
        .collect()
}

/// Diagnóstico: cada dispositivo con nombre, transporte (fourcc), nº de
/// streams de entrada y salida y si está en uso.
pub(crate) fn describe_devices() -> Vec<String> {
    let system = kAudioObjectSystemObject as AudioObjectID;
    get_ids(system, kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal)
        .into_iter()
        .map(|d| {
            let t = get::<u32>(d, kAudioDevicePropertyTransportType).unwrap_or(0).to_be_bytes();
            let rate = get::<f64>(d, kAudioDevicePropertyNominalSampleRate).unwrap_or(0.0);
            format!(
                "{d:>4} {:<4} {rate:>6} Hz in={} out={} running={} {}",
                String::from_utf8_lossy(&t),
                get_ids(d, kAudioDevicePropertyStreams, kAudioObjectPropertyScopeInput).len(),
                get_ids(d, kAudioDevicePropertyStreams, kAudioObjectPropertyScopeOutput).len(),
                device_running(d) as u8,
                string_prop(d, kAudioObjectPropertyName)
            )
        })
        .collect()
}

/// Diagnóstico: dispositivos de entrada y salida de un proceso.
pub(crate) fn process_devices(pid: i32) -> (Vec<AudioObjectID>, Vec<AudioObjectID>) {
    for obj in process_objects() {
        if get::<i32>(obj, kAudioProcessPropertyPID) == Some(pid) {
            return (input_devices(obj), get_ids(obj, kAudioProcessPropertyDevices, kAudioObjectPropertyScopeOutput));
        }
    }
    (Vec::new(), Vec::new())
}
