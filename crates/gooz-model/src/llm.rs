//! The language-model parser (R-0025 AC3 / SPEC-0025 §2).
//!
//! The model-backed half of the [`Parser`](crate::Parser) seam: a small local
//! instruct model reads a description and emits the [`MusicalIntent`] slots as
//! JSON. It runs **on-device** — weights come from disk, nothing is sent
//! anywhere — and it **falls back to [`DefaultParser`] on any failure**, so the
//! model path is never worse than the deterministic one.
//!
//! The prompt construction and JSON handling live here **unconditionally** and
//! are covered by the toolchain gates; only the model loading and decoding sit
//! behind the `llm` cargo feature, which the gates do not build. That split is
//! deliberate: the text handling is where the bugs hide, so it stays testable
//! even though inference is by-hand.
//!
//! The text helpers below are compiled even with the feature off — that is the
//! point — so with `llm` disabled only the tests call them.
#![cfg_attr(not(feature = "llm"), allow(dead_code))]

use crate::intent::MusicalIntent;

/// Builds the instruction sent to the instruct model.
///
/// It names the exact JSON keys so the reply can be deserialized straight into
/// a [`MusicalIntent`]; `serde(default)` then keeps any slot the model omits at
/// its neutral value.
pub(crate) fn instruction_for(prompt: &str) -> String {
    format!(
        "Extract the musical parameters from the description and reply with only a JSON object \
         using these keys: tempoBpm (number), meter (object with beats and unit), tension \
         (0-1), density (0-1), drive (0-1), genre (array of strings), mood (array of strings). \
         Omit any key the description does not mention.\n\nDescription: {prompt}\n\nJSON:"
    )
}

/// Extracts the first balanced JSON object from generated text.
///
/// Instruct models wrap their answer in prose or fences, so the object is found
/// by brace balance rather than by trimming. Braces inside string literals are
/// ignored, and escapes are honoured, so a genre like `"a{b"` cannot end the
/// scan early.
pub(crate) fn extract_json(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            match ch {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=start + offset]);
                }
            }
            _ => {}
        }
    }
    None // unbalanced: the model was cut off mid-object
}

/// Turns generated text into an intent, or `None` if it holds no usable object.
///
/// Always normalized, so a model that invents an out-of-range slider or an
/// impossible meter still yields a valid intent.
pub(crate) fn intent_from_reply(reply: &str) -> Option<MusicalIntent> {
    let json = extract_json(reply)?;
    serde_json::from_str::<MusicalIntent>(json)
        .ok()
        .map(MusicalIntent::normalized)
}

#[cfg(feature = "llm")]
pub use model::LmParser;

#[cfg(feature = "llm")]
mod model {
    use std::fs::File;
    use std::path::Path;

    use candle_core::quantized::gguf_file;
    use candle_core::{Device, Tensor};
    use candle_transformers::generation::LogitsProcessor;
    use candle_transformers::models::quantized_qwen2::ModelWeights;
    use tokenizers::Tokenizer;

    use super::{instruction_for, intent_from_reply};
    use crate::error::ModelError;
    use crate::intent::MusicalIntent;
    use crate::parse::{DefaultParser, Parser};

    /// Sampling is greedy so the same description always yields the same intent.
    const TEMPERATURE: Option<f64> = None;
    const SEED: u64 = 0x9002_2A2A;
    /// A slot-filling reply is short; stop well before the model rambles.
    const MAX_NEW_TOKENS: usize = 192;

    /// A [`Parser`] backed by a local quantized instruct model.
    ///
    /// Load it once and reuse it: construction reads the weights, `parse` only
    /// decodes. Never fails — see [`LmParser::parse`].
    pub struct LmParser {
        weights: std::sync::Mutex<ModelWeights>,
        tokenizer: Tokenizer,
        device: Device,
        eos: u32,
    }

    impl LmParser {
        /// Loads a GGUF instruct model and its tokenizer from disk.
        ///
        /// Both paths normally live in the song's model directory (R-0014). No
        /// network access happens here or at inference.
        pub fn load(model_gguf: &Path, tokenizer_json: &Path) -> Result<LmParser, ModelError> {
            let device = Device::Cpu;
            let mut file = File::open(model_gguf)
                .map_err(|e| ModelError::Io(format!("{model_gguf:?}: {e}")))?;
            let content = gguf_file::Content::read(&mut file)
                .map_err(|e| ModelError::Io(format!("reading gguf metadata: {e}")))?;
            let weights = ModelWeights::from_gguf(content, &mut file, &device)
                .map_err(|e| ModelError::Io(format!("loading gguf weights: {e}")))?;
            let tokenizer = Tokenizer::from_file(tokenizer_json)
                .map_err(|e| ModelError::Io(format!("{tokenizer_json:?}: {e}")))?;
            let eos = tokenizer
                .token_to_id("<|im_end|>")
                .or_else(|| tokenizer.token_to_id("<|endoftext|>"))
                .unwrap_or(0);
            Ok(LmParser {
                weights: std::sync::Mutex::new(weights),
                tokenizer,
                device,
                eos,
            })
        }

