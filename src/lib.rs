mod builder;
mod domain;
mod error;
mod infrastructure;
mod model;
mod provisioning;
mod registry;
mod server;
mod storage;
mod transport;

use ouroboros::OuroborosDb;
use redb::Database;

pub use crate::builder::CorrespondenceBuilder;
pub use crate::error::{Error, Result};
pub use crate::model::{
    AuthenticatedCellReadOutcome, CellDerivationOutcome, CellFusionOutcome, ChildSpec,
    CorrespondenceConfig, FreeMembraneReadOutcome, Membrane, MembraneMutationOutcome,
    MembraneReadOutcome, ResolvedCell, Unauthorized,
};
pub use crate::registry::{DatabaseRegistry, DatabaseSummary};
pub use crate::server::{serve_websocket, WebServerConfig};
pub use crate::transport::{decode_command, encode_response, execute_command, RawCommand, RawResponse};
pub struct Correspondence {
    ouroboros: OuroborosDb,
    membranes: Database,
    max_records: u32,
}

impl Correspondence {
    pub fn max_records(&self) -> u32 {
        self.max_records
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::provisioning::genesis_genoma;
    use ouroboros::Genoma;

    fn temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("correspondence-{unique}"));
        fs::create_dir_all(&path).expect("temporary directory should be created");
        path
    }

    fn write_ouroboros_config(dir: &std::path::Path) -> PathBuf {
        let config_path = dir.join("ouroboros.toml");
        fs::write(&config_path, "data_path = \"ouroboros.db\"\nmax_records = 32\n")
            .expect("config file should be written");
        config_path
    }

    #[test]
    fn bootstrap_creates_genesis_cell() {
        let dir = temp_dir();
        let config_path = write_ouroboros_config(&dir);
        let membranes_path = dir.join("membranes.redb");
        let env_name = format!("GENESIS_SECRET_TEST_{}", std::process::id());
        unsafe {
            std::env::set_var(&env_name, "diarsaba");
        }

        let config = CorrespondenceConfig::new(config_path, membranes_path)
            .with_genesis_secret_env(&env_name);
        let correspondence = Correspondence::builder(config)
            .with_configured_env_bootstrap()
            .build()
            .expect("correspondence should open");

        assert_eq!(correspondence.genesis_index().expect("genesis index"), 0);

        fs::remove_dir_all(dir).expect("temporary directory should be removed");
    }

    #[test]
    fn explicit_genesis_initialization_works() {
        let dir = temp_dir();
        let config_path = write_ouroboros_config(&dir);
        let membranes_path = dir.join("membranes.redb");

        let config = CorrespondenceConfig::new(config_path, membranes_path);
        let mut correspondence = Correspondence::builder(config)
            .without_genesis_provisioning()
            .build()
            .expect("correspondence should open");

        let index = correspondence
            .provision_genesis(b"diarsaba", Correspondence::default_genesis_genoma())
            .expect("genesis should initialize");

        assert_eq!(index, 0);
        assert_eq!(correspondence.genesis_index().expect("genesis index"), 0);

        fs::remove_dir_all(dir).expect("temporary directory should be removed");
    }

    #[test]
    fn builder_rejects_multiple_provisioning_strategies() {
        let dir = temp_dir();
        let config_path = write_ouroboros_config(&dir);
        let membranes_path = dir.join("membranes.redb");

        let config = CorrespondenceConfig::new(config_path, membranes_path);
        let result = Correspondence::builder(config)
            .with_configured_env_bootstrap()
            .with_explicit_genesis(b"diarsaba", Correspondence::default_genesis_genoma())
            .build();

        assert!(matches!(result, Err(Error::InvalidBuilderState(_))));

        fs::remove_dir_all(dir).expect("temporary directory should be removed");
    }

    #[test]
    fn genesis_genoma_matches_requested_permissions() {
        let expected = Genoma::LEER_SELF
            | Genoma::LEER_ANY
            | Genoma::ESCRIBIR_SELF
            | Genoma::ESCRIBIR_ANY
            | Genoma::BORRAR_SELF
            | Genoma::BORRAR_ANY
            | Genoma::DIFERIR
            | Genoma::LEER_LIBRE;

        assert_eq!(genesis_genoma(), expected);
        assert_eq!(genesis_genoma() & Genoma::FUSIONAR, 0);
        assert_eq!(genesis_genoma() & Genoma::CLONAR, 0);
        assert_eq!(genesis_genoma() & Genoma::DOMINANTE, 0);
    }

    #[test]
    fn write_read_delete_round_trip_refreshes_index() {
        let dir = temp_dir();
        let config_path = write_ouroboros_config(&dir);
        let membranes_path = dir.join("membranes.redb");
        let env_name = format!("GENESIS_SECRET_TEST_{}", std::process::id());
        unsafe {
            std::env::set_var(&env_name, "diarsaba");
        }

        let config = CorrespondenceConfig::new(config_path, membranes_path)
            .with_genesis_secret_env(&env_name);
        let mut correspondence = Correspondence::builder(config)
            .with_configured_env_bootstrap()
            .build()
            .expect("correspondence should open");

        let genesis_index = correspondence.genesis_index().expect("genesis index");
        let MembraneMutationOutcome::Ok { new_cell_index } = correspondence
            .write_membrane("hola", b"mundo", genesis_index, b"diarsaba")
            .expect("write should succeed")
        else {
            panic!("write should return ok");
        };

        let MembraneReadOutcome::Ok {
            value,
            new_cell_index: read_index,
        } = correspondence
            .read_membrane("hola", new_cell_index, b"diarsaba")
            .expect("read should succeed")
        else {
            panic!("read should return ok");
        };
        assert_eq!(value, b"mundo");

        let FreeMembraneReadOutcome::Ok { value } = correspondence
            .read_membrane_free("hola")
            .expect("free read should succeed")
        else {
            panic!("free read should return ok");
        };
        assert_eq!(value, b"mundo");

        let MembraneMutationOutcome::Ok {
            new_cell_index: delete_index,
        } = correspondence
            .delete_membrane("hola", read_index, b"diarsaba")
            .expect("delete should succeed")
        else {
            panic!("delete should return ok");
        };

        let MembraneMutationOutcome::Undefined { cell_index } = correspondence
            .delete_membrane("hola", delete_index, b"diarsaba")
            .expect("second delete should succeed")
        else {
            panic!("second delete should return undefined");
        };
        assert_eq!(cell_index, delete_index);

        fs::remove_dir_all(dir).expect("temporary directory should be removed");
    }
}