use std::path::PathBuf;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use std::collections::HashMap;
use std::fmt;

use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;

use crate::event_bus::{EventBus, Event, EventEmitter};
use crate::impl_event_emitter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    SourceCode,
    Configuration,
    Documentation,
    Test,
    Build,
    Script,
    Data,
    Other(String),
}

impl fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactType::SourceCode => write!(f, "SourceCode"),
            ArtifactType::Configuration => write!(f, "Configuration"),
            ArtifactType::Documentation => write!(f, "Documentation"),
            ArtifactType::Test => write!(f, "Test"),
            ArtifactType::Build => write!(f, "Build"),
            ArtifactType::Script => write!(f, "Script"),
            ArtifactType::Data => write!(f, "Data"),
            ArtifactType::Other(s) => write!(f, "Other({})", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub name: String,
    pub artifact_type: ArtifactType,
    pub path: PathBuf,
    pub content: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub version: String,
    pub artifacts: Vec<Artifact>,
    pub metadata: HashMap<String, String>,
}

/// Manages creation, storage, and retrieval of artifacts
pub struct ArtifactManager {
    artifact_dir: PathBuf,
    artifacts: Arc<RwLock<Vec<Artifact>>>,
    event_bus: Option<Arc<EventBus>>,
}

impl ArtifactManager {
    pub fn new(artifact_dir: PathBuf) -> Result<Self> {
        // Create artifact directory if it doesn't exist
        fs::create_dir_all(&artifact_dir)
            .context("Failed to create artifact directory")?;
        
        let manager = Self {
            artifact_dir,
            artifacts: Arc::new(RwLock::new(Vec::new())),
            event_bus: None,
        };
        
        Ok(manager)
    }
    
    /// Initialize the artifact manager by loading existing artifacts
    #[allow(dead_code)]
    pub async fn init(&self) -> Result<()> {
        // Load existing manifest if present
        if let Ok(manifest) = self.load_manifest() {
            let mut artifacts = self.artifacts.write().await;
            *artifacts = manifest.artifacts;
        }
        Ok(())
    }
    
    /// Create a new artifact
    pub async fn create_artifact(
        &self,
        name: String,
        artifact_type: ArtifactType,
        content: String,
        metadata: HashMap<String, String>,
    ) -> Result<Artifact> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        
        // Determine file extension based on type
        let extension = match &artifact_type {
            ArtifactType::SourceCode => {
                // Try to infer from name or metadata
                if name.contains('.') {
                    ""
                } else if let Some(lang) = metadata.get("language") {
                    match lang.as_str() {
                        "rust" => ".rs",
                        "python" => ".py",
                        "javascript" | "js" => ".js",
                        "typescript" | "ts" => ".ts",
                        _ => "",
                    }
                } else {
                    ""
                }
            },
            ArtifactType::Configuration => ".toml",
            ArtifactType::Documentation => ".md",
            ArtifactType::Test => "_test.rs",
            ArtifactType::Script => ".sh",
            ArtifactType::Data => ".json",
            ArtifactType::Build => "",
            ArtifactType::Other(_) => "",
        };
        
        let filename = if name.contains('.') {
            name.clone()
        } else {
            format!("{}{}", name, extension)
        };
        
        let path = self.artifact_dir.join(&filename);
        
        // Write content to file
        let mut file = fs::File::create(&path)
            .context("Failed to create artifact file")?;
        file.write_all(content.as_bytes())
            .context("Failed to write artifact content")?;
        
        let artifact = Artifact {
            id: id.clone(),
            name,
            artifact_type: artifact_type.clone(),
            path: path.clone(),
            content: Some(content),
            created_at: now,
            updated_at: now,
            metadata,
        };
        
        // Add to artifacts list
        {
            let mut artifacts = self.artifacts.write().await;
            artifacts.push(artifact.clone());
        }
        
        // Save manifest
        self.save_manifest().await?;
        
        // Emit event
        if let Some(bus) = &self.event_bus {
            let _ = bus.emit(Event::ArtifactCreated {
                name: artifact.name.clone(),
                artifact_type: format!("{:?}", artifact_type),
                path: path.to_string_lossy().to_string(),
            }).await;
        }
        
