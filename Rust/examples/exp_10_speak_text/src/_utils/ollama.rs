// src/_utils/ollama.rs

// region: --- Modules
use reqwest;
use sentence::SentenceTokenizer;
use sentence::Token;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use crate::AppState;
// endregion: --- Modules

// region: --- Structs
#[derive(Deserialize, Serialize)] // Make sure to derive Deserialize
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaFragment {
    response: String,
    done: bool,
}
// endregion: --- Structs

pub async fn speak_ollama(
    prompt: String,
    nexus: Arc<Mutex<AppState>>,
) -> Result<(), Box<dyn Error>> {
    let (sentence_send, mut sentence_recv) = mpsc::channel::<String>(32);

    // Spawn a task to generate sentences and send them to the channel
    tokio::spawn(async move {
        match ollama_generate_api(prompt.clone(), sentence_send).await {
            Ok(_) => {}
            Err(e) => eprintln!("Failed to generate sentences: {}", e),
        }
    });

    // Clone the Arc to be used inside the closure
    let nexus_clone: Arc<Mutex<AppState>> = Arc::clone(&nexus);
    let mut index = 1;

    // Spawn a task to process sentences and speak them
    // while let Some(sentence) = sentence_recv.recv().await {
    //     // Lock the mutex to access the sentence_map
    //     let mut nexus_lock = nexus_clone.lock().await;
    //     // Obtain a mutable reference to the sentence_map inside the Mutex
    //     let sentence_map = &mut nexus_lock.sentence_map;
    //     // Insert the sentence into the sentence_map
    //     sentence_map.lock().await.insert(index, sentence.clone());
    //     // Release the mutex before calling ollama_playback_queue
    //     drop(nexus_lock);

    //     // Clone the Arc to be used inside the closure
    //     let nexus_clone_inner = Arc::clone(&nexus_clone);

    //     // Call ollama_playback_queue to process the current sentence
    //     ollama_playback_queue(nexus_clone_inner)
    //         .await
    //         .unwrap_or_else(|e| {
    //             eprintln!("Error in ollama_playback_queue: {}", e);
    //         });

    //     index += 1;
    // }

    Ok(())
}

pub async fn ollama_playback_queue(nexus: Arc<Mutex<AppState>>) -> Result<(), Box<dyn Error>> {
    println!("OLLAMA_PLAYBACK_QUEUE - ");

    let nexus_lock = nexus.lock().await;
    // let sentence_map_inner = nexus_lock.sentence_map.lock().await; // Lock the sentence map directly

    // // Sort the keys in ascending order
    // let mut keys: Vec<_> = sentence_map_inner.keys().cloned().collect();
    // keys.sort();

    // // Iterate over the sorted keys and print the corresponding values
    // for key in keys {
    //     if let Some(sentence) = sentence_map_inner.get(&key) {
    //         // speak_text(sentence, nexus_lock.playback_send.to_owned()).await;
    //         println!("{}: {}", key, sentence);
    //     }
    // }

    Ok(())
}

pub async fn ollama_generate_api(
    final_prompt: String,
    sentence_send: mpsc::Sender<String>,
) -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let request_body = GenerateRequest {
        model: "llama2-uncensored".to_string(),
        prompt: final_prompt,
        stream: true,
    };

    let mut response_stream = client
        .post("http://localhost:11434/api/generate")
        .json(&request_body)
        .send()
        .await?
        .bytes_stream();

    let mut stream_ended = false; // Flag to track if response stream has ended
    let mut sentence = String::new();

    while let Some(chunk) = response_stream.next().await {
        let chunk = chunk?;
        let chunk_text = String::from_utf8_lossy(&chunk);

        for line in chunk_text.split('\n').filter(|s| !s.is_empty()) {
            match serde_json::from_str::<OllamaFragment>(line) {
                Ok(fragment) => {
                    sentence.push_str(&fragment.response);
                    if detect_punctuation(fragment).await {
                        let final_sentence = parse_sentence(&sentence).await;
                        sentence_send.send(final_sentence).await; // await here
                        sentence.clear();
                    }
                }
                Err(e) => {
                    eprintln!("JSON parsing error: {}", e);
                }
            }
        }
    }
    // Set stream_ended to true when the response stream ends
    stream_ended = true;
    Ok(())
}

async fn parse_sentence(sentence: &String) -> String {
    let cleaned_sentence = if sentence.starts_with('\n') {
        sentence.chars().skip(1).collect()
    } else {
        sentence.clone()
    };
    cleaned_sentence
}

async fn detect_punctuation(fragment: OllamaFragment) -> bool {
    let text_fragment = fragment.response;
    let tokenizer = SentenceTokenizer::new();
    let tokens = tokenizer.tokenize(text_fragment.as_str());
    for token in tokens {
        match token {
            Token::Punctuation(punctuation) => return true,
            _ => {}
        }
    }
    return false;
}
