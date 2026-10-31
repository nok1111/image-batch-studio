# Image Batch Studio — Propuesta de arquitectura y plan MVP

Estado: **propuesta para aprobación** (no se ha escrito código de implementación).

---

## 1. Análisis de requisitos (resumen)

| Área | Requisito clave | Implicación técnica |
|---|---|---|
| Procesamiento | Miles de archivos, sin congelar la UI, cancelable | Motor en Rust con pool de hilos propio, streaming de entradas, eventos agregados hacia la UI |
| Fiabilidad | "Nunca reportar éxito sin verificar el archivo" | Escritura a temporal → verificación (re-lectura de cabecera + dimensiones + tamaño > 0) → rename atómico |
| Seguridad de datos | No sobrescribir originales | Política de salida centralizada: el motor rechaza cualquier ruta de salida igual a una de entrada |
| Privacidad | 100 % local, sin telemetría | Sin red salvo licencia (opt-in, endpoint único y documentado) |
| Comercial | 3 ediciones, Lemon Squeezy, offline | Módulo de licencias aislado, *feature flags* por edición, procesamiento nunca depende de red |
| UX | Premium, oscura, i18n EN/ES/PT-BR desde el día 1 | Design system propio sobre shadcn/ui + tokens de color del spec; todas las cadenas en ficheros de locale |
| Plataforma | Windows 10/11 primero; macOS/Linux después | Tauri 2 + Rust puro (sin dependencias nativas exclusivas de Windows) |

Conclusión: los 8 módulos son, en realidad, **una sola tubería de procesamiento** con distintas operaciones y *presets*. Diseñarlo así evita 8 motores y habilita después las "operaciones por lotes avanzadas" (encadenar resize + marca de agua + convertir) de la edición Pro.

---

## 2. Arquitectura propuesta

```
┌──────────────────────────── Frontend (WebView2) ────────────────────────────┐
│ React + TS + Tailwind + shadcn/ui                                           │
│  features/<módulo>  →  stores (Zustand)  →  lib/ipc (tipos generados)        │
└───────────────▲───────────────────────────────────────────┬─────────────────┘
                │ eventos agregados (progreso, estado/archivo) │ comandos
┌───────────────┴──────────── src-tauri (capa fina) ──────────▼─────────────────┐
│ Comandos Tauri, validación de rutas, gestión de jobs, canal de eventos        │
└───────┬───────────────────────┬──────────────────────────┬────────────────────┘
        │                       │                          │
┌───────▼────────┐   ┌──────────▼─────────┐     ┌──────────▼─────────┐
│  ibs-core      │   │  ibs-store          │     │  ibs-license        │
│  motor + ops   │   │  SQLite: ajustes,   │     │  Lemon Squeezy +    │
│  códecs, meta, │   │  presets, historial │     │  mock de desarrollo │
│  nombres       │   │                     │     │                     │
└────────────────┘   └─────────────────────┘     └─────────────────────┘
```

### Decisiones principales

1. **Todo el procesamiento en Rust (sin Sharp).** Sharp exigiría empaquetar Node.js como *sidecar* (+~40-80 MB, otro proceso, otra superficie de fallos). Rust ya está en Tauri y el ecosistema cubre todos los formatos.
2. **`ibs-core` es una librería Rust pura, independiente de Tauri.** Se testea con `cargo test` y con un pequeño CLI de pruebas/benchmarks, sin UI. Esto es lo que permite "probar cada módulo antes de avanzar".
3. **Modelo de job:**
   ```
   Job { inputs: Vec<Path>, pipeline: Vec<Operation>, output: OutputSpec }
   Operation = Resize | Crop | Rotate | Flip | Watermark | StripMetadata | ...
   OutputSpec { dir, format + opciones de encoder, plantilla de nombre,
                política de conflicto (skip/renombrar/sobrescribir-salida), política de metadatos }
   ```
   Cada módulo de la UI construye un `Job` con un pipeline predefinido. Los presets son `Job` serializados (JSON) → importar/exportar es trivial.
