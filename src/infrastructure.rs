use ouroboros::{Config as OuroborosConfig, OuroborosDb};

use crate::storage::{get_meta_u32, open_database, META_GENESIS_INDEX};
use crate::{Correspondence, CorrespondenceBuilder, CorrespondenceConfig, Error, Result};

impl Correspondence {
    pub fn builder(config: CorrespondenceConfig) -> CorrespondenceBuilder {
        CorrespondenceBuilder::new(config)
    }

    pub fn open_with_env_bootstrap(config: CorrespondenceConfig) -> Result<Self> {
        let genesis_secret_env = config.genesis_secret_env.clone();
        let mut correspondence = Self::open_infrastructure(config)?;
        correspondence.provision_genesis_from_env(&genesis_secret_env)?;
        Ok(correspondence)
    }

    pub fn open_infrastructure(config: CorrespondenceConfig) -> Result<Self> {
        let ouroboros_config = OuroborosConfig::from_path(&config.ouroboros_config_path)?;
        let ouroboros = OuroborosDb::open_with_config(&ouroboros_config)?;
        let membranes = open_database(&config.membranes_path)?;

        Ok(Self {
            ouroboros,
            membranes,
            max_records: ouroboros_config.max_records,
        })
    }

    pub fn genesis_index(&self) -> Result<u32> {
        get_meta_u32(&self.membranes, META_GENESIS_INDEX)?.ok_or(Error::BootstrapMetadataMissing)
    }
}