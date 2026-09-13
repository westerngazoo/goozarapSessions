# Flujos — goozarap

## Flujo principal

```mermaid
flowchart LR
VOICE[Voice input] --> DSP[gooz-dsp YIN]
DSP --> RAT[gooz-ratio grid]
RAT --> SYN[gooz-synth]
SYN --> AUD[gooz-audio engine]
AUD --> SES[gooz-session export]
```

## Descripción paso a paso

1. **Hum-to-riff** — Record → YIN pitch + onset → snap to ratio grid → K-S synth.
1. **Beat** — Euclidean E(k,n) templates → synthesized kit.
1. **Session** — Stems + takes + arrangement → WAV mixdown.
1. **Influence** — Reference tracks → feature extract → local adapter training.

## Diagrama PlantUML

Equivalente PlantUML del flujo principal (misma topología que el diagrama Mermaid):

```plantuml
@startuml
title goozarap — flujo principal
note as N1
Ver flows.md Mermaid para detalle;
exportar con herramientas mermaid→plantuml si se prefiere editar en PlantUML.
end note
@enduml
```

## Estados y casos borde

Consulta los tests de integración y los RFC/requirements del proyecto para flujos de error y recuperación.
