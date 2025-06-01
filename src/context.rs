use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use anyhow::{Result, Context as AnyhowContext};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;

use crate::event_bus::{EventBus, Event, EventEmitter};
use crate::impl_event_emitter;
use crate::llm_manager::LLMManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub token_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    pub id: String,
    pub messages: VecDeque<Message>,
    pub total_tokens: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedContext {
    pub summary: String,
    pub key_points: Vec<String>,
    pub important_details: HashMap<String, String>,
    pub original_token_count: usize,
    pub compressed_token_count: usize,
}

/// Configuration for context management
#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub max_tokens: usize,
    pub compression_threshold: f32, // 0.0 to 1.0
    pub cache_enabled: bool,
    pub cache_dir: PathBuf,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 100_000,
            compression_threshold: 0.8,
            cache_enabled: true,
            cache_dir: PathBuf::from("./cache"),
        }
    }
}

/// Manages conversation context and token limits
pub struct ContextManager {
    config: ContextConfig,
    contexts: Arc<RwLock<HashMap<String, ConversationContext>>>,
    cache: Arc<RwLock<HashMap<String, CompressedContext>>>,
    event_bus: Option<Arc<EventBus>>,
    llm_manager: Option<Arc<LLMManager>>,
}

impl ContextManager {
    pub fn new(config: ContextConfig) -> Result<Self> {
        // Create cache directory if enabled
        if config.cache_enabled {
            std::fs::create_dir_all(&config.cache_dir)
                .context("Failed to create cache directory")?;
        }
        
        Ok(Self {
            config,
            contexts: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            event_bus: None,
            llm_manager: None,
        })
    }
    
    /// Set the LLM manager for compression
    pub fn set_llm_manager(&mut self, llm_manager: Arc<LLMManager>) {
        self.llm_manager = Some(llm_manager);
    }
    
    /// Update compression threshold
    #[allow(dead_code)]
    pub fn set_compression_threshold(&mut self, threshold: f32) {
        self.config.compression_threshold = threshold.clamp(0.0, 1.0);
    }
    
    /// Get current compression configuration
    #[allow(dead_code)]
    pub fn get_compression_config(&self) -> (f32, usize) {
        (self.config.compression_threshold, self.config.max_tokens)
    }
    
    /// Create a new conversation context
    pub async fn create_context(&self, metadata: HashMap<String, String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        
        let context = ConversationContext {
            id: id.clone(),
            messages: VecDeque::new(),
            total_tokens: 0,
            created_at: now,
            updated_at: now,
            metadata,
        };
        
        let mut contexts = self.contexts.write().await;
        contexts.insert(id.clone(), context);
        
        // Emit event
        if let Some(bus) = &self.event_bus {
            let _ = bus.emit(Event::ContextCreated {
                id: id.clone(),
            }).await;
        }
        
        id
    }
    
    /// Add a message to context
    pub async fn add_message(
        &self,
        context_id: &str,
        role: String,
        content: String,
    ) -> Result<()> {
        let mut contexts = self.contexts.write().await;
        
        if let Some(context) = contexts.get_mut(context_id) {
            // Estimate token count (improved estimation)
            let token_count = self.estimate_tokens(&content);
            
            let message = Message {
                role,
                content,
                timestamp: chrono::Utc::now(),
                token_count: Some(token_count),
            };
            
            context.messages.push_back(message);
            context.total_tokens += token_count;
            context.updated_at = chrono::Utc::now();
            
            // Check if we need compression
            let max_tokens = if let Some(llm_manager) = &self.llm_manager {
                llm_manager.get_context_size()
            } else {
                self.config.max_tokens // Fallback to config if no LLM manager
            };
            
            let usage_ratio = context.total_tokens as f32 / max_tokens as f32;
            if usage_ratio > self.config.compression_threshold {
                drop(contexts);
                self.compress_context(context_id).await?;
            } else {
                // Emit usage event
                if let Some(bus) = &self.event_bus {
                    let _ = bus.emit(Event::ContextUsageChanged {
                        id: context_id.to_string(),
                        usage_percentage: usage_ratio * 100.0,
                        total_tokens: context.total_tokens,
                    }).await;
                }
            }
            
            Ok(())
        } else {
            anyhow::bail!("Context not found: {}", context_id)
        }
    }
    
