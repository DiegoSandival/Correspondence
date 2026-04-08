use ouroboros::Celula;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorrespondenceConfig {
    pub ouroboros_config_path: std::path::PathBuf,
    pub membranes_path: std::path::PathBuf,
    pub genesis_secret_env: String,
}

impl CorrespondenceConfig {
    pub fn new(
        ouroboros_config_path: impl Into<std::path::PathBuf>,
        membranes_path: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            ouroboros_config_path: ouroboros_config_path.into(),
            membranes_path: membranes_path.into(),
            genesis_secret_env: "GENESIS_SECRET".to_string(),
        }
    }

    pub fn with_genesis_secret_env(mut self, env_name: impl Into<String>) -> Self {
        self.genesis_secret_env = env_name.into();
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChildSpec {
    pub salt: [u8; 16],
    pub genoma: u32,
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Membrane {
    pub owner_index: u32,
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCell {
    pub celula: Celula,
    pub index: u32,
    pub original_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unauthorized {
    pub cell_index: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MembraneReadOutcome {
    Ok { value: Vec<u8>, new_cell_index: u32 },
    Undefined { cell_index: u32 },
    Unauthorized(Unauthorized),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FreeMembraneReadOutcome {
    Ok { value: Vec<u8> },
    Undefined,
    Unauthorized,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MembraneMutationOutcome {
    Ok { new_cell_index: u32 },
    Undefined { cell_index: u32 },
    Unauthorized(Unauthorized),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthenticatedCellReadOutcome {
    Ok { celula: Celula, cell_index: u32 },
    Unauthorized,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CellDerivationOutcome {
    Ok {
        deferred_index: u32,
        new_cell_index: u32,
    },
    Unauthorized(Unauthorized),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CellFusionOutcome {
    Ok {
        child_index: u32,
        new_cell_index_a: u32,
        new_cell_index_b: u32,
    },
    Unauthorized {
        cell_index_a: Option<u32>,
        cell_index_b: Option<u32>,
    },
}