        Ok(artifact)
    }
    
    /// Update an existing artifact
    #[allow(dead_code)]
    pub async fn update_artifact(
        &self,
        id: &str,
        content: String,
    ) -> Result<()> {
        let mut artifacts = self.artifacts.write().await;
        
        if let Some(artifact) = artifacts.iter_mut().find(|a| a.id == id) {
            // Write new content
            let mut file = fs::File::create(&artifact.path)
                .context("Failed to open artifact file")?;
            file.write_all(content.as_bytes())
                .context("Failed to write artifact content")?;
            
            artifact.content = Some(content);
            artifact.updated_at = chrono::Utc::now();
            
            // Emit event
            if let Some(bus) = &self.event_bus {
                let _ = bus.emit(Event::ArtifactUpdated {
                    name: artifact.name.clone(),
                    path: artifact.path.to_string_lossy().to_string(),
                }).await;
            }
            
            drop(artifacts);
            self.save_manifest().await?;
            
            Ok(())
        } else {
            anyhow::bail!("Artifact not found: {}", id)
        }
    }
    
    /// Get an artifact by ID
    #[allow(dead_code)]
    pub async fn get_artifact(&self, id: &str) -> Option<Artifact> {
        let artifacts = self.artifacts.read().await;
        artifacts.iter().find(|a| a.id == id).cloned()
    }
    
    /// List all artifacts
    pub async fn list_artifacts(&self) -> Vec<Artifact> {
        let artifacts = self.artifacts.read().await;
        artifacts.clone()
    }
    
    /// List artifacts by type
    #[allow(dead_code)]
    pub async fn list_artifacts_by_type(&self, artifact_type: &ArtifactType) -> Vec<Artifact> {
        let artifacts = self.artifacts.read().await;
        artifacts.iter()
            .filter(|a| std::mem::discriminant(&a.artifact_type) == std::mem::discriminant(artifact_type))
            .cloned()
            .collect()
    }
    
    /// Save manifest to disk
    async fn save_manifest(&self) -> Result<()> {
        let artifacts = self.artifacts.read().await;
        let manifest = ArtifactManifest {
            version: "1.0".to_string(),
            artifacts: artifacts.clone(),
            metadata: HashMap::new(),
        };
        
        let manifest_path = self.artifact_dir.join("manifest.json");
        let json = serde_json::to_string_pretty(&manifest)
            .context("Failed to serialize manifest")?;
        
        fs::write(manifest_path, json)
            .context("Failed to write manifest")?;
        
        Ok(())
    }
    
    /// Load manifest from disk
    #[allow(dead_code)]
    fn load_manifest(&self) -> Result<ArtifactManifest> {
        let manifest_path = self.artifact_dir.join("manifest.json");
        
        if !manifest_path.exists() {
            return Ok(ArtifactManifest {
                version: "1.0".to_string(),
                artifacts: Vec::new(),
                metadata: HashMap::new(),
            });
        }
        
        let json = fs::read_to_string(manifest_path)
            .context("Failed to read manifest")?;
        
        let manifest: ArtifactManifest = serde_json::from_str(&json)
            .context("Failed to parse manifest")?;
        
        Ok(manifest)
    }
    
    /// Clean up orphaned files
    pub async fn cleanup(&self) -> Result<()> {
        let artifacts = self.artifacts.read().await;
        let artifact_paths: Vec<_> = artifacts.iter()
            .map(|a| a.path.clone())
            .collect();
        
        // Read all files in artifact directory
        let entries = fs::read_dir(&self.artifact_dir)
            .context("Failed to read artifact directory")?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            // Skip manifest and directories
            if path.is_dir() || path.file_name() == Some("manifest.json".as_ref()) {
                continue;
            }
            
            // Remove if not in artifacts list
            if !artifact_paths.contains(&path) {
                fs::remove_file(&path)
                    .context("Failed to remove orphaned file")?;
            }
        }
        
        Ok(())
    }
}

// Implement EventEmitter trait
impl_event_emitter!(ArtifactManager);