4. **Ejecución:** pool `rayon` dedicado con concurrencia configurable; cada archivo es una tarea independiente (decodificar → ops → codificar → verificar). Memoria acotada a ~N imágenes en vuelo (N = concurrencia), nunca la carpeta entera.
5. **Cancelación** cooperativa (token atómico comprobado entre etapas); objetivo < 1 s. Los temporales parciales se borran.
6. **Aislamiento de errores:** límites de decodificación (`image::Limits`, anti "decompression bomb") + `catch_unwind` por archivo → un archivo corrupto nunca tumba el lote.
7. **Eventos hacia la UI agregados/throttled (~10 Hz)** con `tauri::ipc::Channel`, no un evento por archivo → la UI no se satura con 10 000 archivos.
8. **Miniaturas** generadas en Rust a una caché local y servidas por protocolo de assets; grid virtualizado en la UI.
9. **Tipos compartidos** Rust→TS generados con `ts-rs` (evita desincronizar la API IPC).
10. **SQLite vía `rusqlite` (bundled)** en el lado Rust, con migraciones versionadas. El frontend nunca habla SQL directamente.
11. **Decisiones arquitectónicas documentadas como ADRs** en `docs/adr/`.

---

## 3. Librerías seleccionadas

### Frontend
| Uso | Librería |
|---|---|
| Build | Vite + React 18 + TypeScript (strict) |
| Estilos / UI | Tailwind CSS + shadcn/ui (Radix) + lucide-react (iconos) |
| Estado | Zustand |
| Listas grandes | @tanstack/react-virtual |
| i18n | i18next + react-i18next (EN, ES, PT-BR) |
| Formularios | react-hook-form + zod |
| Tests | Vitest + Testing Library; Playwright/WebDriver para E2E en fase 6 |

### Rust (todas MIT/Apache/BSD — compatibles con venta comercial)
| Uso | Crate | Nota |
|---|---|---|
| Shell | `tauri` 2 + plugins `dialog`, `fs`, `opener`, `store`(opcional) | |
| Decodificar/codificar base | `image` 0.25 | PNG, JPEG, BMP, TIFF, GIF, ICO, WebP (decode) |
| Resize rápido (SIMD) | `fast_image_resize` | Lanczos3 / CatmullRom / Bilinear |
| JPEG con calidad/tamaño óptimos | `mozjpeg` | Mejor ratio que el encoder de `image` |
| PNG sin pérdida | `oxipng` | Optimización lossless real |
| WebP con pérdida | `webp` (libwebp) | `image` sólo codifica WebP lossless |
| AVIF codificar | `ravif` (rav1e) | Rust puro, lento pero fiable |
| AVIF decodificar | `image` + `dav1d` | Requiere compilar dav1d (C) — ver riesgos |
| Metadatos (leer) | `kamadak-exif` | EXIF/GPS para inspector |
| Metadatos (quitar/reinsertar sin recodificar) | `img-parts` | JPEG/PNG/WebP: EXIF e ICC por chunks |
| Perfiles ICC / CMYK | `lcms2` | Preservar/convertir color |
| Marca de agua texto | `ab_glyph` + `imageproc` | Fuentes OFL incluidas (p. ej. Inter) |
| Recorrer carpetas | `walkdir` | |
| Concurrencia | `rayon` | Pool propio, tamaño configurable |
| BD | `rusqlite` (bundled) + `rusqlite_migration` | |
| HTTP licencia | `reqwest` (rustls) | Sólo módulo de licencias |
| Errores / logs | `thiserror`, `tracing` (logs locales, sin envío) | |

**Descartadas a propósito:** `imagequant`/pngquant (GPLv3 o licencia comercial de pago → incompatible sin comprarla), Sharp/Node sidecar, ImageMagick (binario externo grande, historial de CVEs), cualquier API de IA/cloud.

---

## 4. Estructura de carpetas

