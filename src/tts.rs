use std::time::Duration;

/// Cleans markdown and technical symbols from text so it sounds natural when spoken by TTS.
pub fn clean_text_for_speech(raw: &str) -> String {
    let mut text = raw.to_string();

    // Remove markdown symbols
    for ch in ['*', '#', '`', '_', '~', '|', '>', '<', '[', ']'] {
        text = text.replace(ch, " ");
    }

    // Replace technical abbreviations with spoken words
    text = text.replace("%cpu", "porcentaje de procesador");
    text = text.replace("%mem", "porcentaje de memoria");
    text = text.replace('%', " por ciento ");
    text = text.replace("pid:", "proceso ");
    text = text.replace("pid ", "proceso ");

    // Remove URLs if any
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|w| !w.starts_with("http://") && !w.starts_with("https://"))
        .collect();

    words.join(" ")
}

/// Splits text into chunks under max_len on word boundaries.
fn split_into_chunks(text: &str, max_len: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_len {
            current.push(' ');
            current.push_str(word);
        } else {
            chunks.push(current);
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

/// Generates MP3 audio with Spanish voice (Loquendo / TTS).
pub async fn synthesize_speech(
    client: &reqwest::Client,
    raw_text: &str,
) -> Result<Vec<u8>, String> {
    let cleaned = clean_text_for_speech(raw_text);
    if cleaned.is_empty() {
        return Err("Texto vacío para síntesis de voz".to_string());
    }

    // Limit to 400 chars max to keep audio concise
    let truncated: String = cleaned.chars().take(400).collect();
    let chunks = split_into_chunks(&truncated, 140);

    let mut combined_audio = Vec::new();

    for chunk in chunks {
        let encoded: String = form_urlencoded::byte_serialize(chunk.as_bytes()).collect();
        let url = format!(
            "https://translate.google.com/translate_tts?ie=UTF-8&tl=es&client=tw-ob&q={}",
            encoded
        );

        let resp = client
            .get(&url)
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Error conectando con servicio TTS: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Servicio TTS devolvió status {}", resp.status()));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Error descargando audio TTS: {}", e))?;

        combined_audio.extend_from_slice(&bytes);
    }

    if combined_audio.is_empty() {
        return Err("No se pudo generar audio de voz".to_string());
    }

    Ok(combined_audio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_text() {
        let raw = "## *ALERT* — CPU: 95% | RAM: 85% [host]";
        let cleaned = clean_text_for_speech(raw);
        assert!(!cleaned.contains('*'));
        assert!(!cleaned.contains('#'));
        assert!(cleaned.contains("por ciento"));
    }

    #[test]
    fn test_chunking() {
        let text = "Esta es una frase de prueba para comprobar la división en fragmentos pequeños.";
        let chunks = split_into_chunks(text, 25);
        assert!(chunks.len() > 1);
        for c in chunks {
            assert!(c.len() <= 25);
        }
    }

    #[tokio::test]
    async fn test_synthesize_speech() {
        let client = reqwest::Client::new();
        let res = synthesize_speech(&client, "Alerta de prueba del sistema").await;
        assert!(res.is_ok());
        let audio = res.unwrap();
        assert!(!audio.is_empty());
        assert!(audio.starts_with(&[0xff, 0xf3]) || audio.starts_with(&[0xff, 0xfb]) || audio.starts_with(b"ID3") || audio.len() > 500);
    }
}
