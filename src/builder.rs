use crate::{Correspondence, CorrespondenceConfig, Error, Result};

pub struct CorrespondenceBuilder {
    config: CorrespondenceConfig,
    provisioning: GenesisProvisioning,
    invalid_state: Option<&'static str>,
}

enum GenesisProvisioning {
    None,
    FromConfiguredEnv,
    FromExplicitEnv(String),
    Explicit { secret: Vec<u8>, genoma: u32 },
}

impl CorrespondenceBuilder {
    pub fn new(config: CorrespondenceConfig) -> Self {
        Self {
            config,
            provisioning: GenesisProvisioning::None,
            invalid_state: None,
        }
    }

    pub fn without_genesis_provisioning(mut self) -> Self {
        self.select_provisioning(GenesisProvisioning::None);
        self
    }

    pub fn with_configured_env_bootstrap(mut self) -> Self {
        self.select_provisioning(GenesisProvisioning::FromConfiguredEnv);
        self
    }

    pub fn with_env_bootstrap(mut self, env_name: impl Into<String>) -> Self {
        self.select_provisioning(GenesisProvisioning::FromExplicitEnv(env_name.into()));
        self
    }

    pub fn with_explicit_genesis(mut self, secret: impl AsRef<[u8]>, genoma: u32) -> Self {
        self.select_provisioning(GenesisProvisioning::Explicit {
            secret: secret.as_ref().to_vec(),
            genoma,
        });
        self
    }

    pub fn build(self) -> Result<Correspondence> {
        if let Some(message) = self.invalid_state {
            return Err(Error::InvalidBuilderState(message));
        }

        let configured_env = self.config.genesis_secret_env.clone();
        let mut correspondence = Correspondence::open_infrastructure(self.config)?;

        match self.provisioning {
            GenesisProvisioning::None => {}
            GenesisProvisioning::FromConfiguredEnv => {
                correspondence.provision_genesis_from_env(&configured_env)?;
            }
            GenesisProvisioning::FromExplicitEnv(env_name) => {
                correspondence.provision_genesis_from_env(&env_name)?;
            }
            GenesisProvisioning::Explicit { secret, genoma } => {
                correspondence.provision_genesis(&secret, genoma)?;
            }
        }

        Ok(correspondence)
    }

    fn select_provisioning(&mut self, provisioning: GenesisProvisioning) {
        if self.invalid_state.is_some() {
            return;
        }

        if !matches!(self.provisioning, GenesisProvisioning::None) {
            self.invalid_state = Some(
                "genesis provisioning strategy can only be selected once per builder",
            );
            return;
        }

        self.provisioning = provisioning;
    }
}