```
image-batch-studio/
├─ src/                         # Frontend
│  ├─ app/                      # Router, layout (sidebar + workspace + status bar)
│  ├─ components/ui/            # shadcn/ui
│  ├─ components/shared/        # DropZone, FileGrid, QueuePanel, OutputPicker, PresetPicker...
│  ├─ features/
│  │  ├─ dashboard/  converter/  compressor/  resizer/  renamer/
│  │  └─ watermark/  metadata/  crop-rotate/  analyzer/  settings/   (fase 4)
│  ├─ lib/ipc/                  # wrappers de comandos + tipos generados (ts-rs)
│  ├─ stores/                   # Zustand
│  ├─ i18n/locales/{en,es,pt-BR}.json
│  └─ styles/tokens.css         # paleta del spec
├─ src-tauri/
│  ├─ src/commands/             # una carpeta por dominio (jobs, files, presets, settings, license)
│  ├─ src/events.rs  state.rs  paths.rs (validación de rutas)
│  ├─ capabilities/             # permisos Tauri mínimos
│  └─ tauri.conf.json
├─ crates/
│  ├─ ibs-core/                 # motor, ops/, codecs/, metadata/, naming/, verify.rs
│  ├─ ibs-store/                # SQLite + migraciones
│  ├─ ibs-license/              # trait LicenseProvider: LemonSqueezy | Mock
│  └─ ibs-cli/                  # CLI de pruebas y benchmarks (no se distribuye)
├─ tests/fixtures/              # imágenes de prueba: transparentes, CMYK, EXIF+GPS, corruptas, enormes
├─ docs/adr/  docs/CHECKLIST.md
└─ .github/workflows/           # CI: lint, tests, build Windows (NSIS/MSI)
```

---

## 5. Limitaciones técnicas y riesgos identificados

1. **AVIF:** codificar en Rust puro es lento (segundos por imagen grande; mitigable con `speed`). Decodificar requiere dav1d (C, meson/nasm) → complica el build. Alternativa: MVP con AVIF sólo como **salida**, entrada AVIF en fase 4.
2. **GIF animado:** el MVP procesará el primer fotograma y lo avisará; animación completa (redimensionar todos los frames) en fase posterior. WebP/AVIF animados: fuera del MVP.
3. **ICO:** máximo 256×256; se generará ICO multi-tamaño (16-256).
4. **HEIC/HEIF:** no incluido (patentes HEVC / libheif LGPL). Se puede evaluar más adelante.
5. **"Tamaño objetivo":** sólo factible en formatos con pérdida (JPEG/WebP/AVIF) mediante búsqueda binaria de calidad → varias codificaciones por imagen (más lento). PNG: no se garantiza.
6. **Metadatos:** quitar EXIF sin recodificar es posible en JPEG/PNG/WebP; TIFF guarda metadatos en la propia estructura → requiere reescritura. XMP/IPTC se tratarán como "otros metadatos".
7. **Licencias offline:** las respuestas de la License API de Lemon Squeezy no vienen firmadas; cualquier comprobación cliente es eludible. Propuesta: caché de activación + periodo de gracia configurable (p. ej. 30 días) + revalidación cuando haya red. Más seguridad exigiría un pequeño servicio propio que firme tokens (contradice "sin backend") → decisión para fase 5.
8. **SmartScreen en Windows:** sin certificado de firma de código (OV/EV, de pago) el instalador mostrará advertencia. Necesario para venta comercial.
9. **WebView2:** viene en Win11 y en la mayoría de Win10; el instalador incluirá el bootstrapper.
10. **Entorno de build:** mi VM es Linux. Puedo desarrollar y testear `ibs-core` y el frontend aquí; los builds/instalador de Windows se harían en **GitHub Actions (windows-latest)** y las pruebas reales en tu máquina Windows.
11. **Precisión de progreso/ETA:** la ETA depende de tamaños muy dispares; se calculará sobre bytes procesados, no sobre nº de archivos.

---

## 6. Alcance del MVP (Fases 1-3)

**Incluido**
- Shell de la app: sidebar, workspace, barra de estado, dashboard (acciones rápidas, recientes, estadísticas), tema oscuro con la paleta del spec.
- Ajustes persistentes: idioma (EN/ES/PT-BR), carpeta de salida, formato/calidad por defecto, concurrencia, política de conflictos.
- Motor: selección de archivos/carpetas, drag & drop, cola, procesamiento en segundo plano, progreso, cancelación, estado y error por archivo, verificación de salida, "abrir carpeta de salida".
- **Converter:** PNG, JPG, WebP (lossy/lossless), AVIF (salida), BMP, TIFF, GIF (estático), ICO; transparencia; calidad; conservar/quitar metadatos; saltar existentes.
- **Compressor:** presets (Máxima calidad, Equilibrado, Alta compresión, Web, Personalizado), slider de calidad, tamaño objetivo (formatos con pérdida), tamaño antes/después/% ahorrado, vista comparativa.
- **Resizer:** px, %, ancho, alto, encajar, rellenar con recorte, permitir/impedir ampliar; presets de plataforma editables (crear/editar/borrar).
- **Bulk Renamer:** prefijo/sufijo, numeración con inicio, reemplazar/quitar texto, fecha, plantillas `{name}_{n:03}_{date}`; vista previa completa; detección de colisiones.
- Tests automáticos de lo anterior.