        /// Runs the model and returns its raw reply, or `None` on any failure.
        fn generate(&self, prompt: &str) -> Option<String> {
            let instruction = instruction_for(prompt);
            let encoded = self.tokenizer.encode(instruction, true).ok()?;
            let mut tokens: Vec<u32> = encoded.get_ids().to_vec();
            if tokens.is_empty() {
                return None;
            }

            let mut weights = self.weights.lock().ok()?;
            weights.clear_kv_cache();
            let mut logits_processor = LogitsProcessor::new(SEED, TEMPERATURE, None);
            let mut generated: Vec<u32> = Vec::with_capacity(MAX_NEW_TOKENS);

            // First pass consumes the whole instruction; later passes feed back
            // one token at a time against the model's kv-cache.
            let mut index_pos = 0;
            let mut input = tokens.clone();
            for _ in 0..MAX_NEW_TOKENS {
                let tensor = Tensor::new(input.as_slice(), &self.device)
                    .ok()?
                    .unsqueeze(0)
                    .ok()?;
                let logits = weights.forward(&tensor, index_pos).ok()?;
                let logits = logits.squeeze(0).ok()?;
                let next = logits_processor.sample(&logits).ok()?;
                index_pos += input.len();
                if next == self.eos {
                    break;
                }
                generated.push(next);
                tokens.push(next);
                input = vec![next];
            }
            self.tokenizer.decode(&generated, true).ok()
        }
    }

    impl Parser for LmParser {
        /// Parses `prompt` with the model, falling back to the deterministic
        /// parser whenever the model cannot produce a usable intent — a decode
        /// failure, prose with no JSON object, or malformed JSON. The caller
        /// always gets a valid intent (R-0025 AC6).
        fn parse(&self, prompt: &str) -> MusicalIntent {
            self.generate(prompt)
                .as_deref()
                .and_then(intent_from_reply)
                .unwrap_or_else(|| DefaultParser.parse(prompt))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_instruction_names_every_slot_and_carries_the_description() {
        let instruction = instruction_for("corrido a 135");
        for key in ["tempoBpm", "meter", "tension", "density", "drive", "genre"] {
            assert!(instruction.contains(key), "instruction omits {key}");
        }
        assert!(instruction.contains("corrido a 135"));
    }

    #[test]
    fn extracts_a_json_object_out_of_surrounding_prose() {
        let reply = "Sure! Here you go:\n```json\n{\"tempoBpm\": 135}\n```\nHope that helps.";
        assert_eq!(extract_json(reply), Some("{\"tempoBpm\": 135}"));
    }

    #[test]
    fn extracts_a_nested_object_whole() {
        let reply = "{\"tempoBpm\":135,\"meter\":{\"beats\":6,\"unit\":8}}";
        assert_eq!(extract_json(reply), Some(reply));
    }

    #[test]
    fn a_brace_inside_a_string_does_not_end_the_object() {
        let reply = "{\"genre\":[\"a}b\"],\"drive\":0.9}";
        assert_eq!(extract_json(reply), Some(reply));
    }

    #[test]
    fn an_escaped_quote_does_not_reopen_the_scan() {
        let reply = r#"{"mood":["say \"hi\"}"],"drive":0.5}"#;
        assert_eq!(extract_json(reply), Some(reply));
    }

    #[test]
    fn no_object_or_a_truncated_one_yields_nothing() {
        assert_eq!(extract_json("no json at all"), None);
        assert_eq!(extract_json("{\"tempoBpm\": 135"), None); // cut off mid-object
    }

    #[test]
    fn a_partial_reply_fills_only_what_it_names() {
        let intent = intent_from_reply("{\"tempoBpm\": 135}").expect("a usable object");
        assert_eq!(intent.tempo_bpm, 135.0);
        // Everything the model stayed silent about keeps its neutral value.
        assert_eq!(intent.meter, MusicalIntent::default().meter);
        assert_eq!(intent.density, MusicalIntent::default().density);
    }

    #[test]
    fn an_out_of_range_reply_is_normalized_not_trusted() {
        let intent = intent_from_reply(
            "{\"tempoBpm\": 9000, \"tension\": 7.5, \"meter\": {\"beats\": 0, \"unit\": 99}}",
        )
        .expect("a usable object");
        assert!((40.0..=250.0).contains(&intent.tempo_bpm), "tempo clamped");
        assert!((0.0..=1.0).contains(&intent.tension), "slider clamped");
        assert_eq!(
            intent.meter,
            MusicalIntent::default().meter,
            "meter repaired"
        );
    }

    #[test]
    fn malformed_or_absent_json_is_rejected_rather_than_guessed() {
        assert_eq!(intent_from_reply("I think maybe 135 bpm?"), None);
        assert_eq!(intent_from_reply("{not json}"), None);
    }
}
