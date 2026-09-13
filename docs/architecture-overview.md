# Arquitectura — goozarap

## Propósito

Music from ratios and Euclidean patterns; voice→instrument; per-song local influence models.

## Mapa de módulos

```
goozarap/crates/
├── gooz-ratio/   # Harmonic grids, Euclidean rhythms
├── gooz-dsp/     # FFT, YIN pitch, onsets
├── gooz-audio/   # Real-time engine, transport
├── gooz-synth/   # Karplus-Strong, drums, FM
├── gooz-session/ # Project format, stems, export
├── gooz-model/   # Influence model registry
└── apps/gooz-studio/  # Tauri shell
```

## Diagrama de componentes

```mermaid
flowchart LR
VOICE[Voice input] --> DSP[gooz-dsp YIN]
DSP --> RAT[gooz-ratio grid]
RAT --> SYN[gooz-synth]
SYN --> AUD[gooz-audio engine]
AUD --> SES[gooz-session export]
```

## Capas y responsabilidades

Ver [code-walkthrough.md](./code-walkthrough.md) para el recorrido módulo a módulo.

## Documentación adicional

- `docs/ARCHITECTURE.md`
- `requirements/`
- `specs/`