**Fuera del MVP:** Watermark, Metadata Cleaner, Crop & Rotate, Analyzer, import/export de presets (fase 4); licencias, instalador firmado, onboarding, landing (fase 5).

---

## 7. Hoja de ruta con hitos medibles

| Fase | Hito | Criterio de aceptación (medible) |
|---|---|---|
| **1 Fundación** | M1.1 Proyecto | `cargo test`, `pnpm lint`, `pnpm typecheck` en verde en CI; build Windows generado por CI |
| | M1.2 Design system + navegación | 10 rutas del sidebar navegables; tokens de color del spec; 0 cadenas sin traducir (test que compara claves de los 3 locales) |
| | M1.3 Ajustes | Ajustes sobreviven a reinicio (test de integración sobre SQLite); cambio de idioma en caliente |
| | M1.4 Dashboard | Acciones rápidas abren el módulo con archivos ya cargados (≤ 2 clics) |
| **2 Motor** | M2.1 Entrada | Carpeta de 10 000 archivos enumerada sin bloquear la UI; grid virtualizado fluido |
| | M2.2 Cola + workers | Lote de 1 000 imágenes: UI responde (< 100 ms a interacciones) durante todo el proceso |
| | M2.3 Cancelación | Cancelar detiene en < 1 s y no deja temporales |
| | M2.4 Errores | Fixtures corruptos/truncados/no-imagen/zip-bomb → marcados como fallidos con mensaje claro; resto del lote completa |
| | M2.5 Garantías | Test: ninguna operación puede escribir sobre una ruta de entrada; ningún "éxito" sin verificación |
| **3 MVP** | M3.1 Converter | Matriz formato×formato en tests: dimensiones correctas, alfa preservado donde el formato lo soporta, metadatos según política |
| | M3.2 Compressor | % de ahorro mostrado = tamaño real en disco (test); tamaño objetivo alcanzado ±5 % en JPEG/WebP |
| | M3.3 Resizer | Tests de cada modo (fit/fill/%/ancho/alto/no-upscale) con dimensiones exactas esperadas |
| | M3.4 Renamer | Vista previa == resultado final (test); colisiones detectadas antes de ejecutar |
| | M3.5 Benchmark | Línea base medida y documentada (p. ej. 1 000 JPEG 12 MP → WebP q80) en tu máquina Windows |
| **4 Pro** | Watermark, Metadata, Crop&Rotate, Analyzer, presets import/export, pipelines encadenados | Mismo estándar de tests por módulo |
| **5 Comercial** | Licencias (LS + mock dev), feature flags por edición, instalador NSIS/MSI, icono, onboarding, docs | Activar/desactivar/offline probado contra modo test de Lemon Squeezy |
| **6 Release** | Win10/Win11, lotes grandes, poca memoria, corruptos, instalación/desinstalación, offline | Checklist de release sin incidencias críticas |

Al final de cada fase entregaré: funcionalidades implementadas, archivos modificados, tests ejecutados, problemas pendientes y siguiente fase (como pide el spec).

---

## 8. Decisiones que necesito de ti antes de empezar la Fase 1

1. **Repositorio:** ¿creo un repo nuevo en GitHub (`nok1111/image-batch-studio`, privado) o tienes uno?
2. **Sin licencia, ¿qué ocurre?** Opciones: (a) prueba de N días completa, (b) modo gratuito limitado (p. ej. 20 imágenes por lote), (c) sólo funciona con licencia. Afecta al diseño de *feature flags*.
3. **Renamer:** ¿renombrar *in situ* (con registro para deshacer) o, por defecto, copiar renombrado a la carpeta de salida? El spec prohíbe tocar originales en el resto de módulos; propongo copiar por defecto e *in situ* como opción explícita con deshacer.
4. **AVIF de entrada en MVP:** ¿lo incluyo ya (build más complejo por dav1d) o lo dejo para fase 4 (AVIF sólo como salida en el MVP)? Propongo lo segundo.
