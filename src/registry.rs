use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::{Correspondence, CorrespondenceConfig, Error, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseSummary {
    pub db_id: Uuid,
    pub max_records: u32,
    pub genesis_index: Option<u32>,
}

pub struct DatabaseRegistry {
    root_dir: PathBuf,
    open_databases: HashMap<Uuid, Correspondence>,
}

impl DatabaseRegistry {
    pub fn new(root_dir: impl Into<PathBuf>) -> Result<Self> {
        let root_dir = root_dir.into();
        fs::create_dir_all(&root_dir)?;

        Ok(Self {
            root_dir,
            open_databases: HashMap::new(),
        })
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn create_database(&mut self, max_records: u32) -> Result<DatabaseSummary> {
        let db_id = Uuid::new_v4();
        let db_dir = self.database_dir(db_id);
        fs::create_dir_all(&db_dir)?;
        self.write_config(&db_dir, max_records)?;

        let correspondence = self.open_database_from_dir(&db_dir)?;
        let summary = DatabaseSummary {
            db_id,
            max_records,
            genesis_index: None,
        };

        self.open_databases.insert(db_id, correspondence);
        Ok(summary)
    }

    pub fn delete_database(&mut self, db_id: Uuid) -> Result<bool> {
        self.open_databases.remove(&db_id);
        let db_dir = self.database_dir(db_id);
        if !db_dir.exists() {
            return Ok(false);
        }

        fs::remove_dir_all(db_dir)?;
        Ok(true)
    }

    pub fn list_databases(&mut self) -> Result<Vec<DatabaseSummary>> {
        let mut summaries = Vec::new();
        for entry in fs::read_dir(&self.root_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            let Ok(db_id) = Uuid::parse_str(&file_name) else {
                continue;
            };

            let db_dir = entry.path();
            let correspondence = if let Some(cached) = self.open_databases.get(&db_id) {
                cached
            } else {
                let opened = self.open_database_from_dir(&db_dir)?;
                self.open_databases.insert(db_id, opened);
                self.open_databases
                    .get(&db_id)
                    .expect("database inserted into cache")
            };

            summaries.push(DatabaseSummary {
                db_id,
                max_records: correspondence.max_records(),
                genesis_index: correspondence.genesis_index().ok(),
            });
        }

        summaries.sort_by_key(|summary| summary.db_id);
        Ok(summaries)
    }

    pub fn database_summary(&mut self, db_id: Uuid) -> Result<DatabaseSummary> {
        let correspondence = self.get_database_mut(db_id)?;
        Ok(DatabaseSummary {
            db_id,
            max_records: correspondence.max_records(),
            genesis_index: correspondence.genesis_index().ok(),
        })
    }

    pub fn with_database_mut<T>(
        &mut self,
        db_id: Uuid,
        operation: impl FnOnce(&mut Correspondence) -> Result<T>,
    ) -> Result<T> {
        let correspondence = self.get_database_mut(db_id)?;
        operation(correspondence)
    }

    fn get_database_mut(&mut self, db_id: Uuid) -> Result<&mut Correspondence> {
        if !self.open_databases.contains_key(&db_id) {
            let db_dir = self.database_dir(db_id);
            if !db_dir.exists() {
                return Err(Error::DatabaseNotFound(db_id.to_string()));
            }

            let correspondence = self.open_database_from_dir(&db_dir)?;
            self.open_databases.insert(db_id, correspondence);
        }

        Ok(self
            .open_databases
            .get_mut(&db_id)
            .expect("database must be present in cache"))
    }

    fn open_database_from_dir(&self, db_dir: &Path) -> Result<Correspondence> {
        let config = CorrespondenceConfig::new(
            db_dir.join("ouroboros.toml"),
            db_dir.join("membranes.redb"),
        );
        Correspondence::builder(config)
            .without_genesis_provisioning()
            .build()
    }

    fn write_config(&self, db_dir: &Path, max_records: u32) -> Result<()> {
        let config_path = db_dir.join("ouroboros.toml");
        fs::write(
            config_path,
            format!(
                "data_path = \"ouroboros.db\"\nmax_records = {max_records}\nsync_writes = false\n"
            ),
        )?;
        Ok(())
    }

    fn database_dir(&self, db_id: Uuid) -> PathBuf {
        self.root_dir.join(db_id.to_string())
    }
}