//! The song arrangement: sections, an optional loop region, and stem placements.

use serde::{Deserialize, Serialize};

use crate::error::SessionError;
use crate::model::Stem;

/// A named span of bars in the song (intro, verse, hook, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    /// A human label for the section.
    pub name: String,
    /// The bar this section starts on (0 = the first bar).
    pub start_bar: u32,
    /// How many bars the section spans (≥ 1).
    pub length_bars: u32,
}

impl Section {
    /// The bar one past the section's end (`start_bar + length_bars`).
    pub fn end_bar(&self) -> u32 {
        self.start_bar.saturating_add(self.length_bars)
    }
}

/// A bar span to repeat during playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopRegion {
    /// The bar the loop starts on.
    pub start_bar: u32,
    /// How many bars the loop spans (≥ 1).
    pub length_bars: u32,
}

impl LoopRegion {
    /// The bar one past the loop's end (`start_bar + length_bars`).
    pub fn end_bar(&self) -> u32 {
        self.start_bar.saturating_add(self.length_bars)
    }
}

/// Where a stem sits on the timeline, and its mix state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StemPlacement {
    /// Index into [`crate::Song::stems`].
    pub stem: usize,
    /// The bar the stem starts on.
    pub start_bar: u32,
    /// Whether the stem is silenced in the mix.
    pub muted: bool,
    /// Linear gain in `[0, 1]`.
    pub level: f32,
}

/// A song's timeline: its sections, an optional loop, and stem placements.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Arrangement {
    /// Named bar spans, in author order.
    pub sections: Vec<Section>,
    /// The loop region, if any.
    pub loop_region: Option<LoopRegion>,
    /// Stem placements on the timeline.
    pub placements: Vec<StemPlacement>,
}

impl Arrangement {
    /// The furthest bar any section, placement, or the loop region reaches;
    /// `0` for an empty arrangement. Placement ends use `stems` to resolve each
    /// stem's bar length.
    pub fn total_bars(&self, stems: &[Stem]) -> u32 {
        let section_end = self
            .sections
            .iter()
            .map(Section::end_bar)
            .max()
            .unwrap_or(0);
        let loop_end = self.loop_region.map_or(0, |l| l.end_bar());
        let placement_end = self
            .placements
            .iter()
            .map(|p| {
                let bars = stems.get(p.stem).map_or(0, |s| s.bars);
                p.start_bar.saturating_add(bars)
            })
            .max()
            .unwrap_or(0);
        section_end.max(loop_end).max(placement_end)
    }

    /// Checks the arrangement against a song with `stem_count` stems.
    ///
    /// Rejects a zero-length section or loop, a placement referencing a
    /// non-existent stem, or a level outside `[0, 1]`.
    pub fn validate(&self, stem_count: usize) -> Result<(), SessionError> {
        for section in &self.sections {
            if section.length_bars == 0 {
                return Err(SessionError::InvalidArrangement(format!(
                    "section '{}' has zero length",
                    section.name
                )));
            }
        }
        if let Some(loop_region) = self.loop_region
            && loop_region.length_bars == 0
        {
            return Err(SessionError::InvalidArrangement(
                "loop region has zero length".into(),
            ));
        }
        for (i, placement) in self.placements.iter().enumerate() {
            if placement.stem >= stem_count {
                return Err(SessionError::InvalidArrangement(format!(
                    "placement {i} references stem {} but the song has {stem_count} stem(s)",
                    placement.stem
                )));
            }
            if !placement.level.is_finite() || !(0.0..=1.0).contains(&placement.level) {
                return Err(SessionError::InvalidArrangement(format!(
                    "placement {i} level {} is outside [0, 1]",
                    placement.level
                )));
            }
        }
        Ok(())
    }
}
