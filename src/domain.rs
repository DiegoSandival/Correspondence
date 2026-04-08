use ouroboros::{Celula, Genoma};

use crate::storage::{delete_membrane, get_membrane, set_membrane};
use crate::{
    AuthenticatedCellReadOutcome, CellDerivationOutcome, CellFusionOutcome, ChildSpec,
    Correspondence, Error, FreeMembraneReadOutcome, Membrane, MembraneMutationOutcome,
    MembraneReadOutcome, ResolvedCell, Result, Unauthorized,
};

impl Correspondence {
    pub fn read_membrane(
        &mut self,
        key: &str,
        cell_index: u32,
        secret: &[u8],
    ) -> Result<MembraneReadOutcome> {
        let Some(active) = self.resolve_cell(cell_index, secret)? else {
            return Ok(MembraneReadOutcome::Unauthorized(Unauthorized { cell_index: None }));
        };

        let Some(membrane) = get_membrane(&self.membranes, key)? else {
            return Ok(MembraneReadOutcome::Undefined {
                cell_index: active.index,
            });
        };

        let required_flag = if membrane.owner_index == active.original_index {
            Genoma::LEER_SELF
        } else {
            Genoma::LEER_ANY
        };

        if (active.celula.genoma & required_flag) == 0 {
            return Ok(MembraneReadOutcome::Unauthorized(Unauthorized {
                cell_index: Some(active.index),
            }));
        }

        let new_cell_index = self
            .refresh(active.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(MembraneReadOutcome::Ok {
            value: membrane.value,
            new_cell_index,
        })
    }

    pub fn read_membrane_free(&self, key: &str) -> Result<FreeMembraneReadOutcome> {
        let Some(membrane) = get_membrane(&self.membranes, key)? else {
            return Ok(FreeMembraneReadOutcome::Undefined);
        };

        let Some(owner) = self.resolve_cell_system(membrane.owner_index)? else {
            return Ok(FreeMembraneReadOutcome::Unauthorized);
        };

        if (owner.celula.genoma & Genoma::LEER_LIBRE) == 0 {
            return Ok(FreeMembraneReadOutcome::Unauthorized);
        }

        Ok(FreeMembraneReadOutcome::Ok {
            value: membrane.value,
        })
    }

    pub fn write_membrane(
        &mut self,
        key: &str,
        value: impl AsRef<[u8]>,
        cell_index: u32,
        secret: &[u8],
    ) -> Result<MembraneMutationOutcome> {
        let Some(active) = self.resolve_cell(cell_index, secret)? else {
            return Ok(MembraneMutationOutcome::Unauthorized(Unauthorized { cell_index: None }));
        };

        let value = value.as_ref();
        match get_membrane(&self.membranes, key)? {
            None => {
                set_membrane(
                    &self.membranes,
                    key,
                    &Membrane {
                        owner_index: active.original_index,
                        value: value.to_vec(),
                    },
                )?;
            }
            Some(mut membrane) => {
                let required_flag = if membrane.owner_index == active.original_index {
                    Genoma::ESCRIBIR_SELF
                } else {
                    Genoma::ESCRIBIR_ANY
                };

                if (active.celula.genoma & required_flag) == 0 {
                    return Ok(MembraneMutationOutcome::Unauthorized(Unauthorized {
                        cell_index: Some(active.index),
                    }));
                }

                membrane.value = value.to_vec();
                set_membrane(&self.membranes, key, &membrane)?;
            }
        }

        let new_cell_index = self
            .refresh(active.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(MembraneMutationOutcome::Ok { new_cell_index })
    }

    pub fn delete_membrane(
        &mut self,
        key: &str,
        cell_index: u32,
        secret: &[u8],
    ) -> Result<MembraneMutationOutcome> {
        let Some(active) = self.resolve_cell(cell_index, secret)? else {
            return Ok(MembraneMutationOutcome::Unauthorized(Unauthorized { cell_index: None }));
        };

        let Some(membrane) = get_membrane(&self.membranes, key)? else {
            return Ok(MembraneMutationOutcome::Undefined {
                cell_index: active.index,
            });
        };

        let required_flag = if membrane.owner_index == active.original_index {
            Genoma::BORRAR_SELF
        } else {
            Genoma::BORRAR_ANY
        };

        if (active.celula.genoma & required_flag) == 0 {
            return Ok(MembraneMutationOutcome::Unauthorized(Unauthorized {
                cell_index: Some(active.index),
            }));
        }

        let _ = delete_membrane(&self.membranes, key)?;
        let new_cell_index = self
            .refresh(active.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(MembraneMutationOutcome::Ok { new_cell_index })
    }

    pub fn derive_cell(
        &mut self,
        cell_index: u32,
        parent_secret: &[u8],
        child_secret: &[u8],
        child: ChildSpec,
    ) -> Result<CellDerivationOutcome> {
        let Some(active) = self.resolve_cell(cell_index, parent_secret)? else {
            return Ok(CellDerivationOutcome::Unauthorized(Unauthorized { cell_index: None }));
        };

        if (active.celula.genoma & Genoma::DIFERIR) == 0 {
            return Ok(CellDerivationOutcome::Unauthorized(Unauthorized {
                cell_index: Some(active.index),
            }));
        }

        if (active.celula.genoma & child.genoma) != child.genoma {
            return Ok(CellDerivationOutcome::Unauthorized(Unauthorized {
                cell_index: Some(active.index),
            }));
        }

        let child_cell = Celula::with_secret(
            child.salt,
            child_secret,
            child.genoma,
            child.x,
            child.y,
            child.z,
        );

        let deferred_index = self.ouroboros.append(child_cell)?;
        let new_cell_index = self
            .refresh(active.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(CellDerivationOutcome::Ok {
            deferred_index,
            new_cell_index,
        })
    }

    pub fn fuse_cells(
        &mut self,
        cell_index_a: u32,
        secret_a: &[u8],
        cell_index_b: u32,
        secret_b: &[u8],
        child_secret: &[u8],
        child: ChildSpec,
    ) -> Result<CellFusionOutcome> {
        let active_a = self.resolve_cell(cell_index_a, secret_a)?;
        let active_b = self.resolve_cell(cell_index_b, secret_b)?;

        if active_a.is_none() || active_b.is_none() {
            return Ok(CellFusionOutcome::Unauthorized {
                cell_index_a: active_a.as_ref().map(|cell| cell.index),
                cell_index_b: active_b.as_ref().map(|cell| cell.index),
            });
        }

        let active_a = active_a.expect("checked is_some above");
        let active_b = active_b.expect("checked is_some above");

        if (active_a.celula.genoma & Genoma::FUSIONAR) == 0
            || (active_b.celula.genoma & Genoma::FUSIONAR) == 0
        {
            return Ok(CellFusionOutcome::Unauthorized {
                cell_index_a: Some(active_a.index),
                cell_index_b: Some(active_b.index),
            });
        }

        let child_genoma = active_a.celula.genoma | active_b.celula.genoma;
        let child_cell = Celula::with_secret(
            child.salt,
            child_secret,
            child_genoma,
            child.x,
            child.y,
            child.z,
        );

        let child_index = self.ouroboros.append(child_cell)?;
        let new_cell_index_a = self
            .refresh(active_a.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;
        let new_cell_index_b = self
            .refresh(active_b.index)?
            .ok_or(Error::BootstrapMetadataMissing)?;

        Ok(CellFusionOutcome::Ok {
            child_index,
            new_cell_index_a,
            new_cell_index_b,
        })
    }

    pub fn read_authenticated_cell(
        &self,
        cell_index: u32,
        secret: &[u8],
    ) -> Result<AuthenticatedCellReadOutcome> {
        let Some(active) = self.resolve_cell(cell_index, secret)? else {
            return Ok(AuthenticatedCellReadOutcome::Unauthorized);
        };

        Ok(AuthenticatedCellReadOutcome::Ok {
            celula: active.celula,
            cell_index: active.index,
        })
    }

    fn refresh(&mut self, cell_index: u32) -> Result<Option<u32>> {
        let Some(original_cell) = self.read_cell_system_raw(cell_index)? else {
            return Ok(None);
        };

        let renewed_cell = Celula::new(
            original_cell.hash,
            original_cell.salt,
            original_cell.genoma,
            original_cell.x,
            original_cell.y,
            original_cell.z,
        );

        let new_index = self.ouroboros.append(renewed_cell)?;
        let migrated_cell = Celula::new(
            original_cell.hash,
            original_cell.salt,
            original_cell.genoma | Genoma::MIGRADA,
            new_index,
            original_cell.y,
            original_cell.z,
        );
        self.ouroboros.update(cell_index, migrated_cell)?;

        Ok(Some(new_index))
    }

    fn resolve_cell(&self, cell_index: u32, secret: &[u8]) -> Result<Option<ResolvedCell>> {
        self.resolve_cell_inner(cell_index, |index| self.read_cell_auth_raw(index, secret))
    }

    fn resolve_cell_system(&self, cell_index: u32) -> Result<Option<ResolvedCell>> {
        self.resolve_cell_inner(cell_index, |index| self.read_cell_system_raw(index))
    }

    fn resolve_cell_inner<F>(&self, cell_index: u32, mut read: F) -> Result<Option<ResolvedCell>>
    where
        F: FnMut(u32) -> Result<Option<Celula>>,
    {
        let mut index = cell_index;
        let Some(mut cell) = read(index)? else {
            return Ok(None);
        };

        for _ in 0..self.max_records {
            if (cell.genoma & Genoma::MIGRADA) == 0 {
                return Ok(Some(ResolvedCell {
                    celula: cell,
                    index,
                    original_index: cell_index,
                }));
            }

            index = cell.x;
            let Some(next_cell) = read(index)? else {
                return Ok(None);
            };
            cell = next_cell;
        }

        Err(Error::ResolutionLoop {
            start_index: cell_index,
        })
    }

    fn read_cell_auth_raw(&self, cell_index: u32, secret: &[u8]) -> Result<Option<Celula>> {
        match self.ouroboros.read_auth(cell_index, secret) {
            Ok(cell) if cell.is_empty() => Ok(None),
            Ok(cell) => Ok(Some(cell)),
            Err(ouroboros::Error::Unauthorized) => Ok(None),
            Err(ouroboros::Error::IndexOutOfBounds { .. }) => Ok(None),
            Err(error) => Err(Error::from(error)),
        }
    }

    fn read_cell_system_raw(&self, cell_index: u32) -> Result<Option<Celula>> {
        match self.ouroboros.read(cell_index) {
            Ok(cell) if cell.is_empty() => Ok(None),
            Ok(cell) => Ok(Some(cell)),
            Err(ouroboros::Error::IndexOutOfBounds { .. }) => Ok(None),
            Err(error) => Err(Error::from(error)),
        }
    }
}