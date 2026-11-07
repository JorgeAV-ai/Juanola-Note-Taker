# third_party

## mlxrs-sys

Copia de `mlxrs-sys` 0.2.0 (crates.io) con un parche: si existe la variable
`MLXRS_PREBUILT_METALLIB`, el build copia ese `mlx.metallib` en lugar de compilar
los kernels Metal con `xcrun metal`. Ese compilador solo viene con Xcode
completo; con las Command Line Tools el build original falla.

- `build.rs`: pasa `MLX_PREBUILT_METALLIB` a CMake.
- `vendor/mlx/mlx/backend/metal/kernels/CMakeLists.txt`: bloque inicial que
  copia el metallib y desactiva los kernels NAX (exigen Metal 4 y GPU M5+).

El metallib sale de la wheel oficial `mlx-metal` de la **misma versión de MLX**
que trae el vendor (0.32.2). `scripts/fetch-mlx-metallib.sh` lo descarga y
verifica su SHA-256 en `third_party/mlx-metallib/` (ignorado por git, 182 MB).
`.cargo/config.toml` apunta la variable ahí.

Quitar este parche cuando haya Xcode en las máquinas de build o cuando
`mlxrs-sys` acepte un metallib externo.

## llama-cpp-sys-2 y llama-cpp-2

Copias de 0.1.161 (crates.io). El crate trae llama.cpp del 5 de octubre de 2026
(0.6.0), sin la arquitectura `lfm3`; LFM3-1.7B entró en llama.cpp el 27 de
octubre (#31252).

- `llama-cpp-sys-2/llama.cpp/`: código de llama.cpp **b11960** (ignorado por git,
  181 MB). Lo baja `scripts/fetch-llama-cpp.sh` y verifica su SHA-256. Metal se
  compila con `GGML_METAL_EMBED_LIBRARY` (los shaders se compilan en tiempo de
  ejecución y macOS los cachea: ~15 s la primera vez en cada máquina, 0,3 s
  después), así que no hace falta Xcode.
- `llama-cpp-2/src/timing.rs`: inicializa `n_reallocs_same` y `n_reallocs_diff`,
  campos nuevos de `llama_perf_context_data` en b11960. Es el único cambio de
  API que rompía la compilación.

Quitar ambos parches cuando `llama-cpp-2` publique una versión con llama.cpp
b11960 o posterior.