    /// Get messages from context with optional token limit
    #[allow(dead_code)]
    pub async fn get_messages(
        &self,
        context_id: &str,
        max_tokens: Option<usize>,
    ) -> Result<Vec<Message>> {
        let contexts = self.contexts.read().await;
        
        if let Some(context) = contexts.get(context_id) {
            if let Some(max) = max_tokens {
                // Return most recent messages that fit within token limit
                let mut messages = Vec::new();
                let mut token_count = 0;
                
                for message in context.messages.iter().rev() {
                    let msg_tokens = message.token_count.unwrap_or(0);
                    if token_count + msg_tokens > max {
                        break;
                    }
                    messages.push(message.clone());
                    token_count += msg_tokens;
                }
                
                messages.reverse();
                Ok(messages)
            } else {
                Ok(context.messages.iter().cloned().collect())
            }
        } else {
            anyhow::bail!("Context not found: {}", context_id)
        }
    }
    
    /// Compress context to save tokens
    async fn compress_context(&self, context_id: &str) -> Result<()> {
        let mut contexts = self.contexts.write().await;
        
        if let Some(context) = contexts.get_mut(context_id) {
            // Keep system messages separate
            let system_messages: Vec<_> = context.messages.iter()
                .filter(|m| m.role == "system")
                .cloned()
                .collect();
            
            // Get non-system messages
            let conversation_messages: Vec<_> = context.messages.iter()
                .filter(|m| m.role != "system")
                .cloned()
                .collect();
            
            if conversation_messages.is_empty() {
                return Ok(());
            }
            
            // Calculate token budget (30% of max for recent messages)
            let token_budget = (self.config.max_tokens as f32 * 0.3) as usize;
            
            // Try different window sizes to find what fits in budget
            let window_sizes = [30, 25, 20, 15, 10, 5];
            let mut recent_messages = Vec::new();

            let mut messages_to_summarize = Vec::new();
            
            for window_size in window_sizes.iter() {
                recent_messages.clear();

                
                // Take the last N messages
                let start_idx = conversation_messages.len().saturating_sub(*window_size);
                
                for msg in conversation_messages[start_idx..].iter() {
                    let msg_tokens = msg.token_count.unwrap_or(0);
                    if msg_tokens <= token_budget {
                        recent_messages.push(msg.clone());

                    } else {
                        break;
                    }
                }
                
                // If we found a good window, use it
                if !recent_messages.is_empty() {
                    messages_to_summarize = conversation_messages[..start_idx].to_vec();
                    break;
                }
            }
            
            // If no recent messages fit, just keep the last 5
            if recent_messages.is_empty() {
                let keep_count = conversation_messages.len().min(5);
                recent_messages = conversation_messages[conversation_messages.len() - keep_count..].to_vec();
                messages_to_summarize = conversation_messages[..conversation_messages.len() - keep_count].to_vec();
            }
            
            let original_tokens = context.total_tokens;
            
            // Create summary if we have messages to summarize and LLM is available
            let mut summary_content = String::new();
            
            if !messages_to_summarize.is_empty() {
                if let Some(llm) = &self.llm_manager {
                    // Prepare messages for summarization
                    let mut summary_prompt = String::from(
                        "Please create a concise summary of the following conversation. \
                        Focus on key information, decisions made, and important context. \
                        Format the summary as bullet points.\n\n"
                    );
                    
                    for msg in messages_to_summarize.iter() {
                        summary_prompt.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
                    }
                    
                    // Get summary from LLM
                    match llm.send_prompt(&summary_prompt).await {
                        Ok(summary) => {
                            summary_content = summary;
                        }
                        Err(e) => {
                            // Fallback to basic summary
                            summary_content = format!(
                                "Previous {} messages were compressed. Key topics discussed.",
                                messages_to_summarize.len()
                            );
                            eprintln!("Failed to generate LLM summary: {}", e);
                        }
                    }
                } else {
                    // No LLM available, create basic summary
                    summary_content = format!(
                        "Previous {} messages were compressed to save tokens. \
                        Unable to generate detailed summary without LLM.",
                        messages_to_summarize.len()
                    );
                }
                
                // Create compressed context record
                let compressed = CompressedContext {
                    summary: summary_content.clone(),
                    key_points: vec![
                        format!("Compressed {} messages", messages_to_summarize.len()),
                        format!("Original token count: {}", original_tokens),
                    ],
                    important_details: HashMap::new(),
                    original_token_count: messages_to_summarize.iter()
                        .map(|m| m.token_count.unwrap_or(0))
                        .sum(),
                    compressed_token_count: self.estimate_tokens(&summary_content),
                };
                
                // Store in cache
                if self.config.cache_enabled {
                    let mut cache = self.cache.write().await;
                    cache.insert(
                        format!("{}_{}", context_id, chrono::Utc::now().timestamp()),
                        compressed
                    );
                }
            }
            
            // Rebuild context with compressed version
            context.messages.clear();
            
            // Re-add system messages
            for msg in system_messages {
                context.messages.push_back(msg);
            }
            
            // Add summary if we created one
            if !summary_content.is_empty() {
                context.messages.push_back(Message {
                    role: "system".to_string(),
                    content: format!(
                        "=== Context Summary ===\n{}\n=== End Summary ===",
                        summary_content
                    ),
                    timestamp: chrono::Utc::now(),
                    token_count: Some(self.estimate_tokens(&summary_content) + 10),
                });
            }
            
            // Re-add recent messages
            for msg in recent_messages {
                context.messages.push_back(msg);
            }
            
            // Recalculate tokens
            context.total_tokens = context.messages.iter()
                .map(|m| m.token_count.unwrap_or(0))
                .sum();
            
            // Emit event
            if let Some(bus) = &self.event_bus {
                let _ = bus.emit(Event::ContextCompressed {
                    id: context_id.to_string(),
                    original_tokens,
                    compressed_tokens: context.total_tokens,
                }).await;
            }
            
            Ok(())
        } else {
            anyhow::bail!("Context not found: {}", context_id)
        }
    }
    
