use std::collections::{HashMap, BTreeMap, BTreeSet};

use vm::types::{TypeSignature, FunctionType, QualifiedContractIdentifier, TraitIdentifier};
use vm::types::signatures::FunctionSignature;
use vm::database::{ClaritySerializable, ClarityDeserializable,
                   RollbackWrapper, MarfedKV, ClarityBackingStore};
use vm::analysis::errors::{CheckError, CheckErrors, CheckResult};
use vm::analysis::type_checker::{ContractAnalysis};
use vm::representations::{ClarityName};

pub struct AnalysisDatabase <'a> {
    store: RollbackWrapper <'a>
}

impl ClaritySerializable for ContractAnalysis {
    fn serialize(&self) -> String {
        serde_json::to_string(self)
            .expect("Failed to serialize vm.Value")
    }
}

impl ClarityDeserializable<ContractAnalysis> for ContractAnalysis {
    fn deserialize(json: &str) -> Self {
        serde_json::from_str(json)
            .expect("Failed to serialize vm.Value")
    }
}

impl <'a> AnalysisDatabase <'a> {
    pub fn new(store: &'a mut dyn ClarityBackingStore) -> AnalysisDatabase<'a> {
        AnalysisDatabase {
            store: RollbackWrapper::new(store)
        }
    }

    pub fn execute <F, T, E> (&mut self, f: F) -> Result<T,E> where F: FnOnce(&mut Self) -> Result<T,E>, {
        self.begin();
        let result = f(self)
            .or_else(|e| {
                self.roll_back();
                Err(e)
            })?;
        self.commit();
        Ok(result)
    }

    pub fn begin(&mut self) {
        self.store.nest();
    }

    pub fn commit(&mut self) {
        self.store.commit();
    }

    pub fn roll_back(&mut self) {
        self.store.rollback();
    }

    fn storage_key() -> &'static str {
        "analysis"
    }

    // used by tests to ensure that
    //   the contract -> contract hash key exists in the marf
    //    even if the contract isn't published.
    #[cfg(test)]
    pub fn test_insert_contract_hash(&mut self, contract_identifier: &QualifiedContractIdentifier) {
        use util::hash::Sha512Trunc256Sum;
        self.store.prepare_for_contract_metadata(contract_identifier, Sha512Trunc256Sum([0; 32]));
    }

    pub fn has_contract(&mut self, contract_identifier: &QualifiedContractIdentifier) -> bool {
        self.store.has_metadata_entry(contract_identifier, AnalysisDatabase::storage_key())
    }

    pub fn load_contract(&mut self, contract_identifier: &QualifiedContractIdentifier) -> Option<ContractAnalysis> {
        self.store.get_metadata(contract_identifier, AnalysisDatabase::storage_key())
            // treat NoSuchContract error thrown by get_metadata as an Option::None --
            //    the analysis will propagate that as a CheckError anyways.
            .ok()?
            .map(|x| ContractAnalysis::deserialize(&x))
    }

    pub fn insert_contract(&mut self, contract_identifier: &QualifiedContractIdentifier, contract: &ContractAnalysis) -> CheckResult<()> {
        let key = AnalysisDatabase::storage_key();
        if self.store.has_metadata_entry(contract_identifier, key) {
            return Err(CheckErrors::ContractAlreadyExists(contract_identifier.to_string()).into())
        }

        self.store.insert_metadata(contract_identifier, key, &contract.serialize());
        Ok(())
    }

    pub fn get_public_function_type(&mut self, contract_identifier: &QualifiedContractIdentifier, function_name: &str) -> CheckResult<Option<FunctionType>> {
        // TODO: this function loads the whole contract to obtain the function type.
        //         but it doesn't need to -- rather this information can just be 
        //         stored as its own entry. the analysis cost tracking currently only
        //         charges based on the function type size.
        let contract = self.load_contract(contract_identifier)
            .ok_or(CheckErrors::NoSuchContract(contract_identifier.to_string()))?;
        Ok(contract.get_public_function_type(function_name)
           .cloned())
    }

    pub fn get_read_only_function_type(&mut self, contract_identifier: &QualifiedContractIdentifier, function_name: &str) -> CheckResult<Option<FunctionType>> {
        // TODO: this function loads the whole contract to obtain the function type.
        //         but it doesn't need to -- rather this information can just be 
        //         stored as its own entry. the analysis cost tracking currently only
        //         charges based on the function type size.
        let contract = self.load_contract(contract_identifier)
            .ok_or(CheckErrors::NoSuchContract(contract_identifier.to_string()))?;
        Ok(contract.get_read_only_function_type(function_name)
           .cloned())
    }

    pub fn get_defined_trait(&mut self, contract_identifier: &QualifiedContractIdentifier, trait_name: &str) -> CheckResult<Option<BTreeMap<ClarityName, FunctionSignature>>> {
        // TODO: this function loads the whole contract to obtain the function type.
        //         but it doesn't need to -- rather this information can just be 
        //         stored as its own entry. the analysis cost tracking currently only
        //         charges based on the function type size.
        let contract = self.load_contract(contract_identifier)
            .ok_or(CheckErrors::NoSuchContract(contract_identifier.to_string()))?;
        Ok(contract.get_defined_trait(trait_name)
           .cloned())
    }

    pub fn get_implemented_traits(&mut self, contract_identifier: &QualifiedContractIdentifier) -> CheckResult<BTreeSet<TraitIdentifier>> {
        let contract = self.load_contract(contract_identifier)
            .ok_or(CheckErrors::NoSuchContract(contract_identifier.to_string()))?;
        Ok(contract.implemented_traits)
    }

    pub fn get_map_type(&mut self, contract_identifier: &QualifiedContractIdentifier, map_name: &str) -> CheckResult<(TypeSignature, TypeSignature)> {
        let contract = self.load_contract(contract_identifier)
            .ok_or(CheckErrors::NoSuchContract(contract_identifier.to_string()))?;
        let map_type = contract.get_map_type(map_name)
            .ok_or(CheckErrors::NoSuchMap(map_name.to_string()))?;
        Ok(map_type.clone())
    }

}