    /// Estimate token count for a string
    fn estimate_tokens(&self, text: &str) -> usize {
        // More accurate estimation based on GPT tokenization patterns
        // Average is ~1 token per 4 characters for English text
        // But we account for whitespace and punctuation
        let char_count = text.chars().count();
        let word_count = text.split_whitespace().count();
        
        // Heuristic: average between character-based and word-based estimates
        let char_estimate = char_count / 4;
        let word_estimate = (word_count as f32 * 1.3) as usize; // 1.3 tokens per word on average
        
        (char_estimate + word_estimate) / 2
    }
    
    /// Get context usage statistics
    pub async fn get_usage(&self, context_id: &str) -> Result<(usize, f32)> {
        let contexts = self.contexts.read().await;
        
        if let Some(context) = contexts.get(context_id) {
            let max_tokens = if let Some(llm_manager) = &self.llm_manager {
                llm_manager.get_context_size()
            } else {
                self.config.max_tokens // Fallback to config if no LLM manager
            };
            
            let usage_ratio = context.total_tokens as f32 / max_tokens as f32;
            Ok((context.total_tokens, usage_ratio * 100.0))
        } else {
            anyhow::bail!("Context not found: {}", context_id)
        }
    }
    
    /// Clear all messages from a context
    #[allow(dead_code)]
    pub async fn clear_context(&self, context_id: &str) -> Result<()> {
        let mut contexts = self.contexts.write().await;
        
        if let Some(context) = contexts.get_mut(context_id) {
            context.messages.clear();
            context.total_tokens = 0;
            context.updated_at = chrono::Utc::now();
            
            // Emit event
            if let Some(bus) = &self.event_bus {
                let _ = bus.emit(Event::ContextCleared {
                    id: context_id.to_string(),
                }).await;
            }
            
            Ok(())
        } else {
            anyhow::bail!("Context not found: {}", context_id)
        }
    }
    
    /// Save context to cache
    #[allow(dead_code)]
    pub async fn save_to_cache(&self, context_id: &str) -> Result<()> {
        if !self.config.cache_enabled {
            return Ok(());
        }
        
        let contexts = self.contexts.read().await;
        
        if let Some(context) = contexts.get(context_id) {
            let cache_path = self.config.cache_dir.join(format!("{}.json", context_id));
            let json = serde_json::to_string_pretty(context)
                .context("Failed to serialize context")?;
            
            tokio::fs::write(cache_path, json).await
                .context("Failed to write context to cache")?;
            
            Ok(())
        } else {
            anyhow::bail!("Context not found: {}", context_id)
        }
    }
    
    /// Load context from cache
    #[allow(dead_code)]
    pub async fn load_from_cache(&self, context_id: &str) -> Result<()> {
        if !self.config.cache_enabled {
            anyhow::bail!("Cache is disabled");
        }
        
        let cache_path = self.config.cache_dir.join(format!("{}.json", context_id));
        
        if !cache_path.exists() {
            anyhow::bail!("Context not found in cache");
        }
        
        let json = tokio::fs::read_to_string(cache_path).await
            .context("Failed to read context from cache")?;
        
        let context: ConversationContext = serde_json::from_str(&json)
            .context("Failed to deserialize context")?;
        
        let mut contexts = self.contexts.write().await;
        contexts.insert(context_id.to_string(), context);
        
        Ok(())
    }
}

// Implement EventEmitter trait
impl_event_emitter!(ContextManager);